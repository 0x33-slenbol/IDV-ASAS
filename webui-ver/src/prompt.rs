use crate::checker::Entry;
use std::fs;
use std::io;
use std::path::Path;

fn read(p: &Path) -> io::Result<String> {
    fs::read_to_string(p)
}

pub struct RoundSpec<'a> {
    pub index: usize,
    pub map: &'a Entry,
    pub survivors: Vec<&'a Entry>,
    pub global_req: &'a str,
    pub round_req: &'a str,
}

pub fn system_prompt(root: &Path) -> io::Result<String> {
    let agents = read(&root.join("AGENTS.md"))?;
    let general = read(&root.join("README.md"))?;
    let chars_guide = read(&root.join("characters").join("README.md"))?;
    let maps_guide = read(&root.join("maps").join("README.md"))?;
    let dict = read(&root.parent().unwrap().join("DICTIONARY.md"))?;
    Ok(format!(
        "你是 IDV-ASAS（《第五人格》区域选择辅助系统）的分析核心。\n\n\
# 你的任务\n\n{agents}\n\n\
# 输出要求\n\n\
- 给出 2 到 3 套天赋选择以及区域选择，按推荐程度由高到低排列。\n\
- 每套方案仅包含三部分：方案编号、四名求生者的选点与天赋、该选点的具体描述及分析，不要输出任何前言、思考推理过程、总结或额外说明。\n\
- 每套方案严格遵循以下格式（示例）：\n\n\
方案 1：\n\
5 号选点：佣兵：回光返照、化险为夷\n\
2 号选点：古董商：回光返照、飞轮效应\n\
6 号选点：作曲家：回光返照、膝跳反射\n\
8 号选点：空军：回光返照、化险为夷\n\
具体描述及分析：前锋利用中场板窗承担抗压牵制，医生置于安全的小树林稳定破译，园丁与祭司分别负责大房与沙包的分工运营，整体抗压与分工均衡。\n\n\
- 选点行严格写作\"x 号选点：<求生者名称>：<具体天赋选择>\"，禁止在选点后附加括号、俗称或任何其他说明。这样的行共四行，覆盖四名求生者，四人选点不得重复。\n\
- 除非用户特别说明，否则禁止选择地图数据推荐列表之外的选点模式，也不得选择地图禁区中的点位。\n\
- 用户需求高于一切：若用户需求与数据有差异，以用户需求为准。\n\
- 具体描述及分析部分为不多于 100 字的流利中文，重点介绍抗压与分工，不需要过多介绍推荐理由，禁止出现\"x 号选点\"这类编号说法而应该采用俗称。\n\
- 全部使用中文回答。\n\n\
# 角色与地图字典\n\n{dict}\n\n\
# 数据库通用资料\n\n\
## 求生者与队伍总纲\n\n{general}\n\n\
## 天赋与角色说明\n\n{chars_guide}\n\n\
## 地图与选点说明\n\n{maps_guide}\n\n\
# 重要覆盖说明\n\n\
上述资料中提到\"禁止称呼 x 号选点、应采用俗称\"是面向玩家交互的惯例，**不适用于本系统的输出格式**。本系统选点行一律使用编号（如 5 号选点），禁止附加俗称；仅在\"具体描述及分析\"部分使用俗称。\n"
    ))
}

pub fn compact_request(spec: &RoundSpec) -> String {
    format!(
        "【第 {} 局】\n地图：{}\n求生者：{}\n全局需求：{}\n本局需求：{}\n（该局地图与角色详细资料此处从略）",
        spec.index,
        spec.map.zh,
        spec.survivors
            .iter()
            .map(|e| e.zh.as_str())
            .collect::<Vec<_>>()
            .join("、"),
        fmt_req(spec.global_req),
        fmt_req(spec.round_req),
    )
}

pub fn full_request(spec: &RoundSpec) -> io::Result<String> {
    let map_data = read(&spec.map.path)?;
    let mut char_data = String::new();
    for s in &spec.survivors {
        char_data.push_str(&format!("\n\n【角色资料：{}】\n{}", s.zh, read(&s.path)?));
    }
    Ok(format!(
        "{}\n（以上为本局全部输入。此前的历史局记录见对话上文，其方案结果仅供参考，全局需求对每一局均生效。）\n\n【本局地图资料：{}】\n{}{}\n\n请结合以上资料与全部需求，给出 2~3 套天赋选择与区域选择方案。",
        compact_request(spec),
        spec.map.zh,
        map_data,
        char_data
    ))
}

fn fmt_req(s: &str) -> String {
    if s.trim().is_empty() {
        "（无）".to_string()
    } else {
        s.trim().to_string()
    }
}
