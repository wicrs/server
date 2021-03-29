use std::{
    str::FromStr,
    time::{Duration, Instant},
};

use crate::{ApiError, ID, channel, server::{self, ClientMessage, Connect, Server, Subscribe, Unsubscribe}};
use actix::{
    fut, Actor, ActorContext, ActorFuture, Addr, AsyncContext, ContextFutureSpawner, Handler,
    Message, StreamHandler, WrapFuture,
};
use actix_web_actors::ws;
use parse_display::{Display, FromStr};

#[derive(Message, Display, FromStr)]
#[display(style = "SNAKE_CASE")]
#[rtype(result = "()")]
pub enum ClientCommand {
    #[display("{}({1}:{2},\"{0}\")")]
    SendMessage(String, ID, ID),
    #[display("{}({0}:{1})")]
    Subscribe(ID, ID),
    #[display("{}({0}:{1})")]
    Unsubscribe(ID, ID),
    StartTyping,
    StopTyping,
}

#[derive(Message, Display, FromStr)]
#[display(style = "SNAKE_CASE")]
#[rtype(result = "()")]
pub enum ServerCommand {
    #[display("{}({0})")]
    Error(ApiError),
    InvalidCommand,
    #[display("{}({0}:{1},\"{2}\")")]
    ChatMessage(ID, ID, channel::Message),
    #[display("{}({0})")]
    HubUpdated(ID),
    #[display("{}({0})")]
    UserTyping(ID),
    #[display("{}({0})")]
    UserStopTyping(ID),
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

impl Handler<server::Message> for ChatSocket {
    type Result = ();

    fn handle(&mut self, msg: server::Message, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(ServerCommand::ChatMessage(msg.hub_id, msg.channel_id, msg.message).to_string())
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
                if let Ok(command) = ClientCommand::from_str(&text) {
                    match command {
                        ClientCommand::SendMessage(message, hub, channel) => {
                            self.addr.do_send(ClientMessage {
                                user_id: self.user.clone(),
                                message: message,
                                hub_id: hub,
                                channel_id: channel,
                            })
                        }
                        ClientCommand::Subscribe(hub, channel) => {
                            self.addr.do_send(Subscribe {
                                user_id: self.user.clone(),
                                hub_id: hub,
                                channel_id: channel,
                            });
                        }
                        ClientCommand::Unsubscribe(hub, channel) => {
                            self.addr.do_send(Unsubscribe {
                                user_id: self.user.clone(),
                                hub_id: hub,
                                channel_id: channel,
                            });
                        }
                        ClientCommand::StartTyping => {}
                        ClientCommand::StopTyping => {}
                    }
                } else {
                    ctx.text(ServerCommand::InvalidCommand.to_string());
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
