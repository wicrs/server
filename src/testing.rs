use std::time::Instant;

use uuid::Uuid;

use crate::{get_system_millis, new_id};
use crate::channel::{Message, Channel};

pub async fn run() {
    message_test();
    channel_test().await;
    message_escape_result_test();
}

async fn channel_test() {
    let mut channel = Channel::new("testing".to_string(), Uuid::from_u128(0), Uuid::from_u128(0)).await.expect("Failed to create channel directory.");
    let now = Instant::now();
    let mut successful = 0;
    for i in 0..10000 {
        if let Ok(_) = channel.add_message(Message {
            id: new_id(),
            sender: new_id(),
            created: get_system_millis(),
            content: "testing, here is a number:\n".to_string() + &i.to_string()
        }).await {
            successful = successful + 1;
        }
    }
    println!("Wrote {} messages to channel in {} micros.", successful, now.elapsed().as_micros());
}

fn message_escape_result_test() {
    let test_string = "This is the first line!\nThis is the second line!".to_string();
    let message = Message {
        id: new_id(),
        sender: new_id(),
        created: get_system_millis(),
        content: test_string.clone(),
    };
    println!("Message to string: {}\nMessage content from message string:\n{}", message.to_string(), test_string);
}

fn message_test() {
    let mut messages = Vec::new();
    for i in 0..10000 {
        messages.push(Message {
            id: new_id(),
            sender: new_id(),
            created: get_system_millis(),
            content: "testing, here is a number:\n".to_string() + &i.to_string()
        })
    }
    let mut message_strings = Vec::new();
    let now = Instant::now();
    for message in messages.iter() {
        message_strings.push(message.to_string());
    }
    println!("10000 messages to strings took {} micros.", now.elapsed().as_micros());
    let mut messages_parsed = Vec::new();
    let now = Instant::now();
    for message_string in message_strings.iter() {
        if let Ok(message) = message_string.parse::<Message>() {
            messages_parsed.push(message);
        }
    }
    println!("10000 strings to messages took {} micros.", now.elapsed().as_micros());
    let mut parsed_message_strings = Vec::new();
    let now = Instant::now();
    if let Ok(_) = std::fs::write("message_test", message_strings.join("\n")) {
        println!("Writing 10000 messages to file took {} micros.", now.elapsed().as_micros());
        let now = Instant::now();
        if let Ok(string) = std::fs::read_to_string("message_test") {
            for message_string in string.split('\n').into_iter() {
                parsed_message_strings.push(format!("{:?}", message_string));
            }
        }
        println!("Reading 10000 messages from file took {} micros.", now.elapsed().as_micros());
    }
}
