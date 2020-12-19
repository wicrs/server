use async_trait::async_trait;
use cucumber_rust::{given, then, when, World, WorldInit};
use reqwest::{Method, Response};
use serde::Serialize;
use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};
use wirc_server::user::{User, GenericUser, GenericAccount, Account};

#[derive(WorldInit)]
pub struct MyWorld {
    wirc_running: bool,
    response: String,
}

#[async_trait(?Send)]
impl World for MyWorld {
    type Error = Infallible;

    async fn new() -> Result<Self, Infallible> {
        Ok(Self {
            wirc_running: false,
            response: String::new(),
        })
    }
}

#[given("wirc is running on localhost")]
async fn wirc_running(world: &mut MyWorld) {
    if !world.wirc_running {
        let server = wirc_server::testing().await;
        tokio::task::spawn(
            warp::serve(server.0).run(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 24816)),
        );
        world.wirc_running = true;
    }
}

#[when("a user attempts to login using their GitHub account")]
async fn github_login(world: &mut MyWorld) {
    let response = reqwest::get("http://localhost:24816/api/v1/login/github").await.expect("No response.");
    world.response = response.url().to_string();
}

#[then("the user should be redirected to the github login page")]
async fn github_redirected(world: &mut MyWorld) {
    assert!(world.response.starts_with("https://github.com/login"));
}

#[then(regex = r"the user should receive user information")]
async fn recieve_user(world: &mut MyWorld) {
    serde_json::from_str::<User>(&world.response).expect("Did not receive valid user information");
}

#[then(regex = r"the user should receive basic user information")]
async fn recieve_generic_user(world: &mut MyWorld) {
    serde_json::from_str::<GenericUser>(&world.response).expect("Did not receive valid user information");
}

#[then(regex = r"the user should receive account information")]
async fn recieve_account(world: &mut MyWorld) {
    serde_json::from_str::<Account>(&world.response).expect("Did not receive valid account information");
}

#[then(regex = r"the user should receive basic account information")]
async fn recieve_generic_account(world: &mut MyWorld) {
    serde_json::from_str::<GenericAccount>(&world.response).expect("Did not receive valid account information");
}

#[tokio::main]
async fn main() {
    let runner = MyWorld::init(&["tests/features"]);
    runner.run().await;
}
