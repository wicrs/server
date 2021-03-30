use crate::{api, channel, hub::Hub, ApiError, Result, ID};
use actix::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Message, Clone)]
#[rtype(result = "Result<ID>")]
pub struct SendMessage {
    pub user_id: ID,
    pub message: String,
    pub hub_id: ID,
    pub channel_id: ID,
}

#[derive(Message, Clone)]
#[rtype(result = "Result<()>")]
pub struct Subscribe {
    pub user_id: ID,
    pub hub_id: ID,
    pub channel_id: ID,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct Unsubscribe {
    pub user_id: ID,
    pub hub_id: ID,
    pub channel_id: ID,
}

#[derive(Message, Clone)]
#[rtype(result = "Result<()>")]
pub struct StartTyping {
    pub user_id: ID,
    pub hub_id: ID,
    pub channel_id: ID,
}

#[derive(Message, Clone)]
#[rtype(result = "Result<()>")]
pub struct StopTyping {
    pub user_id: ID,
    pub hub_id: ID,
    pub channel_id: ID,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub user_id: ID,
    pub addr: Recipient<ServerClientMessage>,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct Connect {
    pub user_id: ID,
    pub addr: Recipient<ServerClientMessage>,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub enum ServerClientMessage {
    NewMessage(ID, ID, channel::Message),
    TypingStart(ID, ID, ID),
    TypingStop(ID, ID, ID),
}

pub struct Server {
    subscribed: HashMap<(ID, ID), HashSet<ID>>, // HashMap<(HubID, ChannelID), Vec<UserID>>
    sessions: HashMap<ID, HashSet<Recipient<ServerClientMessage>>>, // HashMap<UserID, UserSession>
    typing: HashSet<ID>,
}

impl Server {
    pub fn new() -> Self {
        Self {
            subscribed: HashMap::new(),
            sessions: HashMap::new(),
            typing: HashSet::new(),
        }
    }

    async fn send_message(&self, message: ServerClientMessage, hub_id: ID, channel_id: ID) {
        if let Some(subscribed) = self.subscribed.get(&(hub_id, channel_id)) {
            for user_id in subscribed {
                if let Some(sessions) = self.sessions.get(user_id) {
                    for connection in sessions {
                        let _ = connection.do_send(message.clone());
                    }
                }
            }
        }
    }
}

impl Actor for Server {
    type Context = Context<Self>;
}

impl Handler<StartTyping> for Server {
    type Result = Result<()>;

    fn handle(&mut self, msg: StartTyping, _: &mut Self::Context) -> Self::Result {
        if self.typing.contains(&msg.user_id) {
            return Err(ApiError::AlreadyTyping);
        } else {
            futures::executor::block_on(async {
                let hub = Hub::load(&msg.hub_id).await?;
                hub.get_channel(&msg.user_id, &msg.channel_id)?;
                self.send_message(
                    ServerClientMessage::TypingStart(
                        msg.hub_id.clone(),
                        msg.channel_id.clone(),
                        msg.user_id.clone(),
                    ),
                    msg.hub_id,
                    msg.channel_id,
                )
                .await;
                Ok(())
            })
        }
    }
}

impl Handler<StopTyping> for Server {
    type Result = Result<()>;

    fn handle(&mut self, msg: StopTyping, _: &mut Self::Context) -> Self::Result {
        if self.typing.remove(&msg.user_id) {
            futures::executor::block_on(self.send_message(
                ServerClientMessage::TypingStop(
                    msg.hub_id.clone(),
                    msg.channel_id.clone(),
                    msg.user_id.clone(),
                ),
                msg.hub_id,
                msg.channel_id,
            ));
            Ok(())
        } else {
            Err(ApiError::AlreadyTyping)
        }
    }
}

impl Handler<Connect> for Server {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Self::Context) -> Self::Result {
        self.sessions
            .entry(msg.user_id)
            .or_insert_with(|| HashSet::new())
            .insert(msg.addr);
    }
}

impl Handler<Disconnect> for Server {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Self::Context) -> Self::Result {
        if let Some(set) = self.sessions.get_mut(&msg.user_id) {
            set.remove(&msg.addr);
        }
    }
}

impl Handler<Subscribe> for Server {
    type Result = Result<()>;

    fn handle(&mut self, msg: Subscribe, _: &mut Self::Context) -> Self::Result {
        let Subscribe {
            user_id,
            hub_id,
            channel_id,
        } = msg.clone();
        let test: Self::Result = futures::executor::block_on(async move {
            let hub = Hub::load(&hub_id).await?;
            hub.get_channel(&user_id, &channel_id)?;
            Ok(())
        });
        test?;
        self.subscribed
            .entry((msg.hub_id, msg.channel_id))
            .or_insert_with(HashSet::new)
            .insert(msg.user_id);
        Ok(())
    }
}

impl Handler<Unsubscribe> for Server {
    type Result = ();

    fn handle(&mut self, msg: Unsubscribe, _: &mut Self::Context) -> Self::Result {
        if let Some(entry) = self.subscribed.get_mut(&(msg.hub_id, msg.channel_id)) {
            entry.remove(&msg.user_id);
        }
    }
}

impl Handler<SendMessage> for Server {
    type Result = Result<ID>;

    fn handle(&mut self, msg: SendMessage, _: &mut Self::Context) -> Self::Result {
        futures::executor::block_on(async {
            let message =
                api::send_message(&msg.user_id, &msg.hub_id, &msg.channel_id, msg.message).await?;
            let id = message.id.clone();
            self.send_message(
                ServerClientMessage::NewMessage(msg.hub_id, msg.channel_id, message),
                msg.hub_id,
                msg.channel_id,
            )
            .await;
            Ok(id)
        })
    }
}
