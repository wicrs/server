use crate::ID;

pub struct Message {
    pub id: ID,
    pub sender: ID,
    pub created: ID,
    pub content: String,
}
