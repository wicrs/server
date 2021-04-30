use async_graphql::{EmptySubscription, Request as GraphQLRequest, Schema};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use xactor::Actor;

use std::convert::Infallible;
use std::convert::TryInto;
use std::net::SocketAddr;
use std::sync::Arc;

use warp::hyper::body::Bytes;
use warp::ws::Ws;
use warp::Reply;
use warp::{http::Response as HttpResponse, Filter};

use pgp::Message as OpenPGPMessage;
use pgp::SignedPublicKey;
use pgp::{crypto::HashAlgorithm, types::CompressionAlgorithm};
use pgp::{packet::LiteralData, types::KeyTrait};

use crate::server::Server;
use crate::signing::{self, KeyPair};
use crate::ID;
use crate::{channel::Message, config::Config};
use crate::{
    error::{Error, Result},
    hub::Hub,
    permission::ChannelPermission,
};
use crate::{
    graphql_model::{MutationRoot, QueryRoot},
    server::ServerNotification,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerInfo {
    pub version: String,
    pub public_key_fingerprint: String,
    pub key_server: String,
}

pub async fn start(config: Config) -> Result {
    let key_pair = Arc::new(
        KeyPair::load_or_create(
            "WICRS Server <server@wic.rs>",
            signing::SECRET_KEY_PATH,
            signing::PUBLIC_KEY_PATH,
        )
        .await?,
    );
    let upload_key = async {
        let armoured_pub_key = key_pair.public_key.to_armored_string(None)?;
        let form = reqwest::multipart::Form::new().text("keytext", armoured_pub_key);
        let url = format!("{}/pks/add", config.key_server);

        let response = reqwest::Client::builder()
            .build()?
            .post(&url)
            .multipart(form)
            .send()
            .await?;
        if response.status() == StatusCode::OK {
            Ok(())
        } else {
            Err(Error::Other(
                "Failed to upload the server's public PGP key to the selected key server."
                    .to_string(),
            ))
        }
    }
    .await;
    if upload_key.is_err() {
        println!("WARNING: Unable to upload public key to key server.");
    }
    upload_key?;
    let server_fingerprint = hex::encode_upper(key_pair.secret_key.fingerprint());
    let key_pair_ws = key_pair.clone();
    let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish();
    let server = Arc::new(
        Server::new(key_pair.secret_key.clone())
            .await?
            .start()
            .await
            .map_err(|_| Error::ServerStartFailed)?,
    );
    let send_message_server_arc = server.clone();
    let key_pair_send = key_pair.clone();
    let key_pair_send_init = key_pair.clone();
    let graphql_server_arc = server.clone();
    let key_server_url = config.key_server.clone();
    let public_key_filter =
        warp::any()
            .and(warp::header("pgp-fingerprint"))
            .and_then(move |header: String| {
                let key_server_url = key_server_url.clone();
                async move {
                    crate::signing::get_or_import_public_key(&header, &key_server_url)
                        .await
                        .and_then(|key| {
                            key.verify()?;
                            Ok(key)
                        })
                        .map_err(warp::reject::custom)
                }
            });

    let signed_body_pub_key = public_key_filter.clone();

    let signed_body = signed_body_pub_key.and(warp::body::bytes()).and_then(
        |requester_public_key: SignedPublicKey, body: Bytes| async move {
            let text = String::from_utf8(body.to_vec())
                .map_err(|e| warp::reject::custom(Error::from(e)))?;
            crate::signing::verify_message_extract(&requester_public_key, &text)
                .map_err(warp::reject::custom)
        },
    );

    let signed_body_graphql = signed_body.clone();
    let graphql_key_pair = key_pair.clone();
    let graphql_post = warp::any()
        .and(warp::path!("v3" / "graphql"))
        .and(signed_body_graphql)
        .and_then(move |(content, fingerprint): (String, String)| {
            let server = graphql_server_arc.clone();
            let schema = schema.clone();
            let key_pair = graphql_key_pair.clone();
            async move {
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
                        .sign(&key_pair.secret_key, String::new, HashAlgorithm::SHA2_256)?
                        .compress(CompressionAlgorithm::ZIP)?;
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
                    .map_or_else(|e| e.into_response(), |r| r.into_response()),
                )
            }
        });

    let signed_body_smi = signed_body.clone();

    let send_message_init = warp::any()
        .and(warp::path!("v3" / "send_message_init" / String / String))
        .and(signed_body_smi)
        .and_then(
            move |hub_id: String, channel_id: String, (content, sender): (String, String)| {
                let key_pair = key_pair_send_init.clone();
                async move {
                    Ok::<_, Infallible>(
                        async {
                            let hub_id = ID::parse_str(&hub_id)?;
                            let hub = Hub::load(hub_id).await?;
                            let channel_id = ID::parse_str(&channel_id)?;
                            let member = hub.get_member(&sender)?;
                            crate::check_permission!(
                                &member,
                                channel_id,
                                ChannelPermission::Write,
                                &hub
                            );
                            Ok::<_, Error>(
                                Message::new(sender, content, hub_id, channel_id)
                                    .sign(&key_pair.secret_key, String::new)?
                                    .compress(CompressionAlgorithm::ZIP)?
                                    .to_armored_string(None)?,
                            )
                        }
                        .await
                        .map_or_else(|e| e.into_response(), |r| r.into_response()),
                    )
                }
            },
        );

    let send_message_pub_key = public_key_filter.clone();

    let send_message = warp::any()
        .and(warp::path!("v3" / "send_message"))
        .and(send_message_pub_key)
        .and(warp::body::bytes())
        .and_then(move |client_public_key: SignedPublicKey, body: Bytes| {
            let key_pair = key_pair_send.clone();
            let server = send_message_server_arc.clone();
            async move {
                Ok::<_, Infallible>(
                    async {
                        let body = String::from_utf8(body.to_vec())?;
                        let message = Message::from_double_signed_verify(
                            &body,
                            &key_pair.public_key,
                            &client_public_key,
                        )?;
                        let _ = server.send(ServerNotification::NewMessage(
                            message.hub_id,
                            message.channel_id,
                            message.id,
                            body,
                            message,
                        ));
                        Ok::<_, Error>(warp::reply())
                    }
                    .await
                    .map_or_else(|e| e.into_response(), |r| r.into_response()),
                )
            }
        });

    let web_socket = warp::path!("v3" / "websocket")
        .and(public_key_filter)
        .and(warp::ws())
        .map(move |public_key: SignedPublicKey, ws: Ws| {
            let key_pair = key_pair_ws.clone();
            let server = server.clone();
            ws.on_upgrade(move |websocket| async move {
                let _ =
                    crate::websocket::handle_connection(websocket, public_key, key_pair, server)
                        .await;
            })
        });

    let server_info_struct = ServerInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        public_key_fingerprint: server_fingerprint,
        key_server: config.key_server,
    };

    let server_info_string = OpenPGPMessage::new_literal(
        "wicrs_server_info",
        &format!("{}\n", serde_json::to_string_pretty(&server_info_struct)?),
    )
    .sign(&key_pair.secret_key, String::new, HashAlgorithm::SHA2_256)?
    .compress(CompressionAlgorithm::ZIP)?
    .to_armored_string(None)?;
    let server_info = warp::path!("v3" / "info").map(move || {
        warp::http::response::Builder::new()
            .header("Content-Type", "application/json")
            .body(server_info_string.clone())
            .unwrap()
    });

    let routes = graphql_post
        .or(server_info)
        .or(web_socket)
        .or(send_message_init)
        .or(send_message);
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
