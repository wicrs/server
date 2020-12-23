use std::convert::Infallible;
use std::panic::AssertUnwindSafe;

use async_trait::async_trait;
use cucumber_rust::{Cucumber, World};
use reqwest::Url;
use tokio::task::JoinHandle;

use wicrs_common::ID;
use wicrs_server::user::Account;

pub struct MyWorld {
    response: String,
    account: Option<ID>,
    hub: Option<ID>,
    channel: Option<ID>,
    message: Option<ID>,
    running: Option<AssertUnwindSafe<JoinHandle<()>>>,
}

#[async_trait(?Send)]
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

async fn create_account(world: &mut MyWorld) {
    let response = reqwest::Client::new()
        .get(
            Url::parse(
                "http://localhost:24816/api/v1/user/addaccount?user=testuser&token=testtoken",
            )
            .unwrap(),
        )
        .json(&wicrs_common::api_types::CreateAccountQuery {
            name: "test account".to_string(),
            is_bot: false,
        })
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.expect("Empty repsonse.").to_string();
    let account = serde_json::from_str::<Account>(&world.response)
        .expect("Failed to create a new account for the test");
    world.account = Some(account.id);
}

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
        .json(&wicrs_common::api_types::HubCreateQuery {
            account: world.account.unwrap(),
            name: "test hub".to_string(),
        })
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.unwrap_or("".to_string());
    world.hub = Some(world.response.parse().unwrap());
}

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
        .json(&wicrs_common::api_types::ChannelCreateQuery {
            account: world.account.unwrap(),
            hub: world.hub.unwrap(),
            name: "test channel".to_string(),
        })
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.unwrap_or("".to_string());
    world.channel = Some(world.response.parse().unwrap());
}

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
        .json(&wicrs_common::api_types::MessageSendQuery {
            account: world.account.unwrap(),
            hub: world.hub.unwrap(),
            channel: world.channel.unwrap(),
            message: "test message".to_string(),
        })
        .send()
        .await
        .expect("No response.");
    world.response = response.text().await.unwrap_or("".to_string());
    world.message = Some(world.response.parse().unwrap());
}

