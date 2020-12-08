use rand::{Rng, distributions::Alphanumeric, thread_rng};
use std::{iter, path::Path, time::Instant};

use uuid::Uuid;

use crate::channel::{Channel, Message};
use crate::{get_system_millis, new_id};

pub async fn run() {
    channel_test().await;
}

async fn channel_test() {
    let search_string = std::env::args().last().expect("No search term provided.");
    let mut channel = Channel::new(
        "testing".to_string(),
        Uuid::from_u128(0),
        Uuid::from_u128(0),
    )
    .await
    .expect("Failed to create channel directory.");
    let mut rng = thread_rng();
    let now = Instant::now();
    if !Path::new(&channel.get_current_file().await).exists() {
        let mut successful = 0;
        for _ in 0..1000000 {
            if let Ok(_) = channel
                .add_message(Message {
                    id: new_id(),
                    sender: new_id(),
                    created: get_system_millis(),
                    content: iter::repeat(())
                        .map(|()| rng.sample(Alphanumeric))
                        .take(128)
                        .collect(),
                })
                .await
            {
                successful = successful + 1;
            }
        }
        println!(
            "Wrote {} messages to channel in {} micros.",
            successful,
            now.elapsed().as_micros()
        );
    }
    let now = Instant::now();
    let mut find = channel
        .find_messages_containing(search_string.as_str(), false)
        .await;
    println!(
        "Found {} messages containing \"{}\" in {} micros.",
        find.len(),
        search_string,
        now.elapsed().as_micros()
    );
    find.sort_unstable_by_key(|m| m.created);
    if let Some(message) = find.first() {
        let now = Instant::now();
        if let Some(message) = channel.get_message(message.id.to_string()).await {
            println!(
                "First message found was \"{}\", it took {} micros to retrieve it.",
                message.content,
                now.elapsed().as_micros()
            );
        }
    }
}
