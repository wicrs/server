use crate::config::Config;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use static_assertions::_core::convert::TryInto;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use warp::Reply;
use warp::{http::Response as HttpResponse, Filter};

use pgp::crypto::HashAlgorithm;
use pgp::packet::LiteralData;
use pgp::types::KeyTrait;
use pgp::Deserializable;
use pgp::Message as OpenPGPMessage;
use pgp::SignedPublicKey;

use crate::error::{Error, Result};
use crate::graphql_model::{MutationRoot, QueryRoot};
use crate::server::Server;
use crate::signing::KeyPair;
use async_graphql::*;
use xactor::{Actor, Addr};

pub async fn start(config: Config) -> Result {
    let key_pair = KeyPair::load_or_create("WICRS Server <server@wic.rs>")?;
    let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish();
    let server = Arc::new(
        Server::new()
            .await?
            .start()
            .await
            .map_err(|_| Error::ServerStartFailed)?,
    );
    let graphql_server_arc = server.clone();

    let public_key_filter = warp::header("pgp-fingerprint").and_then(|header: String| async move {
        crate::signing::get_or_import_public_key(&header)
            .and_then(|key| {
                key.verify()?;
                Ok(key)
            })
            .map_err(warp::reject::custom)
    });

    let graphql_post = warp::any()
        .map(move || (graphql_server_arc.clone(), schema.clone(), key_pair.clone()))
        .and(warp::path("graphql"))
        .and(warp::body::bytes())
        .and(public_key_filter)
        .and_then(
            |(server, schema, key_pair): (
                Arc<Addr<Server>>,
                Schema<QueryRoot, MutationRoot, EmptySubscription>,
                KeyPair,
            ),
             body: warp::hyper::body::Bytes,
             public_key: SignedPublicKey| async move {
                Ok::<_, Infallible>(
                    async {
                        let message =
                            OpenPGPMessage::from_armor_single(std::io::Cursor::new(body.to_vec()))?
                                .0;
                        message.verify(&public_key)?;
                        let message = message.decompress()?;
                        let literal_message = message.get_literal().ok_or(Error::InvalidMessage)?;

                        let content = String::from_utf8(literal_message.data().to_vec())?;
                        let request = Request::new(content);
                        let resp = schema
                            .execute(
                                request
                                    .data(server)
                                    .data(hex::encode(public_key.fingerprint())),
                            )
                            .await;
                        let message = OpenPGPMessage::Literal(LiteralData::from_str(
                            "",
                            &resp.data.to_string(),
                        ))
                        .sign(
                            &key_pair.secret_key,
                            String::new,
                            HashAlgorithm::SHA2_256,
                        )?;
                        let mut reply = HttpResponse::<String>::default();
                        reply.body_mut().push_str(&message.to_armored_string(None)?);
                        let mut response = warp::reply::with_header(
                            reply,
                            "content-type",
                            "application/pgp-encrypted",
                        )
                        .into_response();

                        if resp.is_ok() {
                            if let Some(value) = resp.cache_control.value() {
                                if let Ok(value) = value.try_into() {
                                    response.headers_mut().insert("cache-control", value);
                                }
                            }
                            for (name, value) in resp.http_headers {
                                if let Some(name) = name {
                                    if let Ok(value) = value.try_into() {
                                        response.headers_mut().append(name, value);
                                    }
                                }
                            }
                        }
                        Ok::<_, Error>(response)
                    }
                    .await
                    .map_or_else(
                        |e| e.into_response(),
                        |query_response| query_response.into_response(),
                    ),
                )
            },
        );

    let web_socket =
        warp::path!("v2" / "websocket")
            .and(warp::ws())
            .map(move |ws: warp::ws::Ws| {
                let server = server.clone();
                ws.on_upgrade(move |websocket| async move {
                    let _ =
                        crate::websocket::handle_connection(websocket, "test".to_string(), server)
                            .await;
                })
            });

    let graphql_playground = warp::path("graphql_playground").and(warp::get()).map(|| {
        HttpResponse::builder()
            .header("content-type", "text/html")
            .body(playground_source(
                GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/"),
            ))
    });

    let routes = graphql_playground.or(graphql_post).or(web_socket);
    warp::serve(routes)
        .run(
            config
                .address
                .parse::<SocketAddr>()
                .expect("Unable to parse server bind address."),
        )
        .await;
    Ok(())
}
