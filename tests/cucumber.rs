use async_trait::async_trait;
use std::convert::Infallible;

pub struct APITestWorld {}

#[async_trait(?Send)]
impl cucumber::World for APITestWorld {
    type Error = Infallible;

    async fn new() -> Result<Self, Infallible> {
        Ok(Self {})
    }
}

mod client_steps {
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
                t!(|world, matches, _step| {
                    println!("{:?}", matches);
                    world
                }),
            )
            .given(
                "I am trying out Cucumber",
                |mut world: crate::APITestWorld, _step| world,
            )
            .when("I consider what I am doing", |mut world, _step| world)
            .then("I am interested in ATDD", |world, _step| world)
            .then_regex(
                r"^we can (.*) rules with regex$",
                |world, matches, _step| world,
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
