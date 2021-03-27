use std::{fmt::Display, str::FromStr, time::Instant};

use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web_actors::ws;
use wicrs_server::{auth::Auth, hub::Hub, ID};

use crate::{CLIENT_TIMEOUT, HEARTBEAT_INTERVAL};

pub enum Commands {
    Authenticate(ID, String),
    SendMessage(String),
    SelectLocation(ID, ID),
}

impl Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Commands::Authenticate(id, token) => f.write_fmt(format_args!("aut {}:{}", id, token)),
            Commands::SendMessage(message) => f.write_fmt(format_args!("msg {}", message)),
            Commands::SelectLocation(hub, channel) => {
                f.write_fmt(format_args!("sel {}:{}", hub, channel))
            }
        }
    }
}

impl FromStr for Commands {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.get(0..3) {
            Some("aut") => {
                if let Some(id_token) = s.strip_prefix("aut ") {
                    let mut split  = id_token.split(':');
                    if let (Some(id), Some(token)) = (split.next(), split.next()) {
                        Ok(Self::Authenticate(
                            ID::from_str(id).map_err(|_| ())?,
                            token.to_string(),
                        ))
                    } else {
                        Err(())
                    }
                } else {
                    Err(())
                }
            }
            Some("msg") => {
                if let Some(message) = s.strip_prefix("msg ") {
                    Ok(Self::SendMessage(message.to_string()))
                } else {
                    Err(())
                }
            }
            Some("sel") => {
                if let Some(hub_channel) = s.strip_prefix("sel ") {
                    let mut split  = hub_channel.split(':');
                    if let (Some(hub), Some(channel)) = (split.next(), split.next()) {
                        Ok(Self::SelectLocation(
                            ID::from_str(hub).map_err(|_| ())?,
                            ID::from_str(channel).map_err(|_| ())?,
                        ))
                    } else {
                        Err(())
                    }
                } else {
                    Err(())
                }
            }
            _ => Err(()),
        }
    }
}

pub struct ChatSocket {
    hb: Instant,
    user: Option<ID>,
    send_loc: Option<(ID, ID)>,
}

impl Actor for ChatSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
    }
}

impl ChatSocket {
    pub fn new() -> Self {
        Self {
            hb: Instant::now(),
            send_loc: None,
            user: None,
        }
    }
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL.clone(), |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT.clone() {
                println!("Websocket Client heartbeat failed, disconnecting!");
                ctx.stop();
                return;
            }

            ctx.ping(b"");
        });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ChatSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                if let Ok(command) = Commands::from_str(&text) {
                    match command {
                        Commands::Authenticate(id, token) => {
                            if self.user.is_some() {
                                ctx.text("ALREADY_AUTHENTICATED");
                            } else {
                                if futures::executor::block_on(Auth::is_authenticated(
                                    crate::AUTH.clone(),
                                    id.clone(),
                                    token,
                                )) {
                                    self.user = Some(id);
                                    ctx.text("AUTH_SUCCESS");
                                } else {
                                    ctx.text("AUTH_FAILED");
                                    ctx.close(None)
                                }
                            }
                        }
                        Commands::SendMessage(message) => {
                            if let Some(user) = &self.user {
                                if let Some((hub, channel)) = &self.send_loc {
                                    if message.as_bytes().len() < wicrs_server::MESSAGE_MAX_SIZE {
                                        futures::executor::block_on(async move {
                                            if let Ok(mut hub) = Hub::load(hub).await {
                                                if let Err(err) =
                                                    hub.send_message(user, channel, message).await
                                                {
                                                    ctx.text(serde_json::json!(err).to_string());
                                                } else {
                                                    ctx.text("SEND_SUCCESS");
                                                }
                                            } else {
                                                ctx.text("HUB_NOT_FOUND");
                                            }
                                        })
                                    } else {
                                        ctx.text("MESSAGE_TOO_BIG");
                                    }
                                } else {
                                    ctx.text("LOCATION_NOT_SELECTED");
                                }
                            } else {
                                ctx.text("AUTHENTICATION_REQUIRED");
                            }
                        }
                        Commands::SelectLocation(hub, channel) => {
                            self.send_loc = Some((hub, channel));
                            ctx.text("SELECTION_UPDATED");
                        }
                    }
                } else {
                    ctx.text("INVALID_COMMAND");
                }
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}
