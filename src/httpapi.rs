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

use pgp::packet::LiteralData;
use pgp::Message as OpenPGPMessage;
use pgp::SignedPublicKey;
use pgp::{crypto::HashAlgorithm, types::CompressionAlgorithm};

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
    let send_message_server_arc = server.clone();
    let key_pair_send = key_pair.clone();
    let key_pair_send_init = key_pair.clone();
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
            },
        );

    let send_message_init = warp::any()
        .map(move || key_pair_send_init.clone())
        .and(warp::path!("v2" / "send_message_init" / String / String))
        .and(signed_body)
        .and_then(
            |key_pair: KeyPairArc,
             hub_id: String,
             channel_id: String,
             (content, sender): (String, String)| async move {
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
            },
        );

    let send_message = warp::any()
        .map(move || (key_pair_send.clone(), send_message_server_arc.clone()))
        .and(warp::path!("v2" / "send_message"))
        .and(public_key_filter)
        .and(warp::body::bytes())
        .and_then(
            |(key_pair, server): (KeyPairArc, ServerAddr),
             client_public_key: SignedPublicKey,
             body: Bytes| async move {
                Ok::<_, Infallible>(
                    async {
                        let body = String::from_utf8(body.to_vec())?;
                        let message = Message::from_double_signed_verify(
                            &body,
                            &key_pair.public_key,
                            &client_public_key,
                        )?;
                        let _ = server.send(ServerNotification::NewMessage(message));
                        Ok::<_, Error>(warp::reply())
                    }
                    .await
                    .map_or_else(|e| e.into_response(), |r| r.into_response()),
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

    let routes = graphql_post
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
