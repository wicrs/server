use std::sync::Arc;

use crate::server::HubUpdateType;
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

use serde::{Deserialize, Serialize};

pub use warp::ws::Message as WebSocketMessage;

/// Messages that can be sent to the server by the client
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClientMessage {
    SubscribeHub {
        hub_id: ID,
    },
    UnsubscribeHub {
        hub_id: ID,
    },
    SubscribeChannel {
        hub_id: ID,
        channel_id: ID,
    },
    UnsubscribeChannel {
        hub_id: ID,
        channel_id: ID,
    },
    StartTyping {
        hub_id: ID,
        channel_id: ID,
    },
    StopTyping {
        hub_id: ID,
        channel_id: ID,
    },
    SendMessage {
        hub_id: ID,
        channel_id: ID,
        message: String,
    },
}

/// Messages that the server can send to clients.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerMessage {
    Error(ApiError),
    InvalidCommand,
    NotSigned,
    CommandFailed,
    ChatMessage {
        hub_id: ID,
        channel_id: ID,
        message_id: ID,
        message: String,
    },
    HubUpdated {
        hub_id: ID,
        update_type: HubUpdateType,
    },
    Success,
    UserStartedTyping {
        user_id: ID,
        hub_id: ID,
        channel_id: ID,
    },
    UserStoppedTyping {
        user_id: ID,
        hub_id: ID,
        channel_id: ID,
    },
}

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
                                    ClientMessage::SubscribeChannel { hub_id, channel_id } => {
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
                                                |err| ServerMessage::Error(err.into()),
                                                |_| ServerMessage::Success,
                                            )
                                        } else {
                                            ServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    ClientMessage::UnsubscribeChannel { hub_id, channel_id } => {
                                        if addr
                                            .call(client_command::UnsubscribeChannel {
                                                hub_id,
                                                channel_id,
                                                connection_id,
                                            })
                                            .await
                                            .is_ok()
                                        {
                                            ServerMessage::Success
                                        } else {
                                            ServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    ClientMessage::StartTyping { hub_id, channel_id } => {
                                        if let Ok(result) = addr
                                            .call(client_command::StartTyping {
                                                user_id,
                                                hub_id,
                                                channel_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| ServerMessage::Error(err.into()),
                                                |_| ServerMessage::Success,
                                            )
                                        } else {
                                            ServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    ClientMessage::StopTyping { hub_id, channel_id } => {
                                        if let Ok(result) = addr
                                            .call(client_command::StopTyping {
                                                user_id,
                                                hub_id,
                                                channel_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| ServerMessage::Error(err.into()),
                                                |_| ServerMessage::Success,
                                            )
                                        } else {
                                            ServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    ClientMessage::SubscribeHub { hub_id } => {
                                        if let Ok(result) = addr
                                            .call(client_command::SubscribeHub {
                                                user_id,
                                                hub_id,
                                                connection_id,
                                            })
                                            .await
                                        {
                                            result.map_or_else(
                                                |err| ServerMessage::Error(err.into()),
                                                |_| ServerMessage::Success,
                                            )
                                        } else {
                                            ServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    ClientMessage::UnsubscribeHub { hub_id } => {
                                        if addr
                                            .call(client_command::UnsubscribeHub {
                                                hub_id,
                                                connection_id,
                                            })
                                            .await
                                            .is_ok()
                                        {
                                            ServerMessage::Success
                                        } else {
                                            ServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                    ClientMessage::SendMessage {
                                        message,
                                        hub_id,
                                        channel_id,
                                    } => {
                                        let message =
                                            Message::new(user_id, message, hub_id, channel_id);
                                        if let Err(err) =
                                            crate::channel::Channel::write_message(message.clone())
                                                .await
                                        {
                                            ServerMessage::Error(err.into())
                                        } else if addr
                                            .call(ServerNotification::NewMessage(message))
                                            .await
                                            .is_ok()
                                        {
                                            ServerMessage::Success
                                        } else {
                                            ServerMessage::Error(ApiError::InternalError)
                                        }
                                    }
                                }
                            } else {
                                ServerMessage::InvalidCommand
                            };
                            out_arc
                                .lock()
                                .await
                                .send(WebSocketMessage::text(serde_json::to_string(
                                    &raw_response,
                                )?))
                                .await?;
                        }
                    }
                    return Ok(());
                }
            }
        }
    }
    Err(ApiError::WsNotAuthenticated.into())
}
