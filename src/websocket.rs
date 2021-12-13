use std::sync::Arc;

use crate::{
    channel::Message,
    error::{ApiError, Error, Result},
    server::{Server, ServerNotification},
};
use crate::{server::client_command, ID};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::Mutex;
use warp::ws::WebSocket;
use xactor::Addr;

use warp::ws::Message as WebSocketMessage;

use crate::prelude::{WsClientMessage, WsServerMessage};

pub mod prelude {}

pub async fn handle_connection(
    websocket: WebSocket,
    init_user_id: ID,
    addr: Arc<Addr<Server>>,
) -> Result {
    let (outgoing, mut incoming) = websocket.split();
    if let Some(msg) = incoming.next().await {
        if let Ok(text) = msg?.to_str() {
            if let Ok(user_id) = ID::parse_str(text) {
                if init_user_id == user_id {
                    let out_arc = Arc::new(Mutex::new(outgoing));
                    let connection_id: u128;
                    {
                        let result = addr
                            .call(client_command::Connect {
                                websocket_writer: out_arc.clone(),
                            })
                            .await
                            .map_err(|_| Error::ApiError(ApiError::InternalError))?;
                        connection_id = result;
                    }
                    while let Some(msg) = incoming.next().await {
                        let msg = msg?;
                        if let Ok(text) = msg.to_str() {
                            let raw_response = if let Ok(command) = serde_json::from_str(text) {
                                match command {
                                    WsClientMessage::SubscribeChannel { hub_id, channel_id } => {
                                        if let Ok(result) = addr
                                            .call(client_command::SubscribeChannel {
                                                user_id,
                                                hub_id,
                                                channel_id,
                                                connection_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| WsServerMessage::Error((&err).into()),
                                                |_| WsServerMessage::Success,
                                            )
                                        } else {
                                            WsServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    WsClientMessage::UnsubscribeChannel { hub_id, channel_id } => {
                                        if addr
                                            .call(client_command::UnsubscribeChannel {
                                                hub_id,
                                                channel_id,
                                                connection_id,
                                            })
                                            .await
                                            .is_ok()
                                        {
                                            WsServerMessage::Success
                                        } else {
                                            WsServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    WsClientMessage::StartTyping { hub_id, channel_id } => {
                                        if let Ok(result) = addr
                                            .call(client_command::StartTyping {
                                                user_id,
                                                hub_id,
                                                channel_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| WsServerMessage::Error((&err).into()),
                                                |_| WsServerMessage::Success,
                                            )
                                        } else {
                                            WsServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    WsClientMessage::StopTyping { hub_id, channel_id } => {
                                        if let Ok(result) = addr
                                            .call(client_command::StopTyping {
                                                user_id,
                                                hub_id,
                                                channel_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| WsServerMessage::Error((&err).into()),
                                                |_| WsServerMessage::Success,
                                            )
                                        } else {
                                            WsServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    WsClientMessage::SubscribeHub { hub_id } => {
                                        if let Ok(result) = addr
                                            .call(client_command::SubscribeHub {
                                                user_id,
                                                hub_id,
                                                connection_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| WsServerMessage::Error((&err).into()),
                                                |_| WsServerMessage::Success,
                                            )
                                        } else {
                                            WsServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    WsClientMessage::UnsubscribeHub { hub_id } => {
                                        if addr
                                            .call(client_command::UnsubscribeHub {
                                                hub_id,
                                                connection_id,
                                            })
                                            .await
                                            .is_ok()
                                        {
                                            WsServerMessage::Success
                                        } else {
                                            WsServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    WsClientMessage::SendMessage {
                                        message,
                                        hub_id,
                                        channel_id,
                                    } => {
                                        let message =
                                            Message::new(user_id, message, hub_id, channel_id);
                                        if let Err(err) =
                                            crate::channel::Channel::write_message(&message).await
                                        {
                                            WsServerMessage::Error((&err).into())
                                        } else if addr
                                            .call(ServerNotification::NewMessage(message))
                                            .await
                                            .is_ok()
                                        {
                                            WsServerMessage::Success
                                        } else {
                                            println!("fail here");
                                            WsServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                }
                            } else {
                                println!("this is the fail");
                                WsServerMessage::InvalidCommand
                            };
                            let mut lock = out_arc.lock().await;
                            lock.send(WebSocketMessage::text(serde_json::to_string(
                                &raw_response,
                            )?))
                            .await?;
                            lock.flush().await?;
                        }
                    }
                    return Ok(());
                }
            }
        }
    }
    Err(ApiError::WsNotAuthenticated.into())
}
