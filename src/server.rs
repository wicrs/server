use crate::{api, hub::Hub, Result, ID};
use actix::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Message, Clone)]
#[rtype(result = "Result<ID>")]
pub struct ClientMessage {
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
#[rtype(result = "()")]
pub struct StartTyping {
    pub user_id: ID,
    pub hub_id: ID,
    pub channel_id: ID,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct StopTyping {
    pub user_id: ID,
    pub hub_id: ID,
    pub channel_id: ID,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub user_id: ID,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct Connect {
    pub user_id: ID,
    pub addr: Recipient<Message>,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct Message {
    pub message: crate::channel::Message,
    pub hub_id: ID,
    pub channel_id: ID,
}

pub struct Server {
    subscribed: HashMap<(ID, ID), HashSet<ID>>, // HashMap<(HubID, ChannelID), Vec<UserID>>
    sessions: HashMap<ID, Recipient<Message>>,  // HashMap<UserID, UserSession>
}

impl Server {
    pub fn new() -> Self {
        Self {
            subscribed: HashMap::new(),
            sessions: HashMap::new(),
        }
    }
}

impl Actor for Server {
    type Context = Context<Self>;
}

impl Handler<Connect> for Server {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Self::Context) -> Self::Result {
        self.sessions.insert(msg.user_id, msg.addr);
    }
}

impl Handler<Disconnect> for Server {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Self::Context) -> Self::Result {
        self.sessions.remove(&msg.user_id);
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

impl Handler<ClientMessage> for Server {
    type Result = Result<ID>;

    fn handle(&mut self, msg: ClientMessage, _: &mut Self::Context) -> Self::Result {
        let message = futures::executor::block_on(api::send_message(
            &msg.user_id,
            &msg.hub_id,
            &msg.channel_id,
            msg.message,
        ))?;
        let id = message.id.clone();
        if let Some(subscribed) = self.subscribed.get(&(msg.hub_id, msg.channel_id)) {
            let message = Message {
                message: message,
                hub_id: msg.hub_id,
                channel_id: msg.channel_id,
            };
            for user_id in subscribed {
                if let Some(session) = self.sessions.get(user_id) {
                    let _ = session.do_send(message.clone());
                }
            }
        }
        Ok(id)
    }
}
