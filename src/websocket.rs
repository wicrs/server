use std::{
    str::FromStr,
    time::{Duration, Instant},
};

use crate::{
    channel,
    server::{ClientCommand, ClientServerMessage, Response, Server, ServerMessage, ServerResponse},
    Error, ID,
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
    #[display("{}({0},{1})")]
    StartTyping(ID, ID),
    #[display("{}({0},{1})")]
    StopTyping(ID, ID),
}

#[derive(Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum ServerClientMessage {
    #[display("{}({0})")]
    Error(Error),
    InvalidCommand,
    CommandFailed,
    #[display("{}({0})")]
    CommandSent(u128),
    #[display("{}({0},{1},\"{2}\")")]
    ChatMessage(ID, ID, channel::Message),
    #[display("{}({0})")]
    HubUpdated(ID),
    #[display("{}({0}{1})")]
    Result(u128, Response),
    #[display("{}({0},{1},{2})")]
    UserStartedTyping(ID, ID, ID),
    #[display("{}({0},{1},{2})")]
    UserStoppedTyping(ID, ID, ID),
}

impl From<ServerMessage> for ServerClientMessage {
    fn from(message: ServerMessage) -> Self {
        match message {
            ServerMessage::NewMessage(hub_id, channel_id, message) => {
                Self::ChatMessage(hub_id, channel_id, message)
            }
            ServerMessage::TypingStart(hub_id, channel_id, user_id) => {
                Self::UserStartedTyping(hub_id, channel_id, user_id)
            }
            ServerMessage::TypingStop(hub_id, channel_id, user_id) => {
                Self::UserStoppedTyping(hub_id, channel_id, user_id)
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
            .send(ClientCommand::Connect(self.user.clone(), addr.recipient()).into())
            .into_actor(self)
            .then(|_, _, _| fut::ready(()))
            .wait(ctx);
    }
}

impl Handler<ServerResponse> for ChatSocket {
    type Result = ();

    fn handle(&mut self, msg: ServerResponse, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(ServerClientMessage::Result(msg.responding_to, msg.message).to_string());
    }
}

impl Handler<ServerMessage> for ChatSocket {
    type Result = ();

    fn handle(&mut self, msg: ServerMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(ServerClientMessage::from(msg).to_string())
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
                if let Ok(command) = ClientMessage::from_str(&text) {
                    match command {
                        ClientMessage::SendMessage(hub, channel, message) => {
                            let id = rand::random();
                            let message = ClientServerMessage {
                                client_addr: Some(ctx.address().recipient()),
                                message_id: id,
                                command: ClientCommand::SendMessage(
                                    self.user.clone(),
                                    hub,
                                    channel,
                                    message,
                                ),
                            };
                            if let Ok(_) = self.addr.try_send(message) {
                                ctx.text(ServerClientMessage::CommandSent(id).to_string());
                            } else {
                                ctx.text(ServerClientMessage::CommandFailed.to_string());
                            }
                        }
                        ClientMessage::Subscribe(hub, channel) => {
                            let id = rand::random();
                            let message = ClientServerMessage {
                                client_addr: Some(ctx.address().recipient()),
                                message_id: id,
                                command: ClientCommand::Subscribe(self.user.clone(), hub, channel),
                            };
                            if let Ok(_) = self.addr.try_send(message) {
                                ctx.text(ServerClientMessage::CommandSent(id).to_string());
                            } else {
                                ctx.text(ServerClientMessage::CommandFailed.to_string());
                            }
                        }
                        ClientMessage::Unsubscribe(hub, channel) => {
                            if let Ok(_) = self.addr.try_send(
                                ClientCommand::Unsubscribe(self.user.clone(), hub, channel).into(),
                            ) {
                                ctx.text(ServerClientMessage::CommandSent(0).to_string());
                            } else {
                                ctx.text(ServerClientMessage::CommandFailed.to_string());
                            }
                        }
                        ClientMessage::StartTyping(hub, channel) => {
                            let id = rand::random();
                            let message = ClientServerMessage {
                                client_addr: Some(ctx.address().recipient()),
                                message_id: id,
                                command: ClientCommand::StartTyping(
                                    self.user.clone(),
                                    hub,
                                    channel,
                                ),
                            };
                            if let Ok(_) = self.addr.try_send(message) {
                                ctx.text(ServerClientMessage::CommandSent(id).to_string());
                            } else {
                                ctx.text(ServerClientMessage::CommandFailed.to_string());
                            }
                        }
                        ClientMessage::StopTyping(hub, channel) => {
                            if let Ok(_) = self.addr.try_send(
                                ClientCommand::StopTyping(self.user.clone(), hub, channel).into(),
                            ) {
                                ctx.text(ServerClientMessage::CommandSent(0).to_string());
                            } else {
                                ctx.text(ServerClientMessage::CommandFailed.to_string());
                            }
                        }
                    }
                } else {
                    ctx.text(ServerClientMessage::InvalidCommand.to_string());
                }
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                self.addr
                    .send(
                        ClientCommand::Disconnect(self.user.clone(), ctx.address().recipient())
                            .into(),
                    )
                    .into_actor(self)
                    .then(|_, _, _| fut::ready(()))
                    .wait(ctx);
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}
