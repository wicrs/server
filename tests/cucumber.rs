use async_trait::async_trait;
use cucumber_rust::{given, then, when, World, WorldInit};
use reqwest::Response;
use std::{convert::Infallible, net::{IpAddr, Ipv4Addr, SocketAddr}, panic::AssertUnwindSafe, sync::Arc};

#[derive(WorldInit)]
pub struct MyWorld {
    wirc_running: bool,
    id: Option<String>,
    token: Option<String>,
    response: Option<AssertUnwindSafe<Arc<Response>>>,
}

#[async_trait(?Send)]
impl World for MyWorld {
    type Error = Infallible;

    async fn new() -> Result<Self, Infallible> {
        Ok(Self {
            wirc_running: false,
            id: None,
            token: None,
            response: None,
        })
    }
}

#[given("I have an instance of wirc on localhost")]
async fn wirc_running(world: &mut MyWorld) {
    if !world.wirc_running || world.id.is_none() || world.token.is_none() {
        let server = wirc_server::testing().await;
        tokio::task::spawn(
            warp::serve(server.0).run(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 24816)),
        );
        world.wirc_running = true;
        world.id = Some(server.1);
        world.token = Some(server.2);
    }
}

#[when(regex = r"I perform a GET on (.*)")]
async fn get_url(world: &mut MyWorld, url: String) {
    assert!(world.wirc_running);
    world.response = Some(AssertUnwindSafe(Arc::new(
        reqwest::get(reqwest::Url::parse(&url).unwrap())
            .await
            .unwrap(),
    )));
}

#[when(regex = r"I perform an authenticated GET on (.*)")]
async fn get_url_auth(world: &mut MyWorld, url: String) {
    assert!(world.wirc_running);
    world.response = Some(AssertUnwindSafe(Arc::new(
        reqwest::get(
            reqwest::Url::parse(
                &(url
                    + &format!(
                        "?account={}&token={}",
                        world.id.clone().unwrap(),
                        world.token.clone().unwrap()
                    )),
            )
            .unwrap(),
        )
        .await
        .unwrap(),
    )));
}

#[then(regex = r"I should be redirected to (.*)")]
fn redirect_to(world: &mut MyWorld, url: String) {
    assert!(&world
        .response
        .as_deref()
        .unwrap()
        .url()
        .as_str()
        .starts_with(&url));
}

#[then(regex = r"I should see (.*)")]
async fn recieve_json(world: &mut MyWorld, json: String) {
    let taken = world.response.take().unwrap().0;
    let response = Arc::try_unwrap(taken).expect("Failed to extract resposne from Arc");
    assert_eq!(&json, &response.text().await.expect("Empty response."))
}

#[tokio::main]
async fn main() {
    let runner = MyWorld::init(&["tests/features"]);
    runner.run().await;
}
