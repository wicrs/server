use std::{str::FromStr, sync::Arc};

use crate::{error::Error, server::Server};
use crate::{server::client_command, ID};
use crate::{server::HubUpdateType, signing::KeyPair};
use futures_util::{SinkExt, StreamExt};
use pgp::Message as OpenPGPMessage;
use pgp::{packet::LiteralData, types::KeyTrait, SignedPublicKey};
use tokio::sync::Mutex;
use warp::ws::WebSocket;
use xactor::Addr;

use crate::error::Result;
use parse_display::{Display, FromStr};

pub use warp::ws::Message as WebSocketMessage;

/// Messages that can be sent to the server by the client
#[derive(Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum ClientMessage {
    #[display("{}({0})")]
    SubscribeHub(ID),
    #[display("{}({0})")]
    UnsubscribeHub(ID),
    #[display("{}({0},{1})")]
    SubscribeChannel(ID, ID),
    #[display("{}({0},{1})")]
    UnsubscribeChannel(ID, ID),
    #[display("{}({0},{1})")]
    StartTyping(ID, ID),
    #[display("{}({0},{1})")]
    StopTyping(ID, ID),
}

/// Possible responses to a [`ClientServerMessage`].
#[derive(Clone, Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum Response {
    #[display("{}({0})")]
    Error(String),
    Success,
    #[display("{}({0})")]
    Id(ID),
}

/// Messages that the server can send to clients.
#[derive(Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum ServerMessage {
    #[display("{}({0})")]
    Error(String),
    InvalidCommand,
    CommandFailed,
    #[display("{}({0})")]
    CommandSent(u128),
    #[display("{}({0},{1},\"{2}\")")]
    ChatMessage(ID, ID, ID),
    #[display("{}({0},{1})")]
    HubUpdated(ID, HubUpdateType),
    #[display("{}({0})")]
    Result(Response),
    #[display("{}({0},{1},{2})")]
    UserStartedTyping(String, ID, ID),
    #[display("{}({0},{1},{2})")]
    UserStoppedTyping(String, ID, ID),
}

pub async fn handle_connection(
    websocket: WebSocket,
    public_key: SignedPublicKey,
    server_keys: Arc<KeyPair>,
    addr: Arc<Addr<Server>>,
) -> Result {
    let (mut outgoing, mut incoming) = websocket.split();
    let key = rand::random::<u128>().to_string();
    let message = OpenPGPMessage::Literal(LiteralData::from_str("auth_key", &key)).sign(
        &server_keys.secret_key,
        String::new,
        pgp::crypto::HashAlgorithm::SHA2_256,
    )?;
    outgoing
        .send(WebSocketMessage::text(message.to_armored_string(None)?))
        .await?;

    if let Some(msg) = incoming.next().await {
        let msg = msg?;
        if let Ok(text) = msg.to_str() {
            let message = crate::signing::verify_message_extract(&public_key, text.to_owned())?.0;
            if message == key {
                drop((message, key, text));
                drop(msg);
                let out_arc = Arc::new(Mutex::new(outgoing));
                let connection_id: u128;
                {
                    let result = addr
                        .call(client_command::Connect {
                            websocket_writer: out_arc.clone(),
                        })
                        .await
                        .map_err(|_| Error::InternalMessageFailed)?;
                    connection_id = result;
                }
                let user_id = hex::encode_upper(public_key.fingerprint());
                let internal_message_error = Error::InternalMessageFailed.to_string();
                while let Some(msg) = incoming.next().await {
                    let msg = msg?;
                    if let Ok(text) = msg.to_str() {
                        let message = WebSocketMessage::text(
                            if let Ok(command) = ClientMessage::from_str(text) {
                                match command {
                                    ClientMessage::SubscribeChannel(hub_id, channel_id) => {
                                        if let Ok(result) = addr
                                            .call(client_command::SubscribeChannel {
                                                user_id: user_id.clone(),
                                                hub_id,
                                                channel_id,
                                                connection_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| ServerMessage::Error(err.to_string()),
                                                |_| ServerMessage::Result(Response::Success),
                                            )
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                    ClientMessage::UnsubscribeChannel(hub_id, channel_id) => {
                                        if addr
                                            .call(client_command::UnsubscribeChannel {
                                                hub_id,
                                                channel_id,
                                                connection_id,
                                            })
                                            .await
                                            .is_ok()
                                        {
                                            ServerMessage::Result(Response::Success)
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                    ClientMessage::StartTyping(hub_id, channel_id) => {
                                        if let Ok(result) = addr
                                            .call(client_command::StartTyping {
                                                user_id: user_id.clone(),
                                                hub_id,
                                                channel_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| ServerMessage::Error(err.to_string()),
                                                |_| ServerMessage::Result(Response::Success),
                                            )
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                    ClientMessage::StopTyping(hub_id, channel_id) => {
                                        if let Ok(result) = addr
                                            .call(client_command::StopTyping {
                                                user_id: user_id.clone(),
                                                hub_id,
                                                channel_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| ServerMessage::Error(err.to_string()),
                                                |_| ServerMessage::Result(Response::Success),
                                            )
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                    ClientMessage::SubscribeHub(hub_id) => {
                                        if let Ok(result) = addr
                                            .call(client_command::SubscribeHub {
                                                user_id: user_id.clone(),
                                                hub_id,
                                                connection_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| ServerMessage::Error(err.to_string()),
                                                |_| ServerMessage::Result(Response::Success),
                                            )
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                    ClientMessage::UnsubscribeHub(hub_id) => {
                                        if addr
                                            .call(client_command::UnsubscribeHub {
                                                hub_id,
                                                connection_id,
                                            })
                                            .await
                                            .is_ok()
                                        {
                                            ServerMessage::Result(Response::Success)
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                }
                            } else {
                                ServerMessage::InvalidCommand
                            }
                            .to_string(),
                        );
                        out_arc.lock().await.send(message).await?;
                    }
                }
                return Ok(());
            }
        }
    }
    Err(Error::WsNotAuthenticated)
}
