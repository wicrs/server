use std::time::Instant;

use uuid::Uuid;

use crate::channel::{Channel, Message};
use crate::{get_system_millis, new_id};

pub async fn run() {
    channel_test().await;
}

async fn channel_test() {
    let mut channel = Channel::new(
        "testing".to_string(),
        Uuid::from_u128(0),
        Uuid::from_u128(0),
    )
    .await
    .expect("Failed to create channel directory.");
    let now = Instant::now();
    let mut successful = 0;
    for i in 0..10000 {
        if let Ok(_) = channel
            .add_message(Message {
                id: new_id(),
                sender: new_id(),
                created: get_system_millis(),
                content: "testing, here is a number:\n".to_string() + &i.to_string(),
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
    let now = Instant::now();
    let find = channel.find_messages_containing("128".to_string()).await;
    println!(
        "Found {} messages containing \"128\" in {} micros.",
        find.len(),
        now.elapsed().as_micros()
    );
    let mut found = Vec::new();
    for message in channel.find_messages_containing("128".to_string()).await {
        found.push(message.to_string());
    }
    if let Some(id) = found.get(found.len() / 2) {
        let now = Instant::now();
        if let Some(message) = channel.get_message(id.clone()).await {
            println!("First message found was \"{}\", it took {} micros to retrieve it.", message.content, now.elapsed().as_micros());
        }
    }
}
