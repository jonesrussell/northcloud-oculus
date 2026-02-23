//! Dev-only: publishes random JSON messages to Redis so you can test the live feed
//! without a real north-cloud backend.
//!
//! Run: `cargo run --bin redis_dev_publisher`
//!
//! Env:
//!   REDIS_ADDR          default 127.0.0.1:6379
//!   REDIS_PASSWORD      optional
//!   REDIS_CHANNEL       channel to publish to (default: test — matches app when REDIS_CHANNELS unset)
//!   PUBLISH_INTERVAL_SECS  seconds between messages (default: 3)

use redis::Commands;
use rand::Rng;
use serde::Serialize;
use std::env;
use std::time::Duration;

#[derive(Serialize)]
struct PublisherMeta {
    channel: Option<String>,
    published_at: Option<String>,
}

#[derive(Serialize)]
struct DevArticleMessage {
    id: Option<String>,
    title: Option<String>,
    quality_score: Option<i32>,
    topics: Option<Vec<String>>,
    publisher: Option<PublisherMeta>,
}

const TITLES: &[&str] = &[
    "Breaking: Widget demand soars",
    "Market update: Q3 outlook",
    "New study on message buses",
    "Dev pipeline throughput up",
    "Redis pub/sub in production",
    "VR feed demo message",
    "Simulated article #",
    "Lorem headline generator",
    "Random event stream",
    "Test message from publisher",
];

fn main() {
    let addr = env::var("REDIS_ADDR").unwrap_or_else(|_| "127.0.0.1:6379".to_string());
    let password = env::var("REDIS_PASSWORD").ok();
    let channel = env::var("REDIS_CHANNEL").unwrap_or_else(|_| "test".to_string());
    let interval_secs: u64 = env::var("PUBLISH_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3)
        .max(1);

    eprintln!("[redis_dev_publisher] Connecting to {} …", addr);
    let client = redis::Client::open(format!("redis://{}", addr)).expect("Redis client");
    let mut conn = loop {
        match client.get_connection() {
            Ok(c) => break c,
            Err(e) => {
                eprintln!("[redis_dev_publisher] Connect failed: {}. Retrying in 2s …", e);
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    };
    if let Some(ref p) = password {
        let _: () = redis::cmd("AUTH").arg(p).query(&mut conn).expect("AUTH");
    }
    eprintln!(
        "[redis_dev_publisher] Publishing to channel {:?} every {}s",
        channel, interval_secs
    );

    let mut rng = rand::thread_rng();
    let mut n: u32 = 0;
    loop {
        n += 1;
        let title_idx = rng.gen_range(0..TITLES.len());
        let title = if TITLES[title_idx].ends_with('#') {
            format!("{} {}", TITLES[title_idx], n)
        } else {
            TITLES[title_idx].to_string()
        };
        let quality = rng.gen_range(1..=100);
        let now = chrono_iso();
        let msg = DevArticleMessage {
            id: Some(format!("dev-{}", n)),
            title: Some(title),
            quality_score: Some(quality),
            topics: Some(vec!["dev".to_string(), "test".to_string()]),
            publisher: Some(PublisherMeta {
                channel: Some(channel.clone()),
                published_at: Some(now),
            }),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        if let Err(e) = conn.publish::<_, _, ()>(channel.as_str(), json.as_str()) {
            eprintln!("[redis_dev_publisher] PUBLISH error: {}", e);
        }
        std::thread::sleep(Duration::from_secs(interval_secs));
    }
}

fn chrono_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        1970 + (t / 31536000),
        ((t % 31536000) / 86400) / 31 + 1,
        (t % 86400) / 86400 * 28 + 1,
        (t % 86400) / 3600,
        (t % 3600) / 60,
        t % 60
    )
}
