use std::sync::Arc;

use crate::{
    channel::Message,
    error::Error,
    hub::Hub,
    permission::ChannelPermission,
    server::{Server, ServerNotification},
};
use crate::{server::client_command, ID};
use crate::{server::HubUpdateType, signing::KeyPair};
use futures_util::{SinkExt, StreamExt};
use pgp::{crypto::HashAlgorithm, types::CompressionAlgorithm, Message as OpenPGPMessage};
use pgp::{packet::LiteralData, types::KeyTrait, SignedPublicKey};
use tokio::sync::Mutex;
use warp::ws::WebSocket;
use xactor::Addr;

use crate::error::Result;
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
    SendMessageInit {
        hub_id: ID,
        channel_id: ID,
        content: String,
    },
    SendMessage {
        signed_message: String,
    },
}

/// Messages that the server can send to clients.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerMessage {
    Error(String),
    InvalidCommand,
    NotSigned,
    CommandFailed,
    ChatMessage {
        hub_id: ID,
        channel_id: ID,
        message_id: ID,
        armoured_message: String,
    },
    HubUpdated {
        hub_id: ID,
        update_type: HubUpdateType,
    },
    Success,
    UserStartedTyping {
        user_id: String,
        hub_id: ID,
        channel_id: ID,
    },
    UserStoppedTyping {
        user_id: String,
        hub_id: ID,
        channel_id: ID,
    },
    MessageForSigning {
        server_signed_message: String,
    },
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
        HashAlgorithm::SHA2_256,
    )?;
    outgoing
        .send(WebSocketMessage::text(message.to_armored_string(None)?))
        .await?;

    if let Some(msg) = incoming.next().await {
        let msg = msg?;
        if let Ok(text) = msg.to_str() {
            let message = crate::signing::verify_message_extract(&public_key, text)?.0;
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
                        let raw_response = if let Ok((command_text, _)) =
                            crate::signing::verify_message_extract(&public_key, text)
                        {
                            if let Ok(command) = serde_json::from_str(&command_text) {
                                match command {
                                    ClientMessage::SubscribeChannel { hub_id, channel_id } => {
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
                                                |_| ServerMessage::Success,
                                            )
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
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
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                    ClientMessage::StartTyping { hub_id, channel_id } => {
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
                                                |_| ServerMessage::Success,
                                            )
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                    ClientMessage::StopTyping { hub_id, channel_id } => {
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
                                                |_| ServerMessage::Success,
                                            )
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                    ClientMessage::SubscribeHub { hub_id } => {
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
                                                |_| ServerMessage::Success,
                                            )
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
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
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                    ClientMessage::SendMessageInit {
                                        hub_id,
                                        channel_id,
                                        content,
                                    } => {
                                        let hub = Hub::load(hub_id).await?;
                                        let member = hub.get_member(&user_id)?;
                                        crate::check_permission!(
                                            &member,
                                            channel_id,
                                            ChannelPermission::Write,
                                            &hub
                                        );
                                        ServerMessage::MessageForSigning {
                                            server_signed_message: Message::new(
                                                user_id.clone(),
                                                content,
                                                hub_id,
                                                channel_id,
                                            )
                                            .sign(&server_keys.secret_key, String::new)?
                                            .compress(CompressionAlgorithm::ZIP)?
                                            .to_armored_string(None)?,
                                        }
                                    }
                                    ClientMessage::SendMessage { signed_message } => {
                                        let message = Message::from_double_signed_verify(
                                            &signed_message,
                                            &server_keys.public_key,
                                            &public_key,
                                        )?;
                                        if let Err(err) = crate::channel::Channel::write_message(
                                            message.hub_id,
                                            message.channel_id,
                                            crate::channel::SignedMessage::new(
                                                message.id,
                                                message.created,
                                                signed_message.clone(),
                                            ),
                                        )
                                        .await
                                        {
                                            ServerMessage::Error(err.to_string())
                                        } else if addr
                                            .call(ServerNotification::NewMessage(
                                                message.hub_id,
                                                message.channel_id,
                                                message.id,
                                                signed_message,
                                                message,
                                            ))
                                            .await
                                            .is_ok()
                                        {
                                            ServerMessage::Success
                                        } else {
                                            ServerMessage::Error(internal_message_error.clone())
                                        }
                                    }
                                }
                            } else {
                                ServerMessage::InvalidCommand
                            }
                        } else {
                            ServerMessage::NotSigned
                        };
                        let message = OpenPGPMessage::new_literal(
                            "",
                            serde_json::to_string(&raw_response)?.as_str(),
                        )
                        .sign(
                            &server_keys.secret_key,
                            String::new,
                            HashAlgorithm::SHA2_256,
                        )?
                        .compress(CompressionAlgorithm::ZIP)?;
                        out_arc
                            .lock()
                            .await
                            .send(WebSocketMessage::text(message.to_armored_string(None)?))
                            .await?;
                    }
                }
                return Ok(());
            }
        }
    }
    Err(Error::WsNotAuthenticated)
}