mod steps {
    use serde::Serialize;
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        panic::AssertUnwindSafe,
    };

    use super::ID;
    use wicrs_common::types::{Account, Channel, GenericAccount, GenericUser, Message, User};

    #[derive(Serialize)]
    struct Name {
        name: String,
    }

    use cucumber_rust::{t, Steps};
    use reqwest::Url;

    use crate::{create_account, create_channel, create_hub, send_message};

    pub fn given() -> Steps<crate::MyWorld> {
        let mut builder: Steps<crate::MyWorld> = Steps::new();

        builder
            .given_async(
                "the server is running on localhost",
                t!(|mut world, _step| {
                    assert!(world.running.is_none());
                    let server = wicrs_server::testing().await;
                    world.running = Some(AssertUnwindSafe(tokio::task::spawn(
                        warp::serve(server.0)
                            .run(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 24816)),
                    )));
                    world
                }),
            )
            .given_async(
                "the user has an account",
                t!(|mut world, _step| {
                    create_account(&mut world).await;
                    world
                }),
            )
            .given_async(
                "the user is in a hub",
                t!(|mut world, _step| {
                    create_hub(&mut world).await;
                    world
                }),
            )
            .given_async(
                "the user has access to a text channel",
                t!(|mut world, _step| {
                    create_channel(&mut world).await;
                    world
                }),
            )
            .given_regex_async(
                r"(\d+) messages have been sent in the channel",
                t!(|mut world, matches, _step| {
                    for _i in 0..matches[1].parse::<u32>().unwrap() {
                        send_message(&mut world).await;
                    }
                    world
                }),
            );
        builder
    }

    pub fn when() -> Steps<crate::MyWorld> {
        let mut builder: Steps<crate::MyWorld> = Steps::new();
        builder
        .when_async(
            "the user attempts to login using their GitHub account",
            t!(|mut world, _step| {
                let response = reqwest::get("http://localhost:24816/api/v1/login/github")
                    .await
                    .expect("No response.");
                world.response = response.url().to_string();
                world
            }),
        )
        .when_async(
            "the user attempts to create a new account",
            t!(|mut world, _step| {
                create_account(&mut world).await;
                world
            }),
        )
        .when_async(
            "the user requests their information",
            t!(|mut world, _step| {
                let response = reqwest::get(
                    Url::parse("http://localhost:24816/api/v1/user?user=testuser&token=testtoken").unwrap(),
                )
                .await
                .expect("No response.");
                world.response = response.text().await.expect("Empty repsonse.").to_string();
                world
            }),
        )
        .when_async(
            "the user tells the server to invalidate all of their tokens",
            t!(|mut world, _step| {
                let response = reqwest::get(
                    Url::parse("http://localhost:24816/api/v1/invalidate?user=testuser&token=testtoken")
                        .unwrap(),
                )
                .await
                .expect("No response.");
                world.response = response.text().await.unwrap_or("".to_string());
                assert_eq!(&world.response, "Success!");
                world
            }),
        )
        .when_async(
            "a user requests another user's information",
            t!(|mut world, _step| {
                let response = reqwest::get(Url::parse("http://localhost:24816/api/v1/user/testuser").unwrap())
                    .await
                    .expect("No response.");
                world.response = response.text().await.expect("Empty repsonse.").to_string();
                world
            }),
        )
        .when_async(
            "the user attempts to create a new hub",
            t!(|mut world, _step| {
                create_hub(&mut world).await;
                world
            }),
        )
        .when_async(
            "the user attempts to create a new text channel",
            t!(|mut world, _step| {
                create_channel(&mut world).await;
                world
            }),
        )
        .when_async(
            "the user asks the server for a list of text channels",
            t!(|mut world, _step| {
                assert!(world.account.is_some());
                assert!(world.hub.is_some());
                let response = reqwest::Client::new()
                    .get(
                        Url::parse("http://localhost:24816/api/v1/hubs/channels?user=testuser&token=testtoken")
                            .unwrap(),
                    )
                    .json(&wicrs_common::api_types::ChannelsQuery {
                        account: world.account.unwrap(),
                        hub: world.hub.unwrap(),
                    })
                    .send()
                    .await
                    .expect("No response.");
                world.response = response.text().await.unwrap_or("".to_string());
                world
            }),
        )
        .when_async(
            "the user sends a message in a channel in a hub",
            t!(|mut world, _step| {
                send_message(&mut world).await;
                world
            }),
        )
        .when_regex_async(
            r"the user tries to get the last (\d+) messages",
            t!(|mut world, matches, _step| {
                assert!(world.account.is_some());
                assert!(world.hub.is_some());
                assert!(world.channel.is_some());
                let response = reqwest::Client::new()
                    .get(
                        Url::parse("http://localhost:24816/api/v1/hubs/messages?user=testuser&token=testtoken")
                            .unwrap(),
                    )
                    .json(&wicrs_common::api_types::LastMessagesQuery {
                        account: world.account.unwrap(),
                        hub: world.hub.unwrap(),
                        channel: world.channel.unwrap(),
                        count: matches[1].parse().unwrap(),
                    })
                    .send()
                    .await
                    .expect("No response.");
                world.response = response.text().await.unwrap_or("".to_string());
                world
            }),
        );
        builder
    }

    pub fn then() -> Steps<crate::MyWorld> {
        let mut builder: Steps<crate::MyWorld> = Steps::new();
        builder
            .then_regex_async(
                r"the user should see (\d+) messages",
                t!(|mut world, matches, _step| {
                    assert_eq!(
                        serde_json::from_str::<Vec<Message>>(&world.response)
                            .unwrap()
                            .len(),
                        matches[1].parse::<usize>().unwrap()
                    );
                    world
                }),
            )
            .then_async(
                "the user should be denied access",
                t!(|mut world, _step| {
                    assert_eq!(&world.response, "Invalid authentication details.");
                    world
                }),
            )
            .then_async(
                "the user should be redirected to the github login page",
                t!(|mut world, _step| {
                    assert!(world.response.starts_with("https://github.com/login"));
                    world
                }),
            )
            .then_async(
                "the user should receive user information",
                t!(|mut world, _step| {
                    serde_json::from_str::<User>(&world.response)
                        .expect("Did not receive valid user information");
                    world
                }),
            )
            .then_async(
                "the user should receive basic user information",
                t!(|mut world, _step| {
                    serde_json::from_str::<GenericUser>(&world.response)
                        .expect("Did not receive valid user information");
                    world
                }),
            )
            .then_async(
                "the user should receive account information",
                t!(|mut world, _step| {
                    serde_json::from_str::<Account>(&world.response)
                        .expect("Did not receive valid account information");
                    world
                }),
            )
            .then_async(
                "the user should receive basic account information",
                t!(|mut world, _step| {
                    serde_json::from_str::<GenericAccount>(&world.response)
                        .expect("Did not receive valid account information");
                    world
                }),
            )
            .then_async(
                "the user should receive a list of text channels",
                t!(|mut world, _step| {
                    serde_json::from_str::<Vec<Channel>>(&world.response)
                        .expect("Did not receive a valid list of channels");
                    world
                }),
            )
            .then_async(
                "the user should receive an ID",
                t!(|mut world, _step| {
                    world
                        .response
                        .parse::<ID>()
                        .expect("Did not receive valid account information");
                    world
                }),
            );
        builder
    }
}

#[tokio::main]
async fn main() {
    let _reset = std::fs::remove_dir_all("data");
    let runner = Cucumber::<MyWorld>::new()
        .features(&["./tests/features"])
        .steps(steps::given())
        .steps(steps::when())
        .steps(steps::then());
    runner.run().await;
}
