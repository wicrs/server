use std::panic::AssertUnwindSafe;
use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use async_trait::async_trait;
use cucumber_rust::{given, then, when, World, WorldInit};
use reqwest::Url;
use serde::Serialize;
use tokio::task::JoinHandle;

use wirc_server::channel::{Channel, Message};
use wirc_server::user::{Account, GenericAccount, GenericUser, User};
use wirc_server::ID;

#[derive(WorldInit)]
pub struct MyWorld {
    response: String,
    account: Option<ID>,
    hub: Option<ID>,
    channel: Option<ID>,
    message: Option<ID>,
    running: Option<AssertUnwindSafe<JoinHandle<()>>>,
}

#[async_trait(? Send)]
impl World for MyWorld {
    type Error = Infallible;

    async fn new() -> Result<Self, Infallible> {
        Ok(Self {
            response: String::new(),
            account: None,
            hub: None,
            channel: None,
            message: None,
            running: None,
        })
    }
}

#[given("wirc is running on localhost")]
async fn wirc_running(world: &mut MyWorld) {
    assert!(world.running.is_none());
    let server = wirc_server::testing().await;
    world.running = Some(AssertUnwindSafe(tokio::task::spawn(
        warp::serve(server.0).run(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 24816)),
    )));
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

#[when("the user attempts to create a new account")]
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

#[when("the user requests their information")]
async fn get_user_auth(world: &mut MyWorld) {
    let response = reqwest::get(
        Url::parse("http://localhost:24816/api/v1/user?user=testuser&token=testtoken").unwrap(),
    )
    .await
    .expect("No response.");
    world.response = response.text().await.expect("Empty repsonse.").to_string();
}

#[when("the user tells the server to invalidate all of their tokens")]
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

#[when("the user attempts to create a new hub")]
async fn create_hub(world: &mut MyWorld) {
    if world.hub.is_some() {
        return;
    }
    assert!(world.account.is_some());
    let response = reqwest::Client::new()
        .get(
            Url::parse("http://localhost:24816/api/v1/hubs/create?user=testuser&token=testtoken")
                .unwrap(),
        )
        .json(&serde_json::json!({
        "name": "testhub",
        "account": world.account.unwrap().to_string(),
        }))
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.unwrap_or("".to_string());
    world.hub = Some(world.response.parse().unwrap());
}

#[given("the user has an account")]
async fn setup_account(world: &mut MyWorld) {
    create_account(world).await;
}

#[given("the user is in a hub")]
async fn setup_hub(world: &mut MyWorld) {
    create_hub(world).await;
}

#[when("the user attempts to create a new text channel")]
async fn create_channel(world: &mut MyWorld) {
    if world.channel.is_some() {
        return;
    }
    assert!(world.account.is_some());
    assert!(world.hub.is_some());
    let response = reqwest::Client::new()
        .get(
            Url::parse(
                "http://localhost:24816/api/v1/hubs/create_channel?user=testuser&token=testtoken",
            )
            .unwrap(),
        )
        .json(&serde_json::json!({
        "hub": world.hub.unwrap(),
        "name": "testchannel",
        "account": world.account.unwrap().to_string(),
        }))
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.unwrap_or("".to_string());
    world.channel = Some(world.response.parse().unwrap());
}

#[given("the user has access to a text channel")]
async fn setup_channel(world: &mut MyWorld) {
    create_channel(world).await;
}

#[when("the user asks the server for a list of text channels")]
async fn get_channels(world: &mut MyWorld) {
    assert!(world.account.is_some());
    assert!(world.hub.is_some());
    let response = reqwest::Client::new()
        .get(
            Url::parse("http://localhost:24816/api/v1/hubs/channels?user=testuser&token=testtoken")
                .unwrap(),
        )
        .json(&serde_json::json!({
        "hub": world.hub.unwrap(),
        "account": world.account.unwrap().to_string(),
        }))
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.unwrap_or("".to_string());
}

#[when("the user sends a message in a channel in a hub")]
async fn send_message(world: &mut MyWorld) {
    assert!(world.account.is_some());
    assert!(world.hub.is_some());
    assert!(world.channel.is_some());
    let response = reqwest::Client::new()
        .get(
            Url::parse(
                "http://localhost:24816/api/v1/hubs/send_message?user=testuser&token=testtoken",
            )
            .unwrap(),
        )
        .json(&serde_json::json!({
        "hub": world.hub.unwrap(),
        "account": world.account.unwrap(),
        "channel": world.channel.unwrap(),
        "message": "test message"
        }))
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.unwrap_or("".to_string());
    world.message = Some(world.response.parse().unwrap());
}

#[given(regex = r"(\\d+) messages have been sent in the channel")]
async fn n_messages_sent(world: &mut MyWorld, n: String) {
    for _i in 0..n.parse::<u32>().unwrap() {
        send_message(world).await;
    }
}

#[when(regex = r"the user tries to get the last (\\d+) messages")]
async fn get_last_messages(world: &mut MyWorld, n: String) {
    assert!(world.account.is_some());
    assert!(world.hub.is_some());
    assert!(world.channel.is_some());
    let response = reqwest::Client::new()
        .get(
            Url::parse("http://localhost:24816/api/v1/hubs/messages?user=testuser&token=testtoken")
                .unwrap(),
        )
        .json(&serde_json::json!({
        "hub": world.hub.unwrap(),
        "account": world.account.unwrap().to_string(),
        "channel": world.channel.unwrap().to_string(),
        "count": n.parse::<u128>().unwrap()
        }))
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.unwrap_or("".to_string());
}

#[then(regex = r"the user should see (\\d+) messages")]
async fn check_channel_messages(world: &mut MyWorld, n: String) {
    assert_eq!(
        serde_json::from_str::<Vec<Message>>(&world.response)
            .unwrap()
            .len(),
        n.parse::<usize>().unwrap()
    );
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

#[then("the user should receive user information")]
async fn recieve_user(world: &mut MyWorld) {
    serde_json::from_str::<User>(&world.response).expect("Did not receive valid user information");
}

#[then("the user should receive basic user information")]
async fn recieve_generic_user(world: &mut MyWorld) {
    serde_json::from_str::<GenericUser>(&world.response)
        .expect("Did not receive valid user information");
}

#[then("the user should receive account information")]
async fn recieve_account(world: &mut MyWorld) {
    serde_json::from_str::<Account>(&world.response)
        .expect("Did not receive valid account information");
}

#[then("the user should receive basic account information")]
async fn recieve_generic_account(world: &mut MyWorld) {
    serde_json::from_str::<GenericAccount>(&world.response)
        .expect("Did not receive valid account information");
}

#[then("the user should receive an ID")]
async fn recieve_id(world: &mut MyWorld) {
    world
        .response
        .parse::<ID>()
        .expect("Did not receive valid account information");
}

#[then("the user should receive a list of text channels")]
async fn recieve_channel_list(world: &mut MyWorld) {
    serde_json::from_str::<Vec<Channel>>(&world.response)
        .expect("Did not receive a valid list of channels");
}

#[then("panic response")]
async fn panic_response(world: &mut MyWorld) {
    panic!(world.response.clone());
}

#[tokio::main]
async fn main() {
    let _reset = std::fs::remove_dir_all("data");
    let runner = MyWorld::init(&["tests/features"]);
    runner.cli().run().await;
}
