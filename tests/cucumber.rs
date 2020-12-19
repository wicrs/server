use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use async_trait::async_trait;
use cucumber_rust::{given, then, when, World, WorldInit};
use reqwest::Url;
use serde::Serialize;

use wirc_server::user::{Account, GenericAccount, GenericUser, User};
use wirc_server::ID;

#[derive(WorldInit)]
pub struct MyWorld {
    wirc_running: bool,
    response: String,
    account: Option<ID>,
    hub: Option<ID>,
    channel: Option<ID>,
}

#[async_trait(? Send)]
impl World for MyWorld {
    type Error = Infallible;

    async fn new() -> Result<Self, Infallible> {
        Ok(Self {
            wirc_running: false,
            response: String::new(),
            account: None,
            hub: None,
            channel: None,
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

#[when("the user attempts to login using their GitHub account")]
async fn github_login(world: &mut MyWorld) {
    let response = reqwest::get("http://localhost:24816/api/v1/login/github")
        .await
        .expect("No response.");
    world.response = response.url().to_string();
}

#[derive(Serialize)]
struct Name {
    name: String,
}

#[when("the authenticated user attempts to create a new account")]
async fn create_account(world: &mut MyWorld) {
    let response = reqwest::Client::new()
        .get(
            Url::parse(
                "http://localhost:24816/api/v1/user/addaccount?user=testuser&token=testtoken",
            )
            .unwrap(),
        )
        .json(&serde_json::json!({
            "name": "test"
        }))
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.expect("Empty repsonse.").to_string();
    let account = serde_json::from_str::<Account>(&world.response)
        .expect("Failed to create a new account for the test");
    world.account = Some(account.id);
}

#[when("the authenticated user requests their information")]
async fn get_user_auth(world: &mut MyWorld) {
    let response = reqwest::get(
        Url::parse("http://localhost:24816/api/v1/user?user=testuser&token=testtoken").unwrap(),
    )
    .await
    .expect("No response.");
    world.response = response.text().await.expect("Empty repsonse.").to_string();
}

#[when("the authenticated user tells the server to invalidate all of their tokens")]
async fn invalidate_user_tokens(world: &mut MyWorld) {
    let response = reqwest::get(
        Url::parse("http://localhost:24816/api/v1/invalidate?user=testuser&token=testtoken")
            .unwrap(),
    )
    .await
    .expect("No response.");
    world.response = response.text().await.unwrap_or("".to_string());
    assert_eq!(&world.response, "Success!");
}

#[when("a user requests another user's information")]
async fn get_user(world: &mut MyWorld) {
    let response = reqwest::get(Url::parse("http://localhost:24816/api/v1/user/testuser").unwrap())
        .await
        .expect("No response.");
    world.response = response.text().await.expect("Empty repsonse.").to_string();
}

#[when("the authenticated user attempts to create a new hub")]
async fn create_hub(world: &mut MyWorld) {
    create_account(world).await;
    let response = reqwest::Client::new()
        .get(
            Url::parse("http://localhost:24816/api/v1/hubs/create?user=testuser&token=testtoken")
                .unwrap(),
        )
        .json(&serde_json::json!({
        "name": "testhub",
        "account": world.account.expect("No account has been created for testing").to_string(),
        }))
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.unwrap_or("".to_string());
    world.hub = Some(world.response.parse().unwrap());
}

#[then("the server should respond with the OK status")]
async fn hub_create_success(world: &mut MyWorld) {
    assert!(&world.hub.is_some());
}

#[then("the user should be denied access")]
async fn auth_denied(world: &mut MyWorld) {
    assert_eq!(&world.response, "Invalid authentication details.");
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
    serde_json::from_str::<GenericUser>(&world.response)
        .expect("Did not receive valid user information");
}

#[then(regex = r"the user should receive account information")]
async fn recieve_account(world: &mut MyWorld) {
    serde_json::from_str::<Account>(&world.response)
        .expect("Did not receive valid account information");
}

#[then(regex = r"the user should receive basic account information")]
async fn recieve_generic_account(world: &mut MyWorld) {
    serde_json::from_str::<GenericAccount>(&world.response)
        .expect("Did not receive valid account information");
}

#[tokio::main]
async fn main() {
    let runner = MyWorld::init(&["tests/features"]);
    runner.run().await;
}
