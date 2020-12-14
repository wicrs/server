use async_trait::async_trait;
use reqwest::Response;
use std::{convert::Infallible, panic::AssertUnwindSafe};
use std::sync::Arc;

#[derive(Clone)]
pub struct APITestWorld {
    response: Arc<AssertUnwindSafe<Option<Response>>>
}

#[async_trait(?Send)]
impl cucumber::World for APITestWorld {
    type Error = Infallible;

    async fn new() -> Result<Self, Infallible> {
        Ok(Self {
            response: Arc::new(AssertUnwindSafe(None))
        })
    }
}

mod client_steps {
    use std::{panic::AssertUnwindSafe, sync::Arc};

    use cucumber::{t, Steps};

    pub fn steps() -> Steps<crate::APITestWorld> {
        let mut builder: Steps<crate::APITestWorld> = Steps::new();

        builder
            .given_async(
                "I have an instance of wirc on localhost",
                t!(|mut world, _step| {
                    tokio::task::spawn(wirc_server::run());
                    world
                }),
            )
            .when_regex_async(
                "I GET (.*)",
                t!(|mut world, matches, _step| {
                    world.response = Arc::new(AssertUnwindSafe(Some(reqwest::get(reqwest::Url::parse(&("http://localhost:24816/".to_string() + &matches[0].strip_prefix("I GET ").unwrap())).unwrap()).await.unwrap())));
                    world
                }),
            )
            .then_regex(
                "I should see a redirect to (.*)",
                |world, matches, _step| {
                    let arc = world.response.clone();
                    let option = &arc.0;
                    if let Some(response) = option {
                        let string = matches[0].strip_prefix("I should see a redirect to ").unwrap().to_string();
                        assert!(&response.url().as_str().starts_with(&string));
                    }
                    world
                }
            );

        builder
    }
}

#[tokio::main]
async fn main() {
    // Do any setup you need to do before running the Cucumber runner.
    // e.g. setup_some_db_thing()?;
    let runner = cucumber::Cucumber::<APITestWorld>::new()
        .features(&["./tests/features/"])
        .steps(client_steps::steps());
    runner.run().await;
}
