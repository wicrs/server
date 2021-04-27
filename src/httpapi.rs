use async_graphql::{EmptySubscription, Request as GraphQLRequest, Schema};

use xactor::{Actor, Addr};

use std::convert::Infallible;
use std::convert::TryInto;
use std::net::SocketAddr;
use std::sync::Arc;

use warp::hyper::body::Bytes;
use warp::ws::Ws;
use warp::Reply;
use warp::{http::Response as HttpResponse, Filter};

use pgp::crypto::HashAlgorithm;
use pgp::packet::LiteralData;
use pgp::Message as OpenPGPMessage;
use pgp::SignedPublicKey;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::graphql_model::{MutationRoot, QueryRoot};
use crate::server::Server;
use crate::signing::{self, KeyPair};

type SchemaType = Schema<QueryRoot, MutationRoot, EmptySubscription>;
type ServerAddr = Arc<Addr<Server>>;
type KeyPairArc = Arc<KeyPair>;

pub async fn start(config: Config) -> Result {
    let key_pair = Arc::new(
        KeyPair::load_or_create(
            "WICRS Server <server@wic.rs>",
            signing::SECRET_KEY_PATH,
            signing::PUBLIC_KEY_PATH,
        )
        .await?,
    );
    let key_pair_ws = key_pair.clone();
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
            .await
            .and_then(|key| {
                key.verify()?;
                Ok(key)
            })
            .map_err(warp::reject::custom)
    });

    let signed_body = public_key_filter.and(warp::body::bytes()).and_then(
        |requester_public_key: SignedPublicKey, body: Bytes| async move {
            crate::signing::verify_message_extract(
                &requester_public_key,
                String::from_utf8(body.to_vec())
                    .map_err(|_| warp::reject::custom(Error::InvalidText))?,
            )
            .map_err(warp::reject::custom)
        },
    );

    let graphql_post = warp::any()
        .map(move || (graphql_server_arc.clone(), schema.clone(), key_pair.clone()))
        .and(warp::path("graphql"))
        .and(signed_body)
        .and_then(
            |(server, schema, key_pair): (ServerAddr, SchemaType, KeyPairArc),
             (content, fingerprint): (String, String)| async move {
                Ok::<_, Infallible>(
                    async {
                        let request = GraphQLRequest::new(content);
                        let resp = schema
                            .execute(request.data(server).data(hex::encode_upper(fingerprint)))
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

    let web_socket = warp::path!("v2" / "websocket")
        .map(move || (server.clone(), key_pair_ws.clone()))
        .and(public_key_filter)
        .and(warp::ws())
        .map(
            move |(server, key_pair): (ServerAddr, KeyPairArc),
                  public_key: SignedPublicKey,
                  ws: Ws| {
                ws.on_upgrade(move |websocket| async move {
                    let _ = crate::websocket::handle_connection(
                        websocket, public_key, key_pair, server,
                    )
                    .await;
                })
            },
        );

    let routes = graphql_post.or(web_socket);
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
