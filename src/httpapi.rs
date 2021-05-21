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

use pgp::crypto::HashAlgorithm;
use pgp::types::{CompressionAlgorithm, KeyTrait, SecretKeyTrait};
use pgp::Message as OpenPGPMessage;
use pgp::SignedPublicKey;

use crate::server::Server;
use crate::signing::KeyPair;
use crate::signing::{PUBLIC_KEY_PATH, SECRET_KEY_PATH};
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
    let key_pair = if let Ok(key_pair) = KeyPair::load(SECRET_KEY_PATH, PUBLIC_KEY_PATH).await {
        key_pair
    } else {
        warn!(
            "Failed to load secret key from {}, generating a new one.",
            SECRET_KEY_PATH
        );
        let key_id = if let Some(key_id) = config.key_id.clone() {
            key_id
        } else {
            println!(
                "Enter an ID for the server's PGP key (e.g. WICRS Server <wicrs@examle.com>):"
            );
            let mut buf = String::new();
            std::io::stdin().read_line(&mut buf)?;
            buf
        };
        info!("Generating new PGP key pair for the server...");
        let key_pair = KeyPair::new(key_id)?;
        key_pair.save(SECRET_KEY_PATH, PUBLIC_KEY_PATH).await?;
        info!(
            "Server key pair generated, private key saved to {}, public key saved to {}.",
            SECRET_KEY_PATH, PUBLIC_KEY_PATH
        );
        key_pair
    };
    let key_pair = Arc::new(key_pair);
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
        warn!("Unable to upload public key to key server.");
    }
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
                    crate::signing::get_or_import_public_key(
                        &hex::decode(header).map_err(|_| Error::InvalidFingerprint)?,
                        &key_server_url,
                    )
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

                        let mut response =
                            create_response(resp.data.to_string().as_str(), &key_pair.secret_key)?;
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
                            let msg = Message::new(sender, content, hub_id, channel_id);
                            create_response(&serde_json::to_string(&msg)?, &key_pair.secret_key)
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
                        let response = create_response(
                            &serde_json::to_string(&message)?,
                            &key_pair.secret_key,
                        );
                        let _ = server.send(ServerNotification::NewMessage(
                            message.hub_id,
                            message.channel_id,
                            message.id,
                            body,
                            message,
                        ));
                        response
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
    let server_info_str = serde_json::to_string(&server_info_struct).unwrap();

    let server_info = warp::path!("v3" / "info").map(move || {
        create_response(server_info_str.clone().as_str(), &key_pair.secret_key)
            .map_or_else(|e| e.into_response(), |r| r.into_response())
    });

    let cors = warp::cors().allow_any_origin();
    let log = warp::log("wicrs_server::http");

    let routes = graphql_post
        .or(server_info)
        .or(web_socket)
        .or(send_message_init)
        .or(send_message)
        .with(cors)
        .with(log);
    let server = warp::serve(routes).run(
        config
            .address
            .parse::<SocketAddr>()
            .expect("Invalid bind address"),
    );

    server.await;

    Ok(())
}

fn create_response(response: &str, key: &impl SecretKeyTrait) -> Result<HttpResponse<String>> {
    let msg = OpenPGPMessage::new_literal("", response)
        .sign(key, || String::with_capacity(0), HashAlgorithm::SHA2_256)?
        .compress(CompressionAlgorithm::ZIP)?
        .to_armored_string(None)?;

    // No proper multipart support offered, so we will hack it ourselves
    // Note: will break if "response" contains text "the=boundary"
    // Note: spec requires crlf
    let boundary = "the=boundary";
    let body = format!(
        "--{0}\r\n\
        Content-Type: application/pgp-encrypted\r\n\
        Version: 1\r\n\
        --{0}\r\n\
        Content-Type: application/octet-stream\r\n\
        {1}\r\n\
        --{0}--",
        boundary, msg
    );

    Ok(warp::http::response::Builder::new()
        .header(
            "Content-Type",
            format!(
                "multipart/encrypted; \
                protocol=\"application/pgp-encrypted\"; \
                boundary={0}",
                boundary
            ),
        )
        .body(body)?)
}
