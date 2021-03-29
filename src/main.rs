use std::{path::Path, process::exit, time::Instant};

use rand::Rng;
use tantivy::{Index, ReloadPolicy, collector, doc, query::{Query, QueryParser}, schema::{Schema, STORED, TEXT}};
use wicrs_server::{config::Config, httpapi::server, new_id, ID};

fn load_config(path: &str) -> Config {
    if let Ok(read) = std::fs::read_to_string(path) {
        if let Ok(config) = serde_json::from_str::<Config>(&read) {
            return config;
        } else {
            println!("config.json does not contain a valid configuration.");
            exit(1);
        }
    } else {
        println!("Failed to load config.json.");
        exit(1);
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = load_config("config.json");
    println!("Adding benchmark: ");
    adding_bench().await;
    println!("Getting benchmark: ");
    getting_bench().await;
    server(config).await
}

async fn getting_bench() {
    let index_path = Path::new("test_index");
    let index = Index::open_in_dir(&index_path).unwrap();
    let message_id = index.schema().get_field("id").unwrap();
    let message_content = index.schema().get_field("content").unwrap();
    let message_sender = index.schema().get_field("sender").unwrap();
    let message_created = index.schema().get_field("created").unwrap();
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommit)
        .try_into()
        .unwrap();
    let searcher = reader.searcher();
    let parser = QueryParser::for_index(&index, vec![message_content]);
    let result = searcher.search(&parser.parse_query("a").unwrap(), &collector::TopDocs::with_limit(10)).unwrap();
    dbg!(result);
}

async fn adding_bench() {
    let mut messages: Vec<(String, ID)> = Vec::new();
    for _ in 0..1_000 {
        let mut random: [char; 128] = ['0'; 128];
        for b in random.iter_mut() {
            *b = rand::thread_rng().gen::<char>();
        }
        messages.push((random.iter().collect(), new_id()));
    }
    let channel = wicrs_server::channel::Channel::new("test".to_string(), ID::nil(), ID::nil());
    let now = Instant::now();
    for (message, id) in &messages {
        let _ = channel
            .add_message(wicrs_server::channel::Message {
                id: id.clone(),
                sender: ID::nil(),
                created: 0,
                content: message.clone(),
            })
            .await;
    }
    println!("wicrs took {}us", now.elapsed().as_micros());
    let index_path = Path::new("test_index");
    let _ = std::fs::create_dir(index_path);
    let mut schema_builder = Schema::builder();
    schema_builder.add_i64_field("id", STORED);
    schema_builder.add_i64_field("sender", STORED);
    schema_builder.add_date_field("created", STORED);
    schema_builder.add_text_field("content", TEXT | STORED);
    let schema = schema_builder.build();
    let index = Index::create_in_dir(&index_path, schema.clone()).unwrap();
    let message_id = schema.get_field("id").unwrap();
    let message_content = schema.get_field("content").unwrap();
    let message_sender = schema.get_field("sender").unwrap();
    let message_created = schema.get_field("created").unwrap();
    let now = Instant::now();
    let mut index_writer = index.writer(50_000_000).unwrap();
    for (message, id) in &messages {
        index_writer.add_document(doc!(
            message_id => id.clone().as_u128() as i64,
            message_content => message.clone(),
            message_sender => ID::nil().as_u128() as i64,
            message_created => 0_i64,
        ));
    }
    index_writer.commit().unwrap();
    println!("tantivy took {}us", now.elapsed().as_micros());
}
