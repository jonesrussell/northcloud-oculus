//! Redis Pub/Sub feed for north-cloud publisher messages.
//!
//! Subscriber thread is fire-and-forget: no join handle or cancellation token;
//! it is terminated when the process exits.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

/// Minimal fields we need for the VR feed. Serde ignores the rest of the JSON.
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

/// Configuration for the Redis live feed (from env).
#[derive(Clone, Debug)]
pub struct RedisFeedConfig {
    pub addr: String,
    pub password: Option<String>,
    pub channels: Vec<String>,
    pub max_items: usize,
}

impl RedisFeedConfig {
    /// Load config from environment. Returns `None` if REDIS_CHANNELS is missing, empty, or malformed.
    pub fn from_env() -> Option<Self> {
        let addr = std::env::var("REDIS_ADDR").unwrap_or_else(|_| "127.0.0.1:6379".to_string());
        let password = std::env::var("REDIS_PASSWORD").ok();
        let channels_var = std::env::var("REDIS_CHANNELS").ok()?;
        let channels: Vec<String> = channels_var
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if channels.is_empty() {
            eprintln!("REDIS_CHANNELS empty or invalid, live feed disabled");
            return None;
        }
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

/// Receiver is only read by the drain system; wrapped in Mutex so LiveFeedBuffer is Sync.
#[derive(Debug)]
pub struct RedisReceiver(pub Mutex<mpsc::Receiver<LiveArticle>>);

/// Bounded buffer of live articles plus optional receiver to drain.
/// When disabled (no Redis), receiver is None and drain is a no-op.
#[derive(Debug, bevy::prelude::Resource)]
pub struct LiveFeedBuffer {
    pub items: VecDeque<LiveArticle>,
    pub max_items: usize,
    /// Only the drain system reads from this; no other system touches it.
    /// None when live feed is disabled.
    pub receiver: Option<RedisReceiver>,
}

impl LiveFeedBuffer {
    pub fn new(receiver: mpsc::Receiver<LiveArticle>, max_items: usize) -> Self {
        LiveFeedBuffer {
            items: VecDeque::new(),
            max_items: max_items.max(1),
            receiver: Some(RedisReceiver(Mutex::new(receiver))),
        }
    }

    /// Disabled buffer (no Redis); drain is a no-op.
    pub fn disabled(max_items: usize) -> Self {
        LiveFeedBuffer {
            items: VecDeque::new(),
            max_items: max_items.max(1),
            receiver: None,
        }
    }

    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.receiver.is_some()
    }

    /// Push one item and truncate from the front if over capacity. O(1).
    pub fn push(&mut self, item: LiveArticle) {
        self.items.push_back(item);
        while self.items.len() > self.max_items {
            self.items.pop_front();
        }
    }

    /// Drain available messages from the receiver and push onto the buffer. No-op when disabled.
    pub fn drain_receiver(&mut self) {
        if let Some(r) = &self.receiver {
            let guard = match r.0.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let items: Vec<LiveArticle> = guard.try_iter().collect();
            drop(guard);
            for item in items {
                self.push(item);
            }
        }
    }
}

/// Rate-limited parse error logging: log at most once per interval or every N failures.
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

/// Spawns the subscriber thread (fire-and-forget) and returns the receiver.
/// On connection or subscribe failure, logs once and returns `None`.
pub fn spawn_subscriber(config: RedisFeedConfig) -> Option<mpsc::Receiver<LiveArticle>> {
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

fn run_subscriber_loop(
    addr: String,
    password: Option<String>,
    channels: Vec<String>,
    tx: mpsc::SyncSender<LiveArticle>,
) {
    let url = match &password {
        Some(p) => format!("redis://:{}@{}", p, addr),
        None => format!("redis://{}", addr),
    };
    let client = match redis::Client::open(url.as_str()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[redis_feed] Redis connection failed, live feed disabled: {}", e);
            return;
        }
    };

    let mut conn = match client.get_connection() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[redis_feed] Redis connection failed, live feed disabled: {}", e);
            return;
        }
    };

    let mut pubsub = conn.as_pubsub();

    for ch in &channels {
        if let Err(e) = pubsub.subscribe(ch) {
            eprintln!(
                "[redis_feed] Redis subscribe to {:?} failed, live feed disabled: {}",
                ch, e
            );
            return;
        }
    }

    let mut last_log = Instant::now();
    let parse_error_count = AtomicU32::new(0);

    loop {
        let msg: redis::Msg = match pubsub.get_message() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[redis_feed] get_message error: {}", e);
                continue;
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
        if tx.send(article).is_err() {
            break;
        }
    }
}
