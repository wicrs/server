use std::{str::FromStr, sync::Arc};

use crate::{async_server::client_command, ID};
use crate::{async_server::AsyncServer, error::Error};
use crate::{async_server::HubUpdateType, channel};
use actix::Addr;
use futures_util::{SinkExt, StreamExt};
use tokio::{net::TcpStream, sync::Mutex};
use tokio_tungstenite::accept_async;

use crate::error::Result;
use parse_display::{Display, FromStr};

pub use tokio_tungstenite::tungstenite::Message as WebSocketMessage;

/// Messages that can be sent to the server by the client
#[derive(Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum ClientMessage {
    #[display("{}({0},{1},\"{2}\")")]
    SendMessage(ID, ID, String),
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
    Error(Error),
    Success,
    #[display("{}({0})")]
    Id(ID),
}

/// Messages that the server can send to clients.
#[derive(Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum ServerMessage {
    #[display("{}({0})")]
    Error(Error),
    InvalidCommand,
    CommandFailed,
    #[display("{}({0})")]
    CommandSent(u128),
    #[display("{}({0},{1},\"{2}\")")]
    ChatMessage(ID, ID, channel::Message),
    #[display("{}({0},{1})")]
    HubUpdated(ID, HubUpdateType),
    #[display("{}({0})")]
    Result(Response),
    #[display("{}({0},{1},{2})")]
    UserStartedTyping(ID, ID, ID),
    #[display("{}({0},{1},{2})")]
    UserStoppedTyping(ID, ID, ID),
}

pub async fn handle_connection(
    stream: TcpStream,
    user_id: ID,
    addr: Addr<AsyncServer>,
) -> Result {
    let ws_stream = accept_async(stream).await?;
    let (outgoing, mut incoming) = ws_stream.split();
    let out_arc = Arc::new(Mutex::new(outgoing));
    let connection_id: u128;
    {
        let result = addr
            .send(client_command::Connect {
                websocket_writer: out_arc.clone(),
            })
            .await
            .map_err(|_| Error::InternalMessageFailed)?;
        connection_id = result;
    }
    while let Some(msg) = incoming.next().await {
        let msg = msg?;
        if let Ok(text) = msg.to_text() {
            let message = WebSocketMessage::Text(
                if let Ok(command) = ClientMessage::from_str(text) {
                    match command {
                        ClientMessage::SendMessage(hub_id, channel_id, message) => {
                            if let Ok(result) = addr
                                .send(client_command::SendMessage {
                                    user_id: user_id.clone(),
                                    hub_id,
                                    channel_id,
                                    message,
                                })
                                .await
                            {
                                result.map_or_else(
                                    |err| ServerMessage::Error(err),
                                    |id| ServerMessage::Result(Response::Id(id)),
                                )
                            } else {
                                ServerMessage::Error(Error::InternalMessageFailed)
                            }
                        }
                        ClientMessage::SubscribeChannel(hub_id, channel_id) => {
                            if let Ok(result) = addr
                                .send(client_command::SubscribeChannel {
                                    user_id: user_id.clone(),
                                    hub_id,
                                    channel_id,
                                    connection_id,
                                })
                                .await
                            {
                                result.map_or_else(
                                    |err| ServerMessage::Error(err),
                                    |_| ServerMessage::Result(Response::Success),
                                )
                            } else {
                                ServerMessage::Error(Error::InternalMessageFailed)
                            }
                        }
                        ClientMessage::UnsubscribeChannel(hub_id, channel_id) => {
                            if let Ok(_) = addr
                                .send(client_command::UnsubscribeChannel {
                                    hub_id,
                                    channel_id,
                                    connection_id,
                                })
                                .await
                            {
                                ServerMessage::Result(Response::Success)
                            } else {
                                ServerMessage::Error(Error::InternalMessageFailed)
                            }
                        }
                        ClientMessage::StartTyping(hub_id, channel_id) => {
                            if let Ok(result) = addr
                                .send(client_command::StartTyping {
                                    user_id: user_id.clone(),
                                    hub_id,
                                    channel_id,
                                })
                                .await
                            {
                                result.map_or_else(
                                    |err| ServerMessage::Error(err),
                                    |_| ServerMessage::Result(Response::Success),
                                )
                            } else {
                                ServerMessage::Error(Error::InternalMessageFailed)
                            }
                        }
                        ClientMessage::StopTyping(hub_id, channel_id) => {
                            if let Ok(result) = addr
                                .send(client_command::StopTyping {
                                    user_id: user_id.clone(),
                                    hub_id,
                                    channel_id,
                                })
                                .await
                            {
                                result.map_or_else(
                                    |err| ServerMessage::Error(err),
                                    |_| ServerMessage::Result(Response::Success),
                                )
                            } else {
                                ServerMessage::Error(Error::InternalMessageFailed)
                            }
                        }
                        ClientMessage::SubscribeHub(hub_id) => {
                            if let Ok(result) = addr
                                .send(client_command::SubscribeHub {
                                    user_id: user_id.clone(),
                                    hub_id,
                                    connection_id,
                                })
                                .await
                            {
                                result.map_or_else(
                                    |err| ServerMessage::Error(err),
                                    |_| ServerMessage::Result(Response::Success),
                                )
                            } else {
                                ServerMessage::Error(Error::InternalMessageFailed)
                            }
                        }
                        ClientMessage::UnsubscribeHub(hub_id) => {
                            if let Ok(_) = addr
                                .send(client_command::UnsubscribeHub {
                                    hub_id,
                                    connection_id,
                                })
                                .await
                            {
                                ServerMessage::Result(Response::Success)
                            } else {
                                ServerMessage::Error(Error::InternalMessageFailed)
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
    Ok(())
}
