use async_trait::async_trait;
use cucumber_rust::{given, then, when, World, WorldInit};
use reqwest::Response;
use std::{convert::Infallible, panic::AssertUnwindSafe};

#[derive(WorldInit)]
pub struct MyWorld {
    wirc_running: bool,
    response: Option<AssertUnwindSafe<Response>>,
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

#[given("I have an instance of wirc on localhost")]
async fn wirc_running(world: &mut MyWorld) {
    tokio::task::spawn(wirc_server::run());
    world.wirc_running = true;
}

#[when(regex = r"I GET (.*)")]
async fn get_url(world: &mut MyWorld, url: String) {
    assert!(world.wirc_running);
    world.response = Some(AssertUnwindSafe(
        reqwest::get(reqwest::Url::parse(&url).unwrap())
            .await
            .unwrap(),
    ));
}

#[then(regex = r"I should be redirected to (.*)")]
fn redirect_to(world: &mut MyWorld, url: String) {
    assert!(&world.response.as_deref().unwrap().url().as_str().starts_with(&url));
}

#[tokio::main]
async fn main() {
    let runner = MyWorld::init(&["tests/features"]);
    runner.run_and_exit().await;
}
