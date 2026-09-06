#[path = "ui_impl.rs"]
mod ui_impl;

use crate::api::{ApiEvent, Usage};
use crate::checker::{Entry, ResourceDb};
use crate::config::ApiConfig;
use crate::history::{RoundRecord, Session};
use crate::images::ImageCache;
use crate::parser::Plan;
use crate::positions::MapPosition;
use crate::prompt::RoundSpec;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const MAP_ORDER: &[&str] = &[
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

pub(crate) const CHAR_ORDER: &[&str] = &[
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

pub(crate) fn talent_stem(zh: &str) -> &str {
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

pub(crate) const BG_DARK: egui::Color32 = egui::Color32::from_rgb(15, 20, 28);
pub(crate) const PANEL_BG: egui::Color32 = egui::Color32::from_rgb(22, 28, 38);
pub(crate) const YELLOW: egui::Color32 = egui::Color32::from_rgb(255, 215, 0);
pub(crate) const WHITE: egui::Color32 = egui::Color32::WHITE;
pub(crate) const GRAY: egui::Color32 = egui::Color32::from_rgb(80, 80, 80);

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Step {
    Config,
    HistoryImport,
    GlobalReq,
    MapSelect,
    CharSelect,
    RoundReq,
    Analyzing,
    ResultDisplay,
    WaitingEnd,
    AskNext,
    SaveHistory,
    ErrorExit,
}

pub struct IdvApp {
    pub(crate) project_root: PathBuf,
    pub(crate) db: ResourceDb,
    pub(crate) system_prompt: String,
    pub(crate) config: ApiConfig,
    pub(crate) config_path: PathBuf,
    pub(crate) session: Session,
    pub(crate) session_path: Option<PathBuf>,
    pub(crate) history_dir: PathBuf,
    pub(crate) char_icons: ImageCache,
    pub(crate) map_icons: ImageCache,
    pub(crate) area_images: ImageCache,
    pub(crate) persona_icons: ImageCache,
    pub(crate) step: Step,
    pub(crate) cfg_base_url: String,
    pub(crate) cfg_api_key: String,
    pub(crate) cfg_model: String,
    pub(crate) cfg_temperature: String,
    pub(crate) cfg_max_tokens: String,
    pub(crate) cfg_price_in: String,
    pub(crate) cfg_price_out: String,
    pub(crate) cfg_has_default: bool,
    pub(crate) cfg_use_default: bool,
    pub(crate) cfg_save_default: bool,
    pub(crate) cfg_show_form: bool,
    pub(crate) history_list: Vec<(PathBuf, Session)>,
    pub(crate) selected_map_stem: String,
    pub(crate) selected_char_stems: Vec<String>,
    pub(crate) char_page: usize,
    pub(crate) round_req: String,
    pub(crate) api_rx: Option<mpsc::Receiver<ApiEvent>>,
    pub(crate) api_interrupt: Option<Arc<AtomicBool>>,
    pub(crate) retry_count: usize,
    pub(crate) raw_model_output: String,
    pub(crate) messages: Vec<Value>,
    pub(crate) analyze_error: Option<String>,
    pub(crate) plans: Vec<Plan>,
    pub(crate) current_plan_idx: usize,
    pub(crate) plan_confirmed: bool,
    pub(crate) round_usage: Option<Usage>,
    pub(crate) round_cost: f64,
    pub(crate) round_estimated: bool,
    pub(crate) error_msg: Option<String>,
    pub(crate) show_save_dialog: bool,
    pub(crate) save_name_input: String,
    pub(crate) want_exit: bool,
    pub(crate) positions: HashMap<String, MapPosition>,
    pub(crate) egui_ctx: Option<egui::Context>,
}

impl IdvApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let app_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_root = app_root.parent().unwrap().to_path_buf();
        let resources_dir = project_root.join("resources");
        let icons_dir = project_root.join("icons");
        let history_dir = app_root.join("history");
        let config_path = app_root.join("config.json");

        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "deng".to_string(),
            Arc::new(egui::FontData::from_owned(
                include_bytes!("../assets/Deng.ttf").to_vec(),
            )),
        );
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .push("deng".to_string());
        fonts
            .families
            .get_mut(&egui::FontFamily::Monospace)
            .unwrap()
            .push("deng".to_string());
        cc.egui_ctx.set_fonts(fonts);

        cc.egui_ctx.all_styles_mut(|style| {
            style.text_styles = [
                (egui::TextStyle::Heading, egui::FontId::proportional(36.0)),
                (egui::TextStyle::Body, egui::FontId::proportional(24.0)),
                (egui::TextStyle::Button, egui::FontId::proportional(24.0)),
                (egui::TextStyle::Small, egui::FontId::proportional(18.0)),
                (egui::TextStyle::Monospace, egui::FontId::monospace(22.0)),
            ]
            .into();
        });

        let mut visuals = egui::Visuals::dark();
        visuals.faint_bg_color = egui::Color32::from_rgb(30, 36, 48);
        visuals.extreme_bg_color = egui::Color32::from_rgb(10, 13, 18);
        cc.egui_ctx.set_visuals(visuals);

        let db = crate::checker::ResourceDb::load(&resources_dir).unwrap_or_else(|e| {
            eprintln!("加载资源库失败：{e}");
            crate::checker::ResourceDb {
                maps: Vec::new(),
                survivors: Vec::new(),
            }
        });
        let system_prompt = crate::prompt::system_prompt(&resources_dir).unwrap_or_default();

        let cfg = crate::config::load(&config_path).unwrap_or_default();
        let cfg_has_default = config_path.exists();

        let app = IdvApp {
            project_root: project_root.clone(),
            db,
            system_prompt,
            config: cfg.clone(),
            config_path,
            session: Session {
                version: 1,
                created_at_unix: now_unix(),
                base_url: cfg.base_url.clone(),
                model: cfg.model.clone(),
                global_req: String::new(),
                rounds: Vec::new(),
            },
            session_path: None,
            history_dir,
            char_icons: ImageCache::new(&icons_dir.join("characters")),
            map_icons: ImageCache::new(&icons_dir.join("maps")),
            area_images: ImageCache::new(&icons_dir.join("area-selection")),
            persona_icons: ImageCache::new(&icons_dir.join("personas")),
            step: Step::Config,
            cfg_base_url: cfg.base_url.clone(),
            cfg_api_key: cfg.api_key.clone(),
            cfg_model: if cfg.model.is_empty() {
                "gpt-4o-mini".to_string()
            } else {
                cfg.model.clone()
            },
            cfg_temperature: format!("{}", cfg.temperature),
            cfg_max_tokens: cfg
                .max_tokens
                .map(|v| v.to_string())
                .unwrap_or_default(),
            cfg_price_in: format!("{}", cfg.price_input_per_m),
            cfg_price_out: format!("{}", cfg.price_output_per_m),
            cfg_has_default,
            cfg_use_default: true,
            cfg_save_default: true,
            cfg_show_form: false,
            history_list: Vec::new(),
            selected_map_stem: String::new(),
            selected_char_stems: Vec::new(),
            char_page: 0,
            round_req: String::new(),
            api_rx: None,
            api_interrupt: None,
            retry_count: 0,
            raw_model_output: String::new(),
            messages: Vec::new(),
            analyze_error: None,
            plans: Vec::new(),
            current_plan_idx: 0,
            plan_confirmed: false,
            round_usage: None,
            round_cost: 0.0,
            round_estimated: false,
            error_msg: None,
            show_save_dialog: false,
            save_name_input: String::new(),
            want_exit: false,
            positions: HashMap::new(),
            egui_ctx: Some(cc.egui_ctx.clone()),
        };
        app
    }

    pub(crate) fn icons_dir(&self) -> PathBuf {
        self.project_root.join("icons")
    }

    pub(crate) fn positions_dir(&self) -> PathBuf {
        self.icons_dir().join("positions")
    }

    pub(crate) fn load_positions(&mut self, map_stem: &str) -> Option<&MapPosition> {
        if !self.positions.contains_key(map_stem) {
            let path = self.positions_dir().join(format!("{map_stem}.md"));
            if let Ok(pos) = MapPosition::load(&path) {
                self.positions.insert(map_stem.to_string(), pos);
            }
        }
        self.positions.get(map_stem)
    }

    pub(crate) fn find_entry_by_stem(&self, stem: &str) -> Option<&Entry> {
        let lower = stem.to_lowercase();
        self.db.maps.iter().find(|e| e.stem == lower).or_else(|| {
            self.db.survivors.iter().find(|e| e.stem == lower)
        })
    }

    pub(crate) fn find_char_by_stem(&self, stem: &str) -> Option<&Entry> {
        let lower = stem.to_lowercase();
        self.db.survivors.iter().find(|e| e.stem == lower)
    }

    pub(crate) fn find_map_by_stem(&self, stem: &str) -> Option<&Entry> {
        let lower = stem.to_lowercase();
        self.db.maps.iter().find(|e| e.stem == lower)
    }

    pub(crate) fn start_analysis(&mut self, ctx: &egui::Context) {
        let map_entry = match self.find_map_by_stem(&self.selected_map_stem) {
            Some(e) => e.clone(),
            None => return,
        };
        let char_entries: Vec<Entry> = self
            .selected_char_stems
            .iter()
            .filter_map(|s| self.find_char_by_stem(s).cloned())
            .collect();
        if char_entries.len() != 4 {
            return;
        }
        let idx = self.session.rounds.len() + 1;
        let spec = RoundSpec {
            index: idx,
            map: &map_entry,
            survivors: char_entries.iter().collect(),
            global_req: &self.session.global_req,
            round_req: &self.round_req,
        };
            let full_req = match crate::prompt::full_request(&spec) {
            Ok(s) => s,
            Err(e) => {
                self.error_msg = Some(format!("读取本局资料失败：{e}"));
                self.step = Step::ErrorExit;
                return;
            }
        };
        let _compact_req = crate::prompt::compact_request(&spec);

        let mut messages = vec![json!({ "role": "system", "content": self.system_prompt })];
        for r in &self.session.rounds {
            messages.push(json!({ "role": "user", "content": r.compact_request }));
            let ans = if r.assistant_reply.trim().is_empty() {
                "（该局分析被打断，未生成结果）"
            } else {
                r.assistant_reply.as_str()
            };
            messages.push(json!({ "role": "assistant", "content": ans }));
        }
        messages.push(json!({ "role": "user", "content": full_req }));
        self.messages = messages;
        self.retry_count = 0;
        self.raw_model_output.clear();
        self.analyze_error = None;
        self.plans.clear();
        self.plan_confirmed = false;
        self.round_usage = None;
        self.round_cost = 0.0;
        self.round_estimated = false;
        self.start_api_call(ctx);
    }

    pub(crate) fn start_api_call(&mut self, ctx: &egui::Context) {
        let (rx, interrupt) = crate::api::chat_stream_threaded(self.config.clone(), self.messages.clone());
        self.api_rx = Some(rx);
        self.api_interrupt = Some(interrupt);
        ctx.request_repaint();
    }

    pub(crate) fn poll_api(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.api_rx {
            match rx.try_recv() {
                Ok(ApiEvent::Done(result)) => {
                    self.api_rx = None;
                    self.api_interrupt = None;
                    match result {
                        Ok(outcome) => {
                            self.raw_model_output = outcome.content.clone();
                            if outcome.interrupted {
                                self.analyze_error = Some("分析已打断".to_string());
                                self.step = Step::ErrorExit;
                                return;
                            }
                            let map_stem = self.selected_map_stem.clone();
                            let max_point = self
                                .load_positions(&map_stem)
                                .map(|p| p.max_point())
                                .unwrap_or(12);
                            match crate::parser::parse_and_validate(
                                &outcome.content,
                                &self.db,
                                max_point,
                            ) {
                                Ok(plans) => {
                                    self.plans = plans;
                                    self.current_plan_idx = 0;
                                    let prompt_chars: usize = self
                                        .messages
                                        .iter()
                                        .filter_map(|m| {
                                            m.get("content").and_then(Value::as_str).map(str::chars).map(|c| c.count())
                                        })
                                        .sum();
                                    let (usage, estimated) = match outcome.usage {
                                        Some(u) => (u, false),
                                        None => {
                                            let est = Usage {
                                                prompt_tokens: (prompt_chars as f64 * 0.75).ceil() as u64,
                                                completion_tokens: (outcome.content.chars().count() as f64 * 0.75).ceil() as u64,
                                            };
                                            (est, true)
                                        }
                                    };
                                    self.round_usage = Some(usage);
                                    self.round_estimated = estimated;
                                    self.round_cost = usage.cost(
                                        self.config.price_input_per_m,
                                        self.config.price_output_per_m,
                                    );
                                    self.step = Step::ResultDisplay;
                                }
                                Err(e) => {
                                    if self.retry_count < 3 {
                                        self.retry_count += 1;
                                        self.messages.push(json!({
                                            "role": "assistant",
                                            "content": outcome.content
                                        }));
                                        self.messages.push(json!({
                                            "role": "user",
                                            "content": format!(
                                                "你的输出有误：{}。请严格按照格式要求重新生成。\n格式要求：\n- 每套方案6行：方案编号行、4行选点行、1行描述行。\n- 选点行格式：x 号选点：角色名：天赋1、天赋2\n- 描述行以\"具体描述及分析：\"开头，不超过100字。\n- 不要输出其他内容。",
                                                e
                                            )
                                        }));
                                        self.start_api_call(ctx);
                                    } else {
                                        self.error_msg = Some("您的模型与该项目不适配".to_string());
                                        self.step = Step::ErrorExit;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            self.analyze_error = Some(e);
                            self.step = Step::ErrorExit;
                        }
                    }
                }
                Ok(ApiEvent::Delta(_)) => {}
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.api_rx = None;
                    self.api_interrupt = None;
                    self.analyze_error = Some("API 线程异常断开".to_string());
                    self.step = Step::ErrorExit;
                }
            }
        }
    }

    pub(crate) fn do_interrupt(&mut self) {
        if let Some(flag) = &self.api_interrupt {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn save_round_to_session(&mut self) {
        let map_entry = match self.find_map_by_stem(&self.selected_map_stem) {
            Some(e) => e.clone(),
            None => return,
        };
        let char_entries: Vec<Entry> = self
            .selected_char_stems
            .iter()
            .filter_map(|s| self.find_char_by_stem(s).cloned())
            .collect();
        let idx = self.session.rounds.len() + 1;
        let spec = RoundSpec {
            index: idx,
            map: &map_entry,
            survivors: char_entries.iter().collect(),
            global_req: &self.session.global_req,
            round_req: &self.round_req,
        };
        let compact_req = crate::prompt::compact_request(&spec);
        let record = RoundRecord {
            index: idx,
            map: map_entry.zh.clone(),
            survivors: char_entries.iter().map(|e| e.zh.clone()).collect(),
            round_req: self.round_req.clone(),
            compact_request: compact_req,
            assistant_reply: self.raw_model_output.clone(),
            interrupted: false,
            usage: self.round_usage,
            cost: self.round_cost,
            estimated: self.round_estimated,
        };
        self.session.rounds.push(record);
    }

    pub(crate) fn load_history_list(&mut self) {
        let files = crate::history::list(&self.history_dir);
        self.history_list = files
            .into_iter()
            .filter_map(|p| crate::history::load(&p).ok().map(|s| (p, s)))
            .collect();
    }
}

impl eframe::App for IdvApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.step == Step::Analyzing {
            self.poll_api(ctx);
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            if !self.session.rounds.is_empty() && !self.show_save_dialog {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_save_dialog = true;
                self.step = Step::SaveHistory;
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.painter().rect_filled(ui.max_rect(), 0.0, BG_DARK);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                match self.step {
                    Step::Config => self.ui_config(ui),
                    Step::HistoryImport => self.ui_history_import(ui),
                    Step::GlobalReq => self.ui_global_req(ui),
                    Step::MapSelect => self.ui_map_select(ui),
                    Step::CharSelect => self.ui_char_select(ui),
                    Step::RoundReq => self.ui_round_req(ui),
                    Step::Analyzing => self.ui_analyzing(ui),
                    Step::ResultDisplay => self.ui_result_display(ui),
                    Step::WaitingEnd => self.ui_waiting_end(ui),
                    Step::AskNext => self.ui_ask_next(ui),
                    Step::SaveHistory => self.ui_save_history(ui),
                    Step::ErrorExit => self.ui_error_exit(ui),
                }
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.058, 0.078, 0.109, 1.0]
    }
}
