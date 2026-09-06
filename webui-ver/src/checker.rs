use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Entry {
    pub zh: String,
    pub stem: String,
    pub path: PathBuf,
}

pub struct ResourceDb {
    pub maps: Vec<Entry>,
    pub survivors: Vec<Entry>,
}

fn scan(dir: &Path) -> io::Result<Vec<Entry>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let p = entry?.path();
        if p.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        if stem.eq_ignore_ascii_case("readme") {
            continue;
        }
        let text = fs::read_to_string(&p).unwrap_or_default();
        let zh = text
            .lines()
            .next()
            .unwrap_or("")
            .trim_start_matches('#')
            .trim()
            .to_string();
        let zh = if zh.is_empty() { stem.clone() } else { zh };
        out.push(Entry {
            zh,
            stem: stem.to_lowercase(),
            path: p,
        });
    }
    out.sort_by(|a, b| a.zh.cmp(&b.zh));
    Ok(out)
}

impl ResourceDb {
    pub fn load(root: &Path) -> io::Result<Self> {
        let maps = scan(&root.join("maps"))?;
        let survivors = scan(&root.join("characters"))?;
        if maps.is_empty() || survivors.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("资源目录 {} 下未找到地图或求生者数据", root.display()),
            ));
        }
        Ok(ResourceDb { maps, survivors })
    }

    pub fn find_map(&self, input: &str) -> Option<&Entry> {
        let t = input.trim();
        if t.is_empty() {
            return None;
        }
        let lower = t.to_lowercase();
        self.maps
            .iter()
            .find(|e| e.zh == t)
            .or_else(|| self.maps.iter().find(|e| e.stem == lower))
    }

    pub fn find_character(&self, input: &str) -> Option<&Entry> {
        let t = input.trim();
        if t.is_empty() {
            return None;
        }
        let lower = t.to_lowercase();
        self.survivors
            .iter()
            .find(|e| e.zh == t)
            .or_else(|| self.survivors.iter().find(|e| e.stem == lower))
    }

    pub fn maps_hint(&self) -> String {
        self.maps
            .iter()
            .map(|e| e.zh.as_str())
            .collect::<Vec<_>>()
            .join("、")
    }

    pub fn survivors_hint(&self) -> String {
        self.survivors
            .iter()
            .map(|e| e.zh.as_str())
            .collect::<Vec<_>>()
            .join("、")
    }
}
