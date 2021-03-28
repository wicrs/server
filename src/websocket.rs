use std::{
    fmt::Display,
    str::FromStr,
    time::{Duration, Instant},
};

use crate::{
    server::{self, ClientMessage, Connect, Server, Subscribe, Unsubscribe},
    ID,
};
use actix::{
    fut, Actor, ActorContext, ActorFuture, Addr, AsyncContext, ContextFutureSpawner, Handler,
    Message, StreamHandler, WrapFuture,
};
use actix_web_actors::ws;

#[derive(Message)]
#[rtype(result = "()")]
pub enum ClientCommand {
    SendMessage(String, ID, ID),
    Subscribe(ID, ID),
    Unsubscribe(ID, ID),
}

impl Display for ClientCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientCommand::SendMessage(message, hub, channel) => {
                f.write_fmt(format_args!("msg {}:{} {}", hub, channel, message))
            }
            ClientCommand::Subscribe(hub, channel) => {
                f.write_fmt(format_args!("sub {}:{}", hub, channel))
            }
            ClientCommand::Unsubscribe(hub, channel) => {
                f.write_fmt(format_args!("uns {}:{}", hub, channel))
            }
        }
    }
}

impl FromStr for ClientCommand {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.get(0..3) {
            Some("msg") => {
                if let Some(message) = s.strip_prefix("msg ") {
                    let mut split_spc = message.splitn(2, ' ');
                    if let (Some(loc), Some(msg)) = (split_spc.next(), split_spc.next()) {
                        let mut split = loc.split(':');
                        if let (Some(hub), Some(channel)) = (split.next(), split.next()) {
                            if let (Ok(hub_id), Ok(channel_id)) =
                                (ID::parse_str(hub), ID::parse_str(channel))
                            {
                                Ok(Self::SendMessage(msg.to_string(), hub_id, channel_id))
                            } else {
                                Err(())
                            }
                        } else {
                            Err(())
                        }
                    } else {
                        Err(())
                    }
                } else {
                    Err(())
                }
            }
            Some("sub") => {
                if let Some(hub_channel) = s.strip_prefix("sub ") {
                    let mut split = hub_channel.split(':');
                    if let (Some(hub), Some(channel)) = (split.next(), split.next()) {
                        Ok(Self::Subscribe(
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
            Some("uns") => {
                if let Some(hub_channel) = s.strip_prefix("uns ") {
                    let mut split = hub_channel.split(':');
                    if let (Some(hub), Some(channel)) = (split.next(), split.next()) {
                        Ok(Self::Unsubscribe(
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
        ctx.text(format!(
            "msg {}:{} {}",
            msg.hub_id, msg.channel_id, msg.message
        ))
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
