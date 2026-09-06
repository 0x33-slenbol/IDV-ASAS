use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct MapPosition {
    pub zh: String,
    pub map_width: u32,
    pub map_height: u32,
    pub points: Vec<(u32, u32, u32)>,
    pub icon_size: u32,
}

impl MapPosition {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("读取选点数据失败：{e}"))?;
        let mut zh = String::new();
        let mut map_width = 0u32;
        let mut map_height = 0u32;
        let mut points = Vec::new();
        let mut icon_size = 0u32;

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("## ") {
                zh = line.trim_start_matches('#').trim().to_string();
            }
            if line.contains("像素") && (line.contains("×") || line.contains("\\times")) {
                let s = line.trim_start_matches('#').trim();
                if s.contains("角色图标") {
                    if let Some(sz) = parse_icon_size(s) {
                        icon_size = sz;
                    }
                } else {
                    if let Some((w, h)) = parse_dims(s) {
                        map_width = w;
                        map_height = h;
                    }
                }
            }
            if line.contains("号选点中心") {
                if let Some((num, x, y)) = parse_point_line(line) {
                    points.push((num, x, y));
                }
            }
        }
        if zh.is_empty() {
            zh = path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string();
        }
        if points.is_empty() {
            return Err(format!("选点数据文件 {} 中未找到选点中心坐标", path.display()));
        }
        if icon_size == 0 {
            icon_size = if map_width > 0 {
                (map_width / 7).max(60)
            } else {
                100
            };
        }
        Ok(MapPosition {
            zh,
            map_width,
            map_height,
            points,
            icon_size,
        })
    }

    pub fn max_point(&self) -> u32 {
        self.points.iter().map(|(n, _, _)| *n).max().unwrap_or(0)
    }

    pub fn point_center(&self, num: u32) -> Option<(u32, u32)> {
        self.points
            .iter()
            .find(|(n, _, _)| *n == num)
            .map(|(_, x, y)| (*x, *y))
    }
}

fn parse_dims(s: &str) -> Option<(u32, u32)> {
    let s = s.trim_start_matches('#').trim();
    let parts: Vec<&str> = s.split("像素").collect();
    let front = parts.first()?;
    let front = front.rsplit("为").next()?;
    let front = front.replace("\\times", "×");
    let nums: Vec<u32> = front
        .split(['×', 'x', '*'])
        .filter_map(|p| p.trim().trim_matches('$').trim_matches(['(', ')']).trim().parse::<u32>().ok())
        .collect();
    if nums.len() >= 2 {
        Some((nums[0], nums[1]))
    } else {
        None
    }
}

fn parse_icon_size(s: &str) -> Option<u32> {
    if !s.contains("角色图标") {
        return None;
    }
    let parts: Vec<&str> = s.split("像素").collect();
    let front = parts.first()?;
    let front = front.replace("\\times", "×");
    let nums: Vec<u32> = front
        .split(['×', 'x', '*'])
        .filter_map(|p| {
            p.trim()
                .trim_matches('$')
                .trim()
                .trim_matches(['(', ')'])
                .trim()
                .parse::<u32>()
                .ok()
        })
        .collect();
    nums.first().copied()
}

fn parse_point_line(line: &str) -> Option<(u32, u32, u32)> {
    let num_end = line.find("号选点")?;
    let num_str = &line[..num_end];
    let num: u32 = num_str.trim().parse().ok()?;

    let coord_start = line.find("$(")?;
    let coord_end = line[coord_start..].find(")$")?;
    let coord_str = &line[coord_start + 2..coord_start + coord_end];
    let parts: Vec<&str> = coord_str.split(',').collect();
    if parts.len() < 2 {
        return None;
    }
    let x: u32 = parts[0].trim().parse().ok()?;
    let y: u32 = parts[1].trim().parse().ok()?;
    Some((num, x, y))
}
