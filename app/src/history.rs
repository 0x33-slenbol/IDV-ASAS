use crate::api::Usage;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
pub struct RoundRecord {
    pub index: usize,
    pub map: String,
    pub survivors: Vec<String>,
    pub round_req: String,
    pub compact_request: String,
    pub assistant_reply: String,
    pub interrupted: bool,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub estimated: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Session {
    pub version: u32,
    pub created_at_unix: u64,
    pub base_url: String,
    pub model: String,
    pub global_req: String,
    pub rounds: Vec<RoundRecord>,
}

impl Session {
    pub fn totals(&self) -> (u64, u64, f64) {
        let mut pt = 0u64;
        let mut ct = 0u64;
        let mut cost = 0.0;
        for r in &self.rounds {
            if let Some(u) = r.usage {
                pt += u.prompt_tokens;
                ct += u.completion_tokens;
            }
            cost += r.cost;
        }
        (pt, ct, cost)
    }
}

pub fn now_name() -> String {
    Local::now().format("%Y%m%d-%H%M%S").to_string()
}

pub fn path_for(dir: &Path, name: &str) -> PathBuf {
    let mut n: String = name
        .trim()
        .trim_end_matches(".json")
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    n = n.trim_matches('.').to_string();
    if n.is_empty() {
        n = now_name();
    }
    dir.join(format!("{n}.json"))
}

pub fn list(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect(),
        Err(_) => Vec::new(),
    };
    v.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    v
}

pub fn save(path: &Path, session: &Session) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建历史目录失败：{e}"))?;
    }
    let text = serde_json::to_string_pretty(session).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| format!("写入会话失败：{e}"))
}

pub fn load(path: &Path) -> Result<Session, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("读取会话失败：{e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("会话文件损坏：{e}"))
}
