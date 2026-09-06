use crate::checker::ResourceDb;

#[derive(Clone, Debug)]
pub struct Plan {
    pub index: usize,
    pub selections: Vec<Selection>,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct Selection {
    pub point: u32,
    pub character: String,
    pub talents: Vec<String>,
}

pub enum ParseError {
    NotEnoughPlans,
    InvalidCharacter(String),
    InvalidTalent(String),
    WrongTalentCount(String, usize),
    PointOutOfRange(u32, u32),
    DuplicatePoint(u32),
    WrongCharacterCount(usize),
    NoValidPlans,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::NotEnoughPlans => write!(f, "模型未返回任何有效方案"),
            ParseError::InvalidCharacter(n) => write!(f, "角色「{n}」不存在"),
            ParseError::InvalidTalent(n) => write!(f, "天赋「{n}」不存在"),
            ParseError::WrongTalentCount(c, n) => write!(f, "角色「{c}」的天赋数量为 {n}，应为 2"),
            ParseError::PointOutOfRange(p, max) => write!(f, "选点编号 {p} 超出地图范围（最大 {max}）"),
            ParseError::DuplicatePoint(p) => write!(f, "选点编号 {p} 重复"),
            ParseError::WrongCharacterCount(n) => write!(f, "方案中角色数量为 {n}，应为 4"),
            ParseError::NoValidPlans => write!(f, "无有效方案"),
        }
    }
}

const VALID_TALENTS: &[&str] = &["回光返照", "飞轮效应", "膝跳反射", "化险为夷"];

pub fn parse_and_validate(
    raw: &str,
    db: &ResourceDb,
    max_point: u32,
) -> Result<Vec<Plan>, ParseError> {
    let plans = split_plans(raw);
    if plans.is_empty() {
        return Err(ParseError::NotEnoughPlans);
    }
    let mut valid = Vec::new();
    for (i, plan_text) in plans.iter().enumerate() {
        match parse_single_plan(plan_text, db, max_point) {
            Ok(p) => valid.push(p),
            Err(e) => {
                if i == 0 {
                    return Err(e);
                }
            }
        }
    }
    if valid.is_empty() {
        return Err(ParseError::NoValidPlans);
    }
    for p in &mut valid {
        let d = p.description.trim();
        if d.chars().count() > 100 {
            p.description = truncate_to_sentence(d, 100);
        }
    }
    Ok(valid)
}

fn split_plans(raw: &str) -> Vec<String> {
    let mut plans = Vec::new();
    let mut current = String::new();
    let mut in_plan = false;
    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with("方案") && line.contains("：") {
            if in_plan && !current.is_empty() {
                plans.push(current.trim().to_string());
                current.clear();
            }
            in_plan = true;
            current.push_str(line);
            current.push('\n');
        } else if in_plan {
            current.push_str(line);
            current.push('\n');
        }
    }
    if in_plan && !current.trim().is_empty() {
        plans.push(current.trim().to_string());
    }
    plans
}

fn parse_single_plan(text: &str, db: &ResourceDb, max_point: u32) -> Result<Plan, ParseError> {
    let mut selections = Vec::new();
    let mut description = String::new();
    let mut desc_found = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("方案") {
            continue;
        }
        if line.starts_with("具体描述") || line.starts_with("描述及分析") || line.starts_with("描述") {
            desc_found = true;
            if let Some(colon_pos) = line.find('：') {
                description = line[colon_pos + '：'.len_utf8()..].trim().to_string();
            } else {
                description = line.to_string();
            }
            continue;
        }
        if desc_found {
            if !description.is_empty() {
                description.push('\n');
            }
            description.push_str(line);
            continue;
        }
        if let Some(sel) = parse_selection_line(line) {
            selections.push(sel);
        }
    }

    if selections.len() != 4 {
        return Err(ParseError::WrongCharacterCount(selections.len()));
    }
    let mut seen_points = std::collections::HashSet::new();
    for sel in &selections {
        if db.find_character(&sel.character).is_none() {
            return Err(ParseError::InvalidCharacter(sel.character.clone()));
        }
        if sel.talents.len() != 2 {
            return Err(ParseError::WrongTalentCount(
                sel.character.clone(),
                sel.talents.len(),
            ));
        }
        for t in &sel.talents {
            if !VALID_TALENTS.contains(&t.as_str()) {
                return Err(ParseError::InvalidTalent(t.clone()));
            }
        }
        if sel.point > max_point || sel.point == 0 {
            return Err(ParseError::PointOutOfRange(sel.point, max_point));
        }
        if !seen_points.insert(sel.point) {
            return Err(ParseError::DuplicatePoint(sel.point));
        }
    }
    Ok(Plan {
        index: 0,
        selections,
        description,
    })
}

fn parse_selection_line(line: &str) -> Option<Selection> {
    let line = line.trim();
    let line = line.trim_end_matches('。').trim_end_matches('.');
    let point_end = line.find("号选点")?;
    let point_str = &line[..point_end];
    let point: u32 = point_str.trim().parse().ok()?;
    let rest = &line[point_end + "号选点".len()..];
    let rest = rest.trim_start_matches(['：', ':', ' ']).trim();
    let rest = rest.trim_start_matches("放置").trim();
    let colon_pos = rest.find('：').or_else(|| rest.find(':'))?;
    let character = rest[..colon_pos].trim().to_string();
    let talents_str = rest[colon_pos + '：'.len_utf8()..].trim();
    let talents: Vec<String> = talents_str
        .split(['、', ',', '，', '/', ' '])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Some(Selection {
        point,
        character,
        talents,
    })
}

fn truncate_to_sentence(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    let truncated: String = chars[..max_chars].iter().collect();
    let sentence_endings = ['。', '！', '？', '；', '.', '!', '?', ';'];
    let mut last_end = None;
    for (i, c) in truncated.chars().enumerate() {
        if sentence_endings.contains(&c) {
            last_end = Some(i);
        }
    }
    match last_end {
        Some(pos) => {
            let mut s: String = chars[..=pos].iter().collect();
            s.push('…');
            s
        }
        None => format!("{truncated}…"),
    }
}
