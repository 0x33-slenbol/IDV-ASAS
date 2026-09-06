use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

fn default_temperature() -> f64 {
    0.7
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub price_input_per_m: f64,
    #[serde(default)]
    pub price_output_per_m: f64,
}

impl Default for ApiConfig {
    fn default() -> Self {
        ApiConfig {
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            temperature: default_temperature(),
            max_tokens: None,
            price_input_per_m: 0.0,
            price_output_per_m: 0.0,
        }
    }
}

pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

impl ApiConfig {
    pub fn endpoint(&self) -> String {
        let mut u = self.base_url.trim().trim_end_matches('/').to_string();
        if u.ends_with("/chat/completions") {
            u.truncate(u.len() - "/chat/completions".len());
        }
        format!("{u}/chat/completions")
    }

    pub fn masked_key(&self) -> String {
        let k = self.api_key.trim();
        if k.is_empty() {
            String::new()
        } else {
            let n = k.chars().count();
            if n > 10 {
                let head: String = k.chars().take(4).collect();
                let tail: String = k.chars().skip(n - 4).collect();
                format!("{head}***{tail}")
            } else {
                "***".to_string()
            }
        }
    }
}

pub fn load(path: &Path) -> Option<ApiConfig> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save(path: &Path, cfg: &ApiConfig) -> Result<(), String> {
    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}
