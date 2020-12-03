use crate::ID;

pub struct User {
    pub id: ID,
    pub username: String,
    pub bot: bool,
    pub created: u128,
    pub owner_id: ID,
    pub in_guilds: Vec<ID>,
}

pub struct Account {
    pub id: u128,
    pub users: Vec<User>,
}

impl Account {
    pub fn new(id: u128) -> Self {
        Self {
            id,
            users: Vec::new()
        }
    }
}
