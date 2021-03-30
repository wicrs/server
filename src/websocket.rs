use std::{
    str::FromStr,
    time::{Duration, Instant},
};

use crate::{
    channel,
    server::{Connect, SendMessage, Server, ServerClientMessage, Subscribe, Unsubscribe},
    ApiError, ID,
};
use actix::{
    fut, Actor, ActorContext, ActorFuture, Addr, AsyncContext, ContextFutureSpawner, Handler,
    StreamHandler, WrapFuture,
};
use actix_web_actors::ws;
use parse_display::{Display, FromStr};

#[derive(Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum ClientMessage {
    #[display("{}({0},{1},\"{2}\")")]
    SendMessage(ID, ID, String),
    #[display("{}({0},{1})")]
    Subscribe(ID, ID),
    #[display("{}({0},{1})")]
    Unsubscribe(ID, ID),
    StartTyping,
    StopTyping,
}

#[derive(Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum ServerMessage {
    #[display("{}({0})")]
    Error(ApiError),
    InvalidCommand,
    #[display("{}({0},{1},\"{2}\")")]
    ChatMessage(ID, ID, channel::Message),
    #[display("{}({0})")]
    HubUpdated(ID),
    #[display("{}({0},{1},{2})")]
    UserStartedTyping(ID, ID, ID),
    #[display("{}({0},{1},{2})")]
    UserStoppedTyping(ID, ID, ID),
}

impl From<ServerClientMessage> for ServerMessage {
    fn from(message: ServerClientMessage) -> Self {
        match message {
            ServerClientMessage::NewMessage(hub_id, channel_id, message) => {
                ServerMessage::ChatMessage(hub_id, channel_id, message)
            }
            ServerClientMessage::TypingStart(hub_id, channel_id, user_id) => {
                ServerMessage::UserStartedTyping(hub_id, channel_id, user_id)
            }
            ServerClientMessage::TypingStop(hub_id, channel_id, user_id) => {
                ServerMessage::UserStoppedTyping(hub_id, channel_id, user_id)
            }
        }
    }
}

pub struct ChatSocket {
    hb: Instant,
    user: ID,
    addr: Addr<Server>,
    hb_interval: Duration,
    hb_timeout: Duration,
}

impl Actor for ChatSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        let addr = ctx.address();
        self.addr
            .send(Connect {
                addr: addr.recipient(),
                user_id: self.user.clone(),
            })
            .into_actor(self)
            .then(|_, _, _| fut::ready(()))
            .wait(ctx);
    }
}

impl Handler<ServerClientMessage> for ChatSocket {
    type Result = ();

    fn handle(&mut self, msg: ServerClientMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(ServerMessage::from(msg).to_string())
    }
}

impl ChatSocket {
    pub fn new(user: ID, hb_interval: Duration, hb_timeout: Duration, addr: Addr<Server>) -> Self {
        Self {
            hb: Instant::now(),
            user,
            hb_interval,
            hb_timeout,
            addr,
        }
    }
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        let timeout = self.hb_timeout.clone();
        ctx.run_interval(self.hb_interval.clone(), move |act, ctx| {
            if Instant::now().duration_since(act.hb) > timeout {
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
                println!("WS TEXT: {}", text);
                if let Ok(command) = ClientMessage::from_str(&text) {
                    match command {
                        ClientMessage::SendMessage(hub, channel, message) => {
                            self.addr.do_send(SendMessage {
                                user_id: self.user.clone(),
                                message: message,
                                hub_id: hub,
                                channel_id: channel,
                            })
                        }
                        ClientMessage::Subscribe(hub, channel) => {
                            self.addr.do_send(Subscribe {
                                user_id: self.user.clone(),
                                hub_id: hub,
                                channel_id: channel,
                            });
                        }
                        ClientMessage::Unsubscribe(hub, channel) => {
                            self.addr.do_send(Unsubscribe {
                                user_id: self.user.clone(),
                                hub_id: hub,
                                channel_id: channel,
                            });
                        }
                        ClientMessage::StartTyping => {}
                        ClientMessage::StopTyping => {}
                    }
                } else {
                    ctx.text(ServerMessage::InvalidCommand.to_string());
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
