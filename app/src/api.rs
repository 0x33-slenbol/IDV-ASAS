use crate::config::ApiConfig;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.prompt_tokens + self.completion_tokens
    }
    pub fn cost(&self, price_in: f64, price_out: f64) -> f64 {
        self.prompt_tokens as f64 / 1_000_000.0 * price_in
            + self.completion_tokens as f64 / 1_000_000.0 * price_out
    }
}

pub struct StreamOutcome {
    pub content: String,
    pub usage: Option<Usage>,
    pub interrupted: bool,
    pub elapsed: Duration,
}

pub enum ApiEvent {
    Delta(String),
    Done(Result<StreamOutcome, String>),
}

pub fn chat_stream_threaded(
    cfg: ApiConfig,
    messages: Vec<Value>,
) -> (mpsc::Receiver<ApiEvent>, Arc<AtomicBool>) {
    let (tx, rx) = mpsc::channel::<ApiEvent>();
    let interrupt = Arc::new(AtomicBool::new(false));
    let int = interrupt.clone();
    thread::spawn(move || {
        let outcome = match attempt(&cfg, &messages, &int, true) {
            Err(e) if e.contains("stream_options") => {
                attempt(&cfg, &messages, &int, false)
            }
            other => other,
        };
        let _ = tx.send(ApiEvent::Done(outcome));
    });
    (rx, interrupt)
}

fn attempt(
    cfg: &ApiConfig,
    messages: &[Value],
    interrupt: &AtomicBool,
    include_usage: bool,
) -> Result<StreamOutcome, String> {
    let mut body = json!({
        "model": cfg.model,
        "messages": messages,
        "temperature": crate::config::round2(cfg.temperature),
        "stream": true,
    });
    if include_usage {
        body["stream_options"] = json!({ "include_usage": true });
    }
    if let Some(mt) = cfg.max_tokens {
        body["max_tokens"] = json!(mt);
    }

    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .new_agent();
    let mut req = agent
        .post(&cfg.endpoint())
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");
    if !cfg.api_key.trim().is_empty() {
        req = req.header("Authorization", &format!("Bearer {}", cfg.api_key.trim()));
    }

    let mut res = req
        .send(body.to_string())
        .map_err(|e| format!("网络请求失败：{e}"))?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.body_mut().read_to_string().unwrap_or_default();
            return Err(format!("接口返回错误 HTTP {status}：{}", truncate_chars(&text, 600)));
        }

    let reader = res.into_body().into_reader();
    let (tx, rx) = mpsc::sync_channel::<Option<String>>(128);
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = tx.send(None);
                    break;
                }
                Ok(_) => {
                    let l = line.trim_end_matches(['\r', '\n']).to_string();
                    if tx.send(Some(l)).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = tx.send(None);
                    break;
                }
            }
        }
    });

    let start = Instant::now();
    let mut content = String::new();
    let mut usage = None;
    let mut got_first = false;
    let mut interrupted = false;

    'outer: loop {
        let got = match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(v) => v,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if interrupt.load(Ordering::Relaxed) {
                    interrupted = true;
                    break 'outer;
                }
                if !got_first && start.elapsed() > Duration::from_secs(120) {
                    return Err("等待模型响应超时（120 秒未收到任何输出）。".to_string());
                }
                continue 'outer;
            }
            Err(_) => break 'outer,
        };
        let line = match got {
            Some(l) => l,
            None => break 'outer,
        };
        if interrupt.load(Ordering::Relaxed) {
            interrupted = true;
            break 'outer;
        }
        if !line.starts_with("data:") {
            continue;
        }
        let payload = line["data:".len()..].trim();
        if payload == "[DONE]" {
            break 'outer;
        }
        let Ok(v) = serde_json::from_str::<Value>(payload) else {
            continue;
        };
        if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
            usage = Some(Usage {
                prompt_tokens: u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
                completion_tokens: u.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0),
            });
        }
        if let Some(c) = v
            .pointer("/choices/0/delta/content")
            .and_then(Value::as_str)
        {
            if !c.is_empty() {
                got_first = true;
                content.push_str(c);
            }
        }
    }

    Ok(StreamOutcome {
        content,
        usage,
        interrupted,
        elapsed: start.elapsed(),
    })
}

pub fn truncate_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{t}…")
    }
}
