mod api;
mod checker;
mod config;
mod history;
mod parser;
mod positions;
mod prompt;

use axum::extract::{Path, State};
use axum::response::{sse::Event, sse::Sse, IntoResponse, Json, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

const MAP_ORDER: &[&str] = &[
    "arms-factory",
    "sacred-heart-hospital",
    "the-red-church",
    "lakeside-village",
    "moonlit-river-park",
    "leos-memory",
    "eversleeping-town",
    "chinatown",
    "darkwoods",
];

const CHAR_ORDER: &[&str] = &[
    "doctor", "lawyer", "thief", "gardener", "magician", "explorer",
    "mercenary", "coordinator", "mechanic", "forward", "the-minds-eye",
    "priestess", "perfumer", "cowboy", "female-dancer", "seer",
    "embalmer", "prospector", "enchantress", "wildling", "acrobat",
    "first-officer", "barmaid", "postman", "grave-keeper", "prisoner",
    "entomologist", "painter", "batter", "toy-merchant", "patient",
    "psychologist", "novelist", "little-girl", "weeping-clown",
    "professor", "antiquarian", "composer", "journalist", "aeroplanist",
    "cheerleader", "puppeteer", "fire-investigator", "faro-lady",
    "knight", "meteorologist", "archer", "escapologist", "lanternist",
    "matador", "lucky-guy",
];

fn talent_stem(zh: &str) -> &str {
    match zh {
        "回光返照" => "borrowed-time",
        "飞轮效应" => "flywheel-effect",
        "膝跳反射" => "knee-jerk-reflex",
        "化险为夷" => "tide-turner",
        _ => "",
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

struct AppState {
    project_root: PathBuf,
    db: checker::ResourceDb,
    system_prompt: String,
    config: config::ApiConfig,
    config_path: PathBuf,
    session: history::Session,
    session_path: Option<PathBuf>,
    history_dir: PathBuf,
    selected_map: String,
    selected_chars: Vec<String>,
    round_req: String,
    messages: Vec<Value>,
    retry_count: usize,
    raw_model_output: String,
    plans: Vec<parser::Plan>,
    current_plan_idx: usize,
    round_usage: Option<api::Usage>,
    round_cost: f64,
    round_estimated: bool,
    api_interrupt: Option<Arc<AtomicBool>>,
}

impl AppState {
    fn icons_dir(&self) -> PathBuf {
        self.project_root.join("icons")
    }

    fn positions_dir(&self) -> PathBuf {
        self.icons_dir().join("positions")
    }

    fn find_map_by_stem(&self, stem: &str) -> Option<&checker::Entry> {
        let lower = stem.to_lowercase();
        self.db.maps.iter().find(|e| e.stem == lower)
    }

    fn find_char_by_stem(&self, stem: &str) -> Option<&checker::Entry> {
        let lower = stem.to_lowercase();
        self.db.survivors.iter().find(|e| e.stem == lower)
    }
}

fn plan_to_json(plan: &parser::Plan) -> Value {
    json!({
        "index": plan.index,
        "selections": plan.selections.iter().map(|s| json!({
            "point": s.point,
            "character": s.character,
            "talents": s.talents,
        })).collect::<Vec<_>>(),
        "description": plan.description,
    })
}

fn usage_to_json(u: &api::Usage) -> Value {
    json!({
        "prompt_tokens": u.prompt_tokens,
        "completion_tokens": u.completion_tokens,
    })
}

// ======================== Request types ========================

#[derive(serde::Deserialize)]
struct ConfigRequest {
    base_url: String,
    api_key: String,
    model: String,
    temperature: f64,
    max_tokens: Option<u32>,
    price_input_per_m: f64,
    price_output_per_m: f64,
    save_default: bool,
}

#[derive(serde::Deserialize)]
struct LoadRequest {
    name: String,
}

#[derive(serde::Deserialize)]
struct SaveRequest {
    name: String,
}

#[derive(serde::Deserialize)]
struct GlobalReqRequest {
    text: String,
}

#[derive(serde::Deserialize)]
struct RoundRequest {
    map: String,
    characters: Vec<String>,
    round_req: String,
}

// ======================== API Handlers ========================

async fn get_config(State(state): State<Arc<Mutex<AppState>>>) -> Response {
    let s = state.lock().await;
    let has_default = s.config_path.exists();
    Json(json!({
        "has_default": has_default,
        "base_url": s.config.base_url,
        "model": s.config.model,
        "temperature": s.config.temperature,
        "max_tokens": s.config.max_tokens,
        "price_input_per_m": s.config.price_input_per_m,
        "price_output_per_m": s.config.price_output_per_m,
    }))
    .into_response()
}

async fn set_config(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(req): Json<ConfigRequest>,
) -> Response {
    let mut s = state.lock().await;
    let temp = if (0.0..=2.0).contains(&req.temperature) {
        config::round2(req.temperature)
    } else {
        0.7
    };
    let max_tokens = req.max_tokens.filter(|&v| v >= 1);

    let cfg = config::ApiConfig {
        base_url: req.base_url.trim().to_string(),
        api_key: req.api_key.trim().to_string(),
        model: req.model.trim().to_string(),
        temperature: temp,
        max_tokens,
        price_input_per_m: req.price_input_per_m,
        price_output_per_m: req.price_output_per_m,
    };

    if req.save_default {
        let _ = config::save(&s.config_path, &cfg);
    }
    s.config = cfg;
    Json(json!({"ok": true})).into_response()
}

async fn list_history(State(state): State<Arc<Mutex<AppState>>>) -> Response {
    let s = state.lock().await;
    let files = history::list(&s.history_dir);
    let list: Vec<Value> = files
        .iter()
        .filter_map(|p| {
            history::load(p).ok().map(|session| {
                let (pt, ct, cost) = session.totals();
                json!({
                    "name": p.file_name().unwrap().to_string_lossy(),
                    "model": session.model,
                    "rounds_count": session.rounds.len(),
                    "prompt_tokens": pt,
                    "completion_tokens": ct,
                    "cost": cost,
                    "global_req": if session.global_req.trim().is_empty() {
                        "".to_string()
                    } else {
                        api::truncate_chars(session.global_req.trim(), 30)
                    },
                })
            })
        })
        .collect();
    Json(json!(list)).into_response()
}

async fn load_history(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(req): Json<LoadRequest>,
) -> Response {
    let mut s = state.lock().await;
    let path = s.history_dir.join(&req.name);
    match history::load(&path) {
        Ok(session) => {
            s.session.base_url = s.config.base_url.clone();
            s.session.model = s.config.model.clone();
            s.session.global_req = session.global_req.clone();
            s.session.rounds = session.rounds.clone();
            s.session_path = Some(path);
            Json(json!({
                "global_req": session.global_req,
                "rounds_count": session.rounds.len(),
            }))
            .into_response()
        }
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

async fn save_history(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(req): Json<SaveRequest>,
) -> Response {
    let s = state.lock().await;
    let path = history::path_for(&s.history_dir, &req.name);
    match history::save(&path, &s.session) {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

async fn set_global_req(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(req): Json<GlobalReqRequest>,
) -> Response {
    let mut s = state.lock().await;
    s.session.global_req = req.text;
    Json(json!({"ok": true})).into_response()
}

async fn get_resources(State(state): State<Arc<Mutex<AppState>>>) -> Response {
    let s = state.lock().await;
    let maps: Vec<Value> = MAP_ORDER
        .iter()
        .filter_map(|stem| s.find_map_by_stem(stem))
        .map(|e| json!({"stem": e.stem, "zh": e.zh}))
        .collect();
    let characters: Vec<Value> = CHAR_ORDER
        .iter()
        .filter_map(|stem| s.find_char_by_stem(stem))
        .map(|e| json!({"stem": e.stem, "zh": e.zh}))
        .collect();
    Json(json!({"maps": maps, "characters": characters, "talent_map": {
        "回光返照": "borrowed-time",
        "飞轮效应": "flywheel-effect",
        "膝跳反射": "knee-jerk-reflex",
        "化险为夷": "tide-turner",
    }}))
    .into_response()
}

async fn set_round(
    State(state): State<Arc<Mutex<AppState>>>,
    Json(req): Json<RoundRequest>,
) -> Response {
    let mut s = state.lock().await;
    s.selected_map = req.map;
    s.selected_chars = req.characters;
    s.round_req = req.round_req;
    Json(json!({"ok": true})).into_response()
}

async fn get_positions(
    State(state): State<Arc<Mutex<AppState>>>,
    Path(map): Path<String>,
) -> Response {
    let s = state.lock().await;
    let path = s.positions_dir().join(format!("{map}.md"));
    match positions::MapPosition::load(&path) {
        Ok(pos) => Json(json!({
            "map_width": pos.map_width,
            "map_height": pos.map_height,
            "icon_size": pos.icon_size,
            "points": pos.points.iter().map(|(num, x, y)| json!({
                "num": num, "x": x, "y": y
            })).collect::<Vec<_>>(),
        }))
        .into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

async fn interrupt(State(state): State<Arc<Mutex<AppState>>>) -> Response {
    let s = state.lock().await;
    if let Some(flag) = &s.api_interrupt {
        flag.store(true, Ordering::Relaxed);
        Json(json!({"ok": true})).into_response()
    } else {
        Json(json!({"error": "无正在进行的分析"})).into_response()
    }
}

async fn end_round(State(state): State<Arc<Mutex<AppState>>>) -> Response {
    let mut s = state.lock().await;

    let map_entry = match s.find_map_by_stem(&s.selected_map) {
        Some(e) => e.clone(),
        None => return Json(json!({"error": "未选择地图"})).into_response(),
    };
    let char_entries: Vec<checker::Entry> = s
        .selected_chars
        .iter()
        .filter_map(|stem| s.find_char_by_stem(stem).cloned())
        .collect();
    let idx = s.session.rounds.len() + 1;
    let global_req = s.session.global_req.clone();
    let round_req = s.round_req.clone();
    let spec = prompt::RoundSpec {
        index: idx,
        map: &map_entry,
        survivors: char_entries.iter().collect(),
        global_req: &global_req,
        round_req: &round_req,
    };
    let compact_req = prompt::compact_request(&spec);

    let record = history::RoundRecord {
        index: idx,
        map: map_entry.zh.clone(),
        survivors: char_entries.iter().map(|e| e.zh.clone()).collect(),
        round_req: s.round_req.clone(),
        compact_request: compact_req,
        assistant_reply: s.raw_model_output.clone(),
        interrupted: false,
        usage: s.round_usage,
        cost: s.round_cost,
        estimated: s.round_estimated,
    };
    s.session.rounds.push(record);

    let (pt, ct, cost) = s.session.totals();
    let usage_json = s.round_usage.map(|u| usage_to_json(&u));
    let round_cost = s.round_cost;
    let estimated = s.round_estimated;
    let rounds_count = s.session.rounds.len();
    drop(s);

    Json(json!({
        "usage": usage_json,
        "cost": round_cost,
        "estimated": estimated,
        "session_totals": {
            "prompt_tokens": pt,
            "completion_tokens": ct,
            "cost": cost,
            "rounds_count": rounds_count,
        },
    }))
    .into_response()
}

async fn get_session(State(state): State<Arc<Mutex<AppState>>>) -> Response {
    let s = state.lock().await;
    let (pt, ct, cost) = s.session.totals();
    Json(json!({
        "global_req": s.session.global_req,
        "rounds_count": s.session.rounds.len(),
        "prompt_tokens": pt,
        "completion_tokens": ct,
        "cost": cost,
        "selected_map": s.selected_map,
        "selected_chars": s.selected_chars,
        "round_req": s.round_req,
    }))
    .into_response()
}

// ======================== SSE Analyze ========================

fn sse(data: String) -> Result<Event, std::convert::Infallible> {
    Ok(Event::default().data(data))
}

async fn analyze(State(state): State<Arc<Mutex<AppState>>>) -> Response {
    let (cfg, messages) = {
        let mut s = state.lock().await;

        if let Some(flag) = s.api_interrupt.take() {
            flag.store(true, Ordering::Relaxed);
        }

        let map_entry = match s.find_map_by_stem(&s.selected_map) {
            Some(e) => e.clone(),
            None => {
                return Json(json!({"error": "未选择地图"})).into_response();
            }
        };

        let char_entries: Vec<checker::Entry> = s
            .selected_chars
            .iter()
            .filter_map(|stem| s.find_char_by_stem(stem).cloned())
            .collect();
        if char_entries.len() != 4 {
            return Json(json!({"error": "角色数量不为4"})).into_response();
        }

        let idx = s.session.rounds.len() + 1;
        let global_req = s.session.global_req.clone();
        let round_req = s.round_req.clone();
        let spec = prompt::RoundSpec {
            index: idx,
            map: &map_entry,
            survivors: char_entries.iter().collect(),
            global_req: &global_req,
            round_req: &round_req,
        };

        let full_req = match prompt::full_request(&spec) {
            Ok(text) => text,
            Err(e) => {
                return Json(json!({"error": format!("读取本局资料失败：{e}")}))
                    .into_response();
            }
        };

        let system_prompt = s.system_prompt.clone();
        let mut msgs = vec![json!({ "role": "system", "content": system_prompt })];
        for r in &s.session.rounds {
            msgs.push(json!({ "role": "user", "content": r.compact_request }));
            let ans = if r.assistant_reply.trim().is_empty() {
                "（该局分析被打断，未生成结果）"
            } else {
                r.assistant_reply.as_str()
            };
            msgs.push(json!({ "role": "assistant", "content": ans }));
        }
        msgs.push(json!({ "role": "user", "content": full_req }));

        s.messages = msgs.clone();
        s.retry_count = 0;
        s.raw_model_output.clear();
        s.plans.clear();
        s.round_usage = None;
        s.round_cost = 0.0;
        s.round_estimated = false;

        (s.config.clone(), msgs)
    };

    let (rx, interrupt) = api::chat_stream_threaded(cfg, messages);
    {
        let mut s = state.lock().await;
        s.api_interrupt = Some(interrupt);
    }

    let state_clone = state.clone();
    let stream = async_stream::stream! {
        let mut rx_opt: Option<mpsc::Receiver<api::ApiEvent>> = Some(rx);
        loop {
            let recv_result = if let Some(rx) = &rx_opt {
                rx.try_recv()
            } else {
                return;
            };

            match recv_result {
                Ok(api::ApiEvent::Done(result)) => {
                    match result {
                        Ok(outcome) => {
                            let mut s = state_clone.lock().await;
                            s.api_interrupt = None;

                            if outcome.interrupted {
                                yield sse(json!({"type":"error","message":"分析已打断"}).to_string());
                                return;
                            }

                            s.raw_model_output = outcome.content.clone();

                            let map_stem = s.selected_map.clone();
                            let pos_path = s.positions_dir().join(format!("{map_stem}.md"));
                            let max_point = positions::MapPosition::load(&pos_path)
                                .ok()
                                .map(|p| p.max_point())
                                .unwrap_or(12);

                            match parser::parse_and_validate(&outcome.content, &s.db, max_point) {
                                Ok(plans) => {
                                    s.plans = plans.clone();
                                    let prompt_chars: usize = s.messages.iter()
                                        .filter_map(|m| m.get("content").and_then(Value::as_str).map(str::chars).map(|c| c.count()))
                                        .sum();
                                    let (usage, estimated) = match outcome.usage {
                                        Some(u) => (u, false),
                                        None => {
                                            let est = api::Usage {
                                                prompt_tokens: (prompt_chars as f64 * 0.75).ceil() as u64,
                                                completion_tokens: (outcome.content.chars().count() as f64 * 0.75).ceil() as u64,
                                            };
                                            (est, true)
                                        }
                                    };
                                    s.round_usage = Some(usage);
                                    s.round_estimated = estimated;
                                    s.round_cost = usage.cost(s.config.price_input_per_m, s.config.price_output_per_m);

                                    yield sse(json!({
                                        "type": "plans",
                                        "plans": plans.iter().map(|p| plan_to_json(p)).collect::<Vec<_>>(),
                                        "usage": usage_to_json(&usage),
                                        "cost": s.round_cost,
                                        "estimated": estimated,
                                    }).to_string());
                                    return;
                                }
                                Err(e) => {
                                    if s.retry_count < 3 {
                                        s.retry_count += 1;
                                        let retry_count = s.retry_count;
                                        s.messages.push(json!({
                                            "role": "assistant",
                                            "content": outcome.content
                                        }));
                                        s.messages.push(json!({
                                            "role": "user",
                                            "content": format!(
                                                "你的输出有误：{}。请严格按照格式要求重新生成。\n格式要求：\n- 每套方案6行：方案编号行、4行选点行、1行描述行。\n- 选点行格式：x 号选点：角色名：天赋1、天赋2\n- 描述行以\"具体描述及分析：\"开头，不超过100字。\n- 不要输出其他内容。",
                                                e
                                            )
                                        }));
                                        let cfg2 = s.config.clone();
                                        let msgs2 = s.messages.clone();
                                        let (new_rx, new_int) = api::chat_stream_threaded(cfg2, msgs2);
                                        s.api_interrupt = Some(new_int);
                                        rx_opt = Some(new_rx);

                                        yield sse(json!({"type":"retry","count":retry_count}).to_string());
                                        drop(s);
                                        continue;
                                    } else {
                                        yield sse(json!({"type":"error","message":"您的模型与该项目不适配"}).to_string());
                                        return;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let mut s = state_clone.lock().await;
                            s.api_interrupt = None;
                            yield sse(json!({"type":"error","message":e}).to_string());
                            return;
                        }
                    }
                }
                Ok(api::ApiEvent::Delta(_)) => {}
                Err(mpsc::TryRecvError::Empty) => {
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    continue;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let mut s = state_clone.lock().await;
                    s.api_interrupt = None;
                    yield sse(json!({"type":"error","message":"API 线程异常断开"}).to_string());
                    return;
                }
            }
        }
    };

    Sse::new(stream).into_response()
}

// ======================== Main ========================

#[tokio::main]
async fn main() {
    let app_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = app_root.parent().unwrap().to_path_buf();
    let resources_dir = project_root.join("resources");
    let history_dir = app_root.join("history");
    let config_path = app_root.join("config.json");

    std::fs::create_dir_all(&history_dir).ok();

    let db = checker::ResourceDb::load(&resources_dir).unwrap_or_else(|e| {
        eprintln!("[IDV-ASAS] 加载资源库失败：{e}");
        checker::ResourceDb {
            maps: Vec::new(),
            survivors: Vec::new(),
        }
    });
    let system_prompt = prompt::system_prompt(&resources_dir).unwrap_or_default();
    let cfg = config::load(&config_path).unwrap_or_default();

    let state = Arc::new(Mutex::new(AppState {
        project_root: project_root.clone(),
        db,
        system_prompt,
        config: cfg.clone(),
        config_path,
        session: history::Session {
            version: 1,
            created_at_unix: now_unix(),
            base_url: cfg.base_url.clone(),
            model: cfg.model.clone(),
            global_req: String::new(),
            rounds: Vec::new(),
        },
        session_path: None,
        history_dir,
        selected_map: String::new(),
        selected_chars: Vec::new(),
        round_req: String::new(),
        messages: Vec::new(),
        retry_count: 0,
        raw_model_output: String::new(),
        plans: Vec::new(),
        current_plan_idx: 0,
        round_usage: None,
        round_cost: 0.0,
        round_estimated: false,
        api_interrupt: None,
    }));

    let app = Router::new()
        .route("/", get(|| async { Redirect::permanent("/static/index.html") }))
        .nest_service("/static", ServeDir::new(app_root.join("static")))
        .nest_service("/icons", ServeDir::new(project_root.join("icons")))
        .route("/api/config", get(get_config).post(set_config))
        .route("/api/history", get(list_history))
        .route("/api/history/load", post(load_history))
        .route("/api/history/save", post(save_history))
        .route("/api/session/global-req", post(set_global_req))
        .route("/api/resources", get(get_resources))
        .route("/api/session/round", post(set_round))
        .route("/api/positions/{map}", get(get_positions))
        .route("/api/session/analyze", get(analyze))
        .route("/api/session/interrupt", post(interrupt))
        .route("/api/session/end-round", post(end_round))
        .route("/api/session", get(get_session))
        .with_state(state);

    let addr = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    eprintln!("[IDV-ASAS WebUI] 服务已启动：http://{addr}");
    eprintln!("[IDV-ASAS WebUI] 在浏览器中打开上述地址即可使用");
    axum::serve(listener, app).await.unwrap();
}
