//! Redis Pub/Sub feed for north-cloud publisher messages.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RedisConnectionStatus {
    #[default]
    Disabled,
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Clone, Debug)]
pub struct LiveArticle {
    #[allow(dead_code)]
    pub id: String,
    pub title: String,
    pub channel: String,
    pub quality_score: Option<i32>,
    #[allow(dead_code)]
    pub published_at: String,
    #[allow(dead_code)]
    pub topics: Vec<String>,
}

#[derive(Deserialize)]
struct PublisherMeta {
    #[serde(rename = "channel")]
    channel: Option<String>,
    #[serde(rename = "published_at")]
    published_at: Option<String>,
}

#[derive(Deserialize)]
struct LiveArticleRaw {
    id: Option<String>,
    title: Option<String>,
    quality_score: Option<i32>,
    topics: Option<Vec<String>>,
    publisher: Option<PublisherMeta>,
}

impl LiveArticle {
    fn from_raw(raw: LiveArticleRaw, channel_from_msg: String) -> Self {
        let publisher = raw.publisher.as_ref();
        LiveArticle {
            id: raw.id.unwrap_or_default(),
            title: raw.title.unwrap_or_default(),
            channel: publisher
                .and_then(|p| p.channel.clone())
                .unwrap_or(channel_from_msg),
            quality_score: raw.quality_score,
            published_at: publisher
                .and_then(|p| p.published_at.clone())
                .unwrap_or_else(|| "".to_string()),
            topics: raw.topics.unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RedisFeedConfig {
    pub addr: String,
    pub password: Option<String>,
    pub channels: Vec<String>,
    pub max_items: usize,
}

/// Default channel when REDIS_CHANNELS is not set — status bar still reflects connection state.
const DEFAULT_CHANNEL: &str = "test";

impl RedisFeedConfig {
    pub fn from_env() -> Option<Self> {
        let addr = std::env::var("REDIS_ADDR").unwrap_or_else(|_| "127.0.0.1:6379".to_string());
        let password = std::env::var("REDIS_PASSWORD").ok();
        let channels: Vec<String> = match std::env::var("REDIS_CHANNELS") {
            Ok(v) => {
                let list: Vec<String> = v
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if list.is_empty() {
                    eprintln!("REDIS_CHANNELS empty or invalid, using default channel {:?}", DEFAULT_CHANNEL);
                    vec![DEFAULT_CHANNEL.to_string()]
                } else {
                    list
                }
            }
            Err(_) => vec![DEFAULT_CHANNEL.to_string()],
        };
        let max_items = std::env::var("REDIS_MAX_ITEMS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20)
            .max(1);

        Some(RedisFeedConfig {
            addr,
            password,
            channels,
            max_items,
        })
    }
}

pub enum FeedMessage {
    Connected,
    Disconnected,
    Article(LiveArticle),
}

#[derive(Debug)]
pub struct RedisReceiver(pub Mutex<mpsc::Receiver<FeedMessage>>);

/// Max number of "just received" articles to hand to the VR animation system per drain.
const RECENTLY_RECEIVED_CAP: usize = 8;

#[derive(Debug, bevy::prelude::Resource)]
pub struct LiveFeedBuffer {
    pub items: VecDeque<LiveArticle>,
    pub max_items: usize,
    pub connection_status: RedisConnectionStatus,
    pub receiver: Option<RedisReceiver>,
    /// Articles received this drain; consumed by VR animation to spawn flying message cards.
    pub recently_received: VecDeque<LiveArticle>,
}

impl LiveFeedBuffer {
    pub fn new(receiver: mpsc::Receiver<FeedMessage>, max_items: usize) -> Self {
        LiveFeedBuffer {
            items: VecDeque::new(),
            max_items: max_items.max(1),
            connection_status: RedisConnectionStatus::Connecting,
            receiver: Some(RedisReceiver(Mutex::new(receiver))),
            recently_received: VecDeque::new(),
        }
    }

    pub fn disabled(max_items: usize) -> Self {
        LiveFeedBuffer {
            items: VecDeque::new(),
            max_items: max_items.max(1),
            connection_status: RedisConnectionStatus::Disabled,
            receiver: None,
            recently_received: VecDeque::new(),
        }
    }

    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.receiver.is_some()
    }

    pub fn push(&mut self, item: LiveArticle) {
        self.items.push_back(item);
        while self.items.len() > self.max_items {
            self.items.pop_front();
        }
    }

    pub fn drain_receiver(&mut self) {
        if let Some(r) = &self.receiver {
            let guard = match r.0.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let messages: Vec<FeedMessage> = guard.try_iter().collect();
            drop(guard);
            for msg in messages {
                match msg {
                    FeedMessage::Connected => {
                        eprintln!("[redis_feed] Received Connected, status -> green");
                        self.connection_status = RedisConnectionStatus::Connected;
                    }
                    FeedMessage::Disconnected => {
                        eprintln!("[redis_feed] Received Disconnected, status -> red");
                        self.connection_status = RedisConnectionStatus::Disconnected;
                    }
                    FeedMessage::Article(a) => {
                        self.push(a.clone());
                        self.recently_received.push_back(a);
                        while self.recently_received.len() > RECENTLY_RECEIVED_CAP {
                            self.recently_received.pop_front();
                        }
                    }
                }
            }
        }
    }
}

const PARSE_LOG_INTERVAL: Duration = Duration::from_secs(5);
const PARSE_LOG_EVERY_N: u32 = 50;

fn log_parse_error(last_log: &mut Instant, count: &AtomicU32, channel: &str, err: &str) {
    let n = count.fetch_add(1, Ordering::Relaxed) + 1;
    let now = Instant::now();
    if now.duration_since(*last_log) >= PARSE_LOG_INTERVAL || n == 1 || n % PARSE_LOG_EVERY_N == 0 {
        eprintln!(
            "[redis_feed] JSON parse error on channel {:?} (total errors ~{}): {}",
            channel, n, err
        );
        *last_log = now;
    }
}

pub fn spawn_subscriber(config: RedisFeedConfig) -> Option<mpsc::Receiver<FeedMessage>> {
    let (tx, rx) = mpsc::sync_channel(128);

    let addr = config.addr.clone();
    let password = config.password.clone();
    let channels = config.channels.clone();

    std::thread::Builder::new()
        .name("redis_feed".into())
        .spawn(move || {
            run_subscriber_loop(addr, password, channels, tx);
        })
        .ok()?;

    Some(rx)
}

const CONNECT_RETRY_ATTEMPTS: u32 = 10;
const CONNECT_RETRY_DELAY_MS: u64 = 500;

fn run_subscriber_loop(
    addr: String,
    password: Option<String>,
    channels: Vec<String>,
    tx: mpsc::SyncSender<FeedMessage>,
) {
    let url = match &password {
        Some(p) => format!("redis://:{}@{}", p, addr),
        None => format!("redis://{}", addr),
    };
    let client = match redis::Client::open(url.as_str()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[redis_feed] Redis client open failed: {}", e);
            let _ = tx.send(FeedMessage::Disconnected);
            return;
        }
    };

    let mut conn = 'connect: loop {
        match client.get_connection() {
            Ok(c) => break c,
            Err(e) => {
                for attempt in 1..=CONNECT_RETRY_ATTEMPTS {
                    eprintln!(
                        "[redis_feed] Redis connection attempt {} failed: {} (retrying in {}ms)",
                        attempt, e, CONNECT_RETRY_DELAY_MS
                    );
                    std::thread::sleep(std::time::Duration::from_millis(CONNECT_RETRY_DELAY_MS));
                    if let Ok(c) = client.get_connection() {
                        eprintln!("[redis_feed] Redis connected on retry {}", attempt);
                        break 'connect c;
                    }
                }
                eprintln!("[redis_feed] Redis connection failed after {} retries", CONNECT_RETRY_ATTEMPTS);
                let _ = tx.send(FeedMessage::Disconnected);
                return;
            }
        }
    };

    let mut pubsub = conn.as_pubsub();

    for ch in &channels {
        if let Err(e) = pubsub.subscribe(ch) {
            eprintln!(
                "[redis_feed] Redis subscribe to {:?} failed, live feed disabled: {}",
                ch, e
            );
            let _ = tx.send(FeedMessage::Disconnected);
            return;
        }
    }
    let _ = tx.send(FeedMessage::Connected);

    let mut last_log = Instant::now();
    let parse_error_count = AtomicU32::new(0);

    loop {
        let msg: redis::Msg = match pubsub.get_message() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[redis_feed] get_message error: {}", e);
                let _ = tx.send(FeedMessage::Disconnected);
                break;
            }
        };
        let channel_name = msg.get_channel_name().to_string();
        let payload: Vec<u8> = match msg.get_payload::<Vec<u8>>() {
            Ok(p) => p,
            Err(e) => {
                let err_str = e.to_string();
                log_parse_error(&mut last_log, &parse_error_count, &channel_name, &err_str);
                continue;
            }
        };
        let s = match String::from_utf8(payload) {
            Ok(x) => x,
            Err(e) => {
                log_parse_error(
                    &mut last_log,
                    &parse_error_count,
                    &channel_name,
                    &e.to_string(),
                );
                continue;
            }
        };
        let raw: LiveArticleRaw = match serde_json::from_str(&s) {
            Ok(x) => x,
            Err(e) => {
                log_parse_error(
                    &mut last_log,
                    &parse_error_count,
                    &channel_name,
                    &e.to_string(),
                );
                continue;
            }
        };
        let article = LiveArticle::from_raw(raw, channel_name);
        if tx.send(FeedMessage::Article(article)).is_err() {
            break;
        }
    }
}
