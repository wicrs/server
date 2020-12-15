use async_trait::async_trait;
use cucumber_rust::{given, then, when, World, WorldInit};
use reqwest::{Method, Response};
use serde::Serialize;
use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    panic::AssertUnwindSafe,
    sync::Arc,
};
use wirc_server::user::{Account, GenericAccount, GenericUser, User};

#[derive(WorldInit)]
pub struct MyWorld {
    wirc_running: bool,
    response: Option<AssertUnwindSafe<Arc<Response>>>,
}

#[async_trait(?Send)]
impl World for MyWorld {
    type Error = Infallible;

    async fn new() -> Result<Self, Infallible> {
        Ok(Self {
            wirc_running: false,
            response: None,
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

#[when(regex = r"a user navigates to (.*)")]
async fn get_url(world: &mut MyWorld, url: String) {
    assert!(world.wirc_running);
    world.response = Some(AssertUnwindSafe(Arc::new(
        reqwest::get(reqwest::Url::parse(&url).unwrap())
            .await
            .unwrap(),
    )));
}

#[when(regex = r"an authenticated user navigates to (.*)")]
async fn get_url_auth(world: &mut MyWorld, url: String) {
    assert!(world.wirc_running);
    world.response = Some(AssertUnwindSafe(Arc::new(
        reqwest::get(reqwest::Url::parse(&(url + "?account=testaccount&token=testtoken")).unwrap())
            .await
            .unwrap(),
    )));
}

#[derive(Serialize)]
struct Name {
    name: String,
}

#[when(regex = r"an authenticated user sends a name to (.*)")]
async fn get_send_name(world: &mut MyWorld, url: String) {
    assert!(world.wirc_running);
    let request = reqwest::Client::new()
        .request(
            Method::GET,
            reqwest::Url::parse(&(url + "?account=testaccount&token=testtoken")).unwrap(),
        )
        .json(&Name {
            name: "test_name".to_string(),
        });
    world.response = Some(AssertUnwindSafe(Arc::new(request.send().await.unwrap())));
}

fn get_response(world: &mut MyWorld) -> Response {
    let taken = world.response.take().unwrap().0;
    Arc::try_unwrap(taken).expect("Failed to extract response from Arc")
}

#[then(regex = r"the user should be redirected to (.*)")]
fn redirect_to(world: &mut MyWorld, url: String) {
    assert!(get_response(world).url().as_str().starts_with(&url));
}

#[then(regex = r"the user should receive an account")]
async fn recieve_account(world: &mut MyWorld) {
    get_response(world)
        .json::<Account>()
        .await
        .expect("Did not receive valid account data");
}

#[then(regex = r"the user should receive a generic account")]
async fn recieve_generic_account(world: &mut MyWorld) {
    get_response(world)
        .json::<GenericAccount>()
        .await
        .expect("Did not receive valid generic account data");
}

#[then(regex = r"the user should receive a user")]
async fn recieve_user(world: &mut MyWorld) {
    get_response(world)
        .json::<User>()
        .await
        .expect("Did not receive valid user data");
}

#[then(regex = r"the user should receive a generic user")]
async fn recieve_generic_user(world: &mut MyWorld) {
    get_response(world)
        .json::<GenericUser>()
        .await
        .expect("Did not receive valid generic user data");
}

#[then(regex = r"the user should receive text (.*)")]
async fn recieve_text(world: &mut MyWorld, json: String) {
    assert_eq!(
        &json,
        &get_response(world).text().await.expect("Empty response.")
    );
}

#[tokio::main]
async fn main() {
    let runner = MyWorld::init(&["tests/features"]);
    runner.run().await;
}
