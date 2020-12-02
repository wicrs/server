pub struct User {
    pub id: usize,
    pub username: String,
    pub bot: bool,
    pub created: u128,
    pub in_guilds: Vec<usize>
}
