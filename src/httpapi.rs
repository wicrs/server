use async_graphql::{EmptySubscription, Request as GraphQLRequest, Schema};

use tokio::sync::RwLock;

use chrono::Duration;
use chrono::Utc;

use xactor::{Actor, Addr};

use std::collections::HashMap;
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
use pgp::types::KeyTrait;
use pgp::Deserializable;
use pgp::Message as OpenPGPMessage;
use pgp::SignedPublicKey;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::graphql_model::{MutationRoot, QueryRoot};
use crate::server::Server;
use crate::signing::{self, KeyPair};

type SchemaType = Schema<QueryRoot, MutationRoot, EmptySubscription>;
type WsAuthList = Arc<RwLock<HashMap<String, String>>>;
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
            async {
                let message =
                    OpenPGPMessage::from_armor_single(std::io::Cursor::new(body.to_vec()))?.0;
                message.verify(&requester_public_key)?;
                let message = message.decompress()?;
                if let pgp::composed::message::Message::Signed {
                    message,
                    one_pass_signature: _,
                    signature,
                } = message
                {
                    let sig_time = signature.created().ok_or(Error::InvalidMessage)?;
                    let expire = Utc::now()
                        .checked_add_signed(Duration::seconds(10))
                        .ok_or(Error::UnexpectedServerArg)?;
                    if sig_time > &expire {
                        return Err(Error::InvalidMessage);
                    }
                    let message = message.ok_or(Error::InvalidMessage)?;
                    let literal_message = message.get_literal().ok_or(Error::InvalidMessage)?;

                    let content = String::from_utf8(literal_message.data().to_vec())?;
                    Ok((content, hex::encode(requester_public_key.fingerprint())))
                } else {
                    Err(Error::InvalidMessage)
                }
            }
            .await
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
                            .execute(request.data(server).data(hex::encode(fingerprint)))
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

    let auth_list: WsAuthList = Arc::new(RwLock::new(HashMap::new()));
    let auth_list_ws = auth_list.clone();

    let pre_web_socket = warp::path!("v2" / "websocket_init")
        .map(move || (auth_list.clone(), key_pair_ws.clone()))
        .and(signed_body)
        .and_then(
            |(auth_list, key_pair): (WsAuthList, KeyPairArc),
             (content, fingerprint): (String, String)| async move {
                async {
                    if content == format!("websocket_connect {}", fingerprint) {
                        let key = rand::random::<u128>().to_string();
                        auth_list.write().await.insert(fingerprint, key.clone());
                        let resp = OpenPGPMessage::Literal(LiteralData::from_str(
                            "websocket_connect_key",
                            &key,
                        ))
                        .sign(&key_pair.secret_key, String::new, HashAlgorithm::SHA2_256)?
                        .to_armored_string(None)?;
                        Ok::<_, Error>(HttpResponse::new(resp))
                    } else {
                        Err(Error::InvalidMessage)
                    }
                }
                .await
                .map_err(warp::reject::custom)
            },
        );

    let web_socket = warp::path!("v2" / "websocket")
        .map(move || (auth_list_ws.clone(), server.clone()))
        .and(warp::ws())
        .and(signed_body)
        .and_then(
            |(auth_list, server): (WsAuthList, ServerAddr),
             ws: Ws,
             (content, fingerprint): (String, String)| async move {
                let read = auth_list.read().await;
                if let Some(key) = read.get(&fingerprint) {
                    if key == &content {
                        drop(read);
                        let _ = auth_list.write().await.remove(&fingerprint);
                        let server = server.clone();
                        return Ok(ws
                            .on_upgrade(move |websocket| async move {
                                let _ = crate::websocket::handle_connection(
                                    websocket,
                                    fingerprint,
                                    server,
                                )
                                .await;
                            })
                            .into_response());
                    }
                }
                Err(Error::WsNotAuthenticated).map_err(warp::reject::custom)
            },
        );

    let routes = graphql_post.or(web_socket).or(pre_web_socket);
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
