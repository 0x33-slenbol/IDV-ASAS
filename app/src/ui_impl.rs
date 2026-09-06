use crate::app::*;
use crate::app::{BG_DARK, GRAY, PANEL_BG, WHITE, YELLOW};
use crate::history::Session;
use std::path::PathBuf;

// ======================== UI helpers ========================

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(YELLOW)
            .text_style(egui::TextStyle::Heading),
    );
    ui.add_space(20.0);
}

fn subheading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(YELLOW)
            .text_style(egui::TextStyle::Body),
    );
    ui.add_space(10.0);
}

fn info_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(WHITE));
}

fn warn_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(egui::Color32::from_rgb(255, 140, 0)));
}

fn err_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(egui::Color32::from_rgb(255, 80, 80)));
}

fn yellow_button(ui: &mut egui::Ui, text: &str) -> bool {
    let btn = egui::Button::new(egui::RichText::new(text).color(YELLOW).text_style(egui::TextStyle::Button))
        .fill(egui::Color32::from_rgb(40, 50, 70))
        .stroke(egui::Stroke::new(2.0, YELLOW))
        .min_size(egui::vec2(140.0, 50.0));
    ui.add(btn).clicked()
}

fn gray_button(ui: &mut egui::Ui, text: &str) -> bool {
    let btn = egui::Button::new(egui::RichText::new(text).color(WHITE))
        .fill(egui::Color32::from_rgb(35, 40, 50))
        .stroke(egui::Stroke::new(1.0, GRAY))
        .min_size(egui::vec2(100.0, 40.0));
    ui.add(btn).clicked()
}

fn yellow_button_sized(ui: &mut egui::Ui, text: &str, w: f32, h: f32) -> bool {
    let btn = egui::Button::new(egui::RichText::new(text).color(YELLOW))
        .fill(egui::Color32::from_rgb(40, 50, 70))
        .stroke(egui::Stroke::new(2.0, YELLOW))
        .min_size(egui::vec2(w, h));
    ui.add(btn).clicked()
}

fn field_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    hint: &str,
    password: bool,
) {
    ui.horizontal(|ui| {
        ui.allocate_ui(egui::vec2(220.0, 20.0), |ui| {
            ui.label(egui::RichText::new(label).color(WHITE));
        });
        let te = egui::TextEdit::singleline(value)
            .desired_width(600.0)
            .text_color(WHITE);
        let te = if password { te.password(true) } else { te };
        ui.add(te);
        if !hint.is_empty() {
            ui.label(egui::RichText::new(hint).color(GRAY));
        }
    });
}

// ======================== Step 0: API Config ========================

impl crate::app::IdvApp {

pub(crate) fn ui_config(&mut self, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        heading(ui, "IDV-ASAS 区域选择辅助系统");
        subheading(ui, "第 0 步：配置 API 接口");

        if self.cfg_has_default && self.cfg_use_default && !self.cfg_show_form {
            info_label(ui, &format!(
                "检测到默认配置：\n  端点 {}  |  模型 {}  |  温度 {}",
                self.config.base_url,
                self.config.model,
                self.config.temperature,
            ));
            info_label(ui, &format!(
                "  输入单价 ¥{:.4}  |  输出单价 ¥{:.4}  每百万 tokens",
                self.config.price_input_per_m,
                self.config.price_output_per_m,
            ));
            ui.add_space(20.0);
            ui.label(egui::RichText::new("是否沿用默认配置？").color(WHITE).text_style(egui::TextStyle::Body));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(480.0);
                if yellow_button(ui, "是") {
                    self.step = Step::HistoryImport;
                    self.load_history_list();
                }
                ui.add_space(30.0);
                if yellow_button(ui, "否") {
                    self.cfg_show_form = true;
                }
            });
        } else {
            field_row(ui, "API Base URL:", &mut self.cfg_base_url, "如 https://api.xxx.com/v1", false);
            field_row(ui, "API Key:", &mut self.cfg_api_key, "以 Bearer 方式携带", true);
            field_row(ui, "模型名称:", &mut self.cfg_model, "由服务商决定", false);
            field_row(ui, "温度 temperature:", &mut self.cfg_temperature, "0~2, 默认 0.7", false);
            field_row(ui, "max_tokens:", &mut self.cfg_max_tokens, "留空 = 不限制", false);
            field_row(ui, "输入 token 单价:", &mut self.cfg_price_in, "元/每百万 tokens", false);
            field_row(ui, "输出 token 单价:", &mut self.cfg_price_out, "元/每百万 tokens", false);
            ui.add_space(10.0);
            ui.checkbox(&mut self.cfg_save_default, egui::RichText::new("保存为默认配置（config.json）").color(WHITE));
            warn_label(ui, "注意：保存后 API Key 将明文存储，请勿提交到仓库或分发给他人。");
            ui.add_space(20.0);
            if yellow_button(ui, "确认") {
                self.apply_config_form(ui.ctx());
            }
        }
    });
}

pub(crate) fn apply_config_form(&mut self, ctx: &egui::Context) {
    let base_url = self.cfg_base_url.trim().to_string();
    let api_key = self.cfg_api_key.trim().to_string();
    let model = self.cfg_model.trim().to_string();

    let temp: f64 = match self.cfg_temperature.trim().parse() {
        Ok(v) if (0.0..=2.0).contains(&v) => crate::config::round2(v),
        _ => 0.7,
    };
    let max_tokens = if self.cfg_max_tokens.trim().is_empty() {
        None
    } else {
        match self.cfg_max_tokens.trim().parse::<u32>() {
            Ok(v) if v >= 1 => Some(v),
            _ => None,
        }
    };
    let price_in: f64 = self.cfg_price_in.trim().parse().unwrap_or(0.0);
    let price_out: f64 = self.cfg_price_out.trim().parse().unwrap_or(0.0);

    let cfg = crate::config::ApiConfig {
        base_url,
        api_key,
        model,
        temperature: temp,
        max_tokens,
        price_input_per_m: price_in,
        price_output_per_m: price_out,
    };

    if self.cfg_save_default {
        let _ = crate::config::save(&self.config_path, &cfg);
    }
    self.config = cfg;
    self.cfg_show_form = false;
    self.step = Step::HistoryImport;
    self.load_history_list();
}

// ======================== Step 1: Global Requirements ========================

pub(crate) fn ui_global_req(&mut self, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        subheading(ui, "第 1 步：输入全局用户需求");
        info_label(ui, "全局需求对本会话的每一局均生效（如：需要两个化险为夷、优先保护羸弱角色等）。");
        info_label(ui, "输入完成后点击确定。若未输入直接点击确定则说明无全局用户需求。");
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(100.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.session.global_req)
                    .desired_width(1080.0)
                    .desired_rows(4)
                    .text_color(WHITE),
            );
        });
        ui.add_space(20.0);
        if yellow_button(ui, "确定") {
            self.step = Step::MapSelect;
            self.selected_map_stem.clear();
        }
    });
}

// ======================== Step 4: Round Requirements ========================

pub(crate) fn ui_round_req(&mut self, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        subheading(ui, "第 4 步：输入当局用户需求");
        info_label(ui, "当局需求仅对当前这一局生效（如某个角色的特殊安排）。");
        info_label(ui, "输入完成后点击确定。若未输入直接点击确定则说明无当局用户需求。");
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(100.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.round_req)
                    .desired_width(1080.0)
                    .desired_rows(4)
                    .text_color(WHITE),
            );
        });
        ui.add_space(20.0);
        if yellow_button(ui, "确定") {
            self.step = Step::Analyzing;
            self.start_analysis(ui.ctx());
        }
    });
}

// ======================== History Import ========================

pub(crate) fn ui_history_import(&mut self, ui: &mut egui::Ui) {
    if self.history_list.is_empty() {
        self.step = Step::GlobalReq;
        return;
    }
    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        subheading(ui, "检测到历史会话记录");
        info_label(ui, "是否导入历史会话？");
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(480.0);
            if yellow_button(ui, "是") {
            }
            ui.add_space(30.0);
            if yellow_button(ui, "否，直接新开") {
                self.step = Step::GlobalReq;
            }
        });
        ui.add_space(30.0);
        if !self.history_list.is_empty() {
            subheading(ui, "可加载的会话：");
            let mut clicked_idx: Option<usize> = None;
            let n = self.history_list.len();
            for i in 0..n {
                let (p, s) = &self.history_list[i];
                let (pt, ct, cost) = s.totals();
                let summary = format!(
                    "模型 {}  |  {} 局  |  累计 {} tokens  ¥{:.4}  |  全局需求：{}",
                    s.model,
                    s.rounds.len(),
                    pt + ct,
                    cost,
                    if s.global_req.trim().is_empty() {
                        "（无）".to_string()
                    } else {
                        crate::api::truncate_chars(s.global_req.trim(), 30)
                    },
                );
                let label = format!("{}. {}", i + 1, p.file_name().unwrap().to_string_lossy());
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(&label).color(WHITE),
                            )
                            .fill(egui::Color32::from_rgb(30, 38, 52))
                            .min_size(egui::vec2(900.0, 35.0)),
                        )
                        .clicked()
                    {
                        clicked_idx = Some(i);
                    }
                    ui.label(egui::RichText::new(&summary).color(GRAY));
                });
            }
            if let Some(i) = clicked_idx {
                let (p, s) = &self.history_list[i];
                self.session_path = Some(p.clone());
                self.session = s.clone();
                self.session.base_url = self.config.base_url.clone();
                self.session.model = self.config.model.clone();
                self.step = Step::MapSelect;
            }
        }
    });
}

// ======================== Save History ========================

pub(crate) fn ui_save_history(&mut self, ui: &mut egui::Ui) {
    if self.session.rounds.is_empty() && self.show_save_dialog {
        std::process::exit(0);
    }
    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        subheading(ui, "是否保存本次会话历史？");
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(490.0);
            if yellow_button(ui, "保存") {
                self.show_save_dialog = true;
            }
            ui.add_space(30.0);
            if yellow_button(ui, "不保存") {
                self.show_save_dialog = false;
                std::process::exit(0);
            }
        });
        if self.show_save_dialog {
            ui.add_space(30.0);
            info_label(ui, "请为本次历史记录命名（留空则以当前时间命名）：");
            ui.horizontal(|ui| {
                ui.add_space(340.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.save_name_input)
                        .desired_width(600.0)
                        .text_color(WHITE),
                );
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(490.0);
                if yellow_button(ui, "确认保存") {
                    let name = self.save_name_input.trim();
                    let path = if name.is_empty() {
                        crate::history::path_for(&self.history_dir, "")
                    } else {
                        crate::history::path_for(&self.history_dir, name)
                    };
                    match crate::history::save(&path, &self.session) {
                        Ok(()) => {
                            self.session_path = Some(path);
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        Err(e) => {
                            err_label(ui, &format!("保存失败：{e}"));
                        }
                    }
                }
                ui.add_space(20.0);
                if gray_button(ui, "取消") {
                    self.show_save_dialog = false;
                }
            });
        }
    });
}

// ======================== Error Exit ========================

pub(crate) fn ui_error_exit(&mut self, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        let msg = self
            .error_msg
            .as_ref()
            .or(self.analyze_error.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("未知错误");
        err_label(ui, &format!("错误：{msg}"));
        ui.add_space(30.0);

        let is_api_error = self.analyze_error.is_some();
        let is_model_err = self
            .error_msg
            .as_ref()
            .map(|s| s.contains("不适配"))
            .unwrap_or(false);

        if is_api_error && !is_model_err {
            ui.horizontal(|ui| {
                if yellow_button(ui, "重试") {
                    self.analyze_error = None;
                    self.step = Step::Analyzing;
                    self.start_api_call(ui.ctx());
                }
                ui.add_space(20.0);
                if gray_button(ui, "重新输入地图与阵容") {
                    self.analyze_error = None;
                    self.step = Step::MapSelect;
                    self.selected_map_stem.clear();
                    self.selected_char_stems.clear();
                    self.char_page = 0;
                }
            });
        }
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            if !self.session.rounds.is_empty() {
                if gray_button(ui, "保存历史") {
                    self.step = Step::SaveHistory;
                    self.show_save_dialog = false;
                }
                ui.add_space(20.0);
            }
            if gray_button(ui, "退出") {
                if self.session.rounds.is_empty() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.step = Step::SaveHistory;
                    self.show_save_dialog = false;
                }
            }
        });
    });
}

// ======================== Step 2: Map Selection ========================

pub(crate) fn ui_map_select(&mut self, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
    ui.vertical(|ui| {
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            subheading(ui, "第 2 步：从 9 张排位地图中选择当局的地图");
        });
        if self.session_path.is_some() {
            ui.horizontal(|ui| {
                ui.add_space(40.0);
                info_label(ui, &format!("✓ 已导入历史会话（共 {} 局记录）", self.session.rounds.len()));
            });
        }
        ui.add_space(10.0);

        let cell_w = 360.0;
        let cell_h = 170.0;
        let map_ctx = ui.ctx().clone();
        let selected = self.selected_map_stem.clone();

        let mut clicked_stem: Option<String> = None;

        egui::Grid::new("map_grid")
            .num_columns(3)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                for (i, stem) in MAP_ORDER.iter().enumerate() {
                    let map_entry = self.find_map_by_stem(stem);
                    let zh_name = map_entry.map(|e| e.zh.clone()).unwrap_or_else(|| stem.to_string());

                    let handle = self.map_icons.get(stem, &map_ctx).cloned();

                    let is_selected = selected == *stem;

                    ui.allocate_ui(
                        egui::vec2(cell_w, cell_h),
                        |ui| {
                            ui.vertical_centered(|ui| {
                                if let Some(h) = &handle {
                                    let img = egui::Image::from_texture(h)
                                        .max_width(cell_w - 20.0)
                                        .max_height(cell_h - 35.0)
                                        .maintain_aspect_ratio(true)
                                        .sense(egui::Sense::click());
                                    let resp = ui.add(img);
                                    if resp.clicked() {
                                        clicked_stem = Some(stem.to_string());
                                    }
                                    if is_selected {
                                        let rect = resp.rect;
                                        ui.painter().rect_stroke(
                                            rect.expand(3.0),
                                            3.0,
                                            egui::Stroke::new(3.0, YELLOW),
                                            egui::StrokeKind::Outside,
                                        );
                                    }
                                } else {
                                    let _ = ui.allocate_at_least(
                                        egui::vec2(cell_w - 20.0, cell_h - 35.0),
                                        egui::Sense::click(),
                                    );
                                }
                                let color = if is_selected { YELLOW } else { WHITE };
                                ui.label(egui::RichText::new(zh_name).color(color));
                            });
                        },
                    );

                    if (i + 1) % 3 == 0 {
                        ui.end_row();
                    }
                }
            });

        if let Some(stem) = clicked_stem {
            if self.selected_map_stem == stem {
                self.selected_map_stem.clear();
            } else {
                self.selected_map_stem = stem;
            }
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(490.0);
            let enabled = !self.selected_map_stem.is_empty();
            if ui.add_enabled(enabled, egui::Button::new(
                egui::RichText::new("确认").color(if enabled { YELLOW } else { GRAY })
            )
            .fill(egui::Color32::from_rgb(40, 50, 70))
            .stroke(egui::Stroke::new(2.0, if enabled { YELLOW } else { GRAY }))
            .min_size(egui::vec2(120.0, 40.0))).clicked() && enabled
            {
                self.step = Step::CharSelect;
                self.selected_char_stems.clear();
                self.char_page = 0;
            }
        });
    });
}

// ======================== Step 3: Character Selection ========================

pub(crate) fn ui_char_select(&mut self, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
    ui.label(egui::RichText::new("第 3 步：请选择四名角色").color(YELLOW));

    let ctx = ui.ctx().clone();
    let selected = self.selected_char_stems.clone();
    let full = selected.len() >= 4;
    let page_start = self.char_page * 18;

    let mut clicked_stem: Option<String> = None;

    let cell_w = 150.0;
    let cell_h = 150.0;
    let img_size = 110.0;

    // 18 slots per page, laid out as left 3x3 + right 3x3
    egui::Grid::new("char_select_grid")
        .num_columns(6)
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            for row in 0..3 {
                for col in 0..6 {
                    // Left half (col 0-2): fill 0-8 top to bottom, left to right
                    // Right half (col 3-5): fill 9-17 top to bottom, left to right
                    let idx = if col < 3 {
                        page_start + row * 3 + col
                    } else {
                        page_start + 9 + row * 3 + (col - 3)
                    };
                    if idx >= CHAR_ORDER.len() {
                        ui.allocate_space(egui::vec2(cell_w, cell_h));
                        if col == 5 { ui.end_row(); }
                        continue;
                    }
                    let stem = CHAR_ORDER[idx];
                    let entry = self.find_char_by_stem(stem);
                    let zh = entry.map(|e| e.zh.clone()).unwrap_or_else(|| stem.to_string());
                    let handle = self.char_icons.get(stem, &ctx).cloned();
                    let is_sel = selected.contains(&stem.to_string());
                    let disabled = full && !is_sel;

                    ui.allocate_ui(egui::vec2(cell_w, cell_h), |ui| {
                        ui.vertical_centered(|ui| {
                            if let Some(h) = &handle {
                                let tint = if disabled {
                                    egui::Color32::from_rgba_premultiplied(100, 100, 100, 200)
                                } else { egui::Color32::WHITE };
                                let img = egui::Image::from_texture(h)
                                    .max_width(img_size).max_height(img_size)
                                    .maintain_aspect_ratio(true).tint(tint);
                                let resp = ui.add(if disabled {
                                    img.sense(egui::Sense::hover())
                                } else {
                                    img.sense(egui::Sense::click())
                                });
                                if !disabled && resp.clicked() {
                                    clicked_stem = Some(stem.to_string());
                                }
                                if is_sel {
                                    ui.painter().rect_stroke(
                                        resp.rect.expand(2.0), 2.0,
                                        egui::Stroke::new(2.0, YELLOW),
                                        egui::StrokeKind::Outside,
                                    );
                                }
                            }
                            let color = if is_sel { YELLOW } else if disabled { GRAY } else { WHITE };
                            ui.label(egui::RichText::new(zh).color(color).text_style(egui::TextStyle::Small));
                        });
                    });

                    if col == 5 { ui.end_row(); }
                }
            }
        });

    // Separator
    ui.separator();

    // Bottom: pagination (left) + selected characters (right)
    ui.horizontal(|ui| {
        // Pagination
        ui.horizontal(|ui| {
            if ui.add_enabled(self.char_page > 0,
                egui::Button::new(egui::RichText::new("◀").color(WHITE)).min_size(egui::vec2(70.0, 45.0)),
                ).clicked() && self.char_page > 0 { self.char_page -= 1; }
                ui.label(egui::RichText::new(format!("第 {} / 3 页", self.char_page + 1)).color(GRAY).text_style(egui::TextStyle::Small));
                if ui.add_enabled(self.char_page < 2,
                    egui::Button::new(egui::RichText::new("▶").color(WHITE)).min_size(egui::vec2(70.0, 45.0)),
            ).clicked() && self.char_page < 2 { self.char_page += 1; }
        });

        ui.separator();

        // Selected characters
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("已选角色").color(YELLOW));
            for i in 0..4 {
                if i < self.selected_char_stems.len() {
                    let stem = &self.selected_char_stems[i];
                    let entry = self.find_char_by_stem(stem);
                    let zh = entry.map(|e| e.zh.clone()).unwrap_or_else(|| stem.clone());
                    let handle = self.char_icons.get(stem, &ctx).cloned();
                    if let Some(h) = &handle {
                        ui.add(egui::Image::from_texture(h).max_width(40.0).max_height(40.0).maintain_aspect_ratio(true));
                    }
                    ui.label(egui::RichText::new(zh).color(YELLOW).text_style(egui::TextStyle::Small));
                } else {
                    ui.label(egui::RichText::new("（空）").color(GRAY).text_style(egui::TextStyle::Small));
                }
            }
        });

        // Confirm button
        let enabled = self.selected_char_stems.len() == 4;
        if ui.add_enabled(enabled, egui::Button::new(
            egui::RichText::new("确认").color(if enabled { YELLOW } else { GRAY }))
            .fill(egui::Color32::from_rgb(40, 50, 70))
            .stroke(egui::Stroke::new(2.0, if enabled { YELLOW } else { GRAY }))
            .min_size(egui::vec2(140.0, 50.0))).clicked() && enabled
        {
            self.step = Step::RoundReq;
            self.round_req.clear();
        }
    });

    if let Some(stem) = clicked_stem {
        if let Some(pos) = self.selected_char_stems.iter().position(|s| s == &stem) {
            self.selected_char_stems.remove(pos);
        } else if self.selected_char_stems.len() < 4 {
            self.selected_char_stems.push(stem);
        }
    }
}

// ======================== Step 5: Analyzing (with interrupt) ========================

pub(crate) fn ui_analyzing(&mut self, ui: &mut egui::Ui) {
    ui.ctx().request_repaint();
    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
    ui.vertical_centered(|ui| {
        ui.add_space(80.0);
        subheading(ui, "第 5 步：正在调用模型进行分析……");
        ui.add_space(30.0);
        ui.spinner();
        ui.add_space(20.0);
        info_label(ui, &format!(
            "模型：{}  @  {}",
            self.config.model,
            self.config.base_url,
        ));
        ui.add_space(10.0);
        if self.retry_count > 0 {
            warn_label(ui, &format!("正在重试（第 {} 次）……", self.retry_count));
        }
        ui.add_space(40.0);
        if yellow_button(ui, "打断") {
            self.do_interrupt();
        }
        ui.add_space(20.0);
        info_label(ui, "分析期间可随时点击打断按钮停止分析。");
    });
}

// ======================== Step 7: Waiting for Round End ========================

pub(crate) fn ui_waiting_end(&mut self, ui: &mut egui::Ui) {
    self.ui_result_display_inner(ui, true);
}

// ======================== Step 8: Ask Next Round ========================

pub(crate) fn ui_ask_next(&mut self, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        if let Some(u) = self.round_usage {
            subheading(ui, &format!(
                "本局 Token 用量：输入 {} + 输出 {} = {} tokens{}",
                u.prompt_tokens,
                u.completion_tokens,
                u.total(),
                if self.round_estimated { "（按字符估算）" } else { "" },
            ));
            info_label(ui, &format!(
                "费用：{} / 1M × ¥{:.4} + {} / 1M × ¥{:.4} = ¥{:.4}",
                u.prompt_tokens,
                self.config.price_input_per_m,
                u.completion_tokens,
                self.config.price_output_per_m,
                self.round_cost,
            ));
            ui.add_space(20.0);
            let (pt, ct, cost) = self.session.totals();
            info_label(ui, &format!(
                "本次会话累计：输入 {} tokens，输出 {} tokens，费用 ¥{:.4}（共 {} 局）",
                pt, ct, cost, self.session.rounds.len(),
            ));
        } else {
            info_label(ui, "本局未产生 Token 用量。");
        }
        ui.add_space(40.0);
        subheading(ui, "是否开启下一对局？");
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(490.0);
            if yellow_button(ui, "是") {
                self.selected_map_stem.clear();
                self.selected_char_stems.clear();
                self.char_page = 0;
                self.round_req.clear();
                self.plans.clear();
                self.current_plan_idx = 0;
                self.plan_confirmed = false;
                self.round_usage = None;
                self.round_cost = 0.0;
                self.step = Step::MapSelect;
            }
            ui.add_space(30.0);
            if yellow_button(ui, "否") {
                self.step = Step::SaveHistory;
                self.show_save_dialog = false;
            }
        });
    });
}

// ======================== Step 6: Result Display ========================

pub(crate) fn ui_result_display(&mut self, ui: &mut egui::Ui) {
    self.ui_result_display_inner(ui, false);
}

pub(crate) fn ui_result_display_inner(&mut self, ui: &mut egui::Ui, locked: bool) {
    let ctx = ui.ctx().clone();
    let map_stem = self.selected_map_stem.clone();
    let cur_plan_idx = self.current_plan_idx;
    let plan = self.plans.get(cur_plan_idx).cloned();
    let map_stem_for_pos = map_stem.clone();
    let positions = self.load_positions(&map_stem_for_pos).cloned();

    let mut char_textures: std::collections::HashMap<String, egui::TextureHandle> = std::collections::HashMap::new();
    for stem in &self.selected_char_stems {
        let stem_clone = stem.clone();
        let zh = self.find_char_by_stem(&stem_clone).map(|e| e.zh.clone());
        if let Some(zh) = zh {
            if let Some(h) = self.char_icons.get(&stem_clone, &ctx) {
                char_textures.insert(zh, h.clone());
            }
        }
    }

    let mut talent_textures: std::collections::HashMap<String, egui::TextureHandle> = std::collections::HashMap::new();
    if let Some(ref p) = plan {
        for sel in &p.selections {
            for t in &sel.talents {
                let stem = talent_stem(t).to_string();
                if let Some(h) = self.persona_icons.get(&stem, &ctx) {
                    talent_textures.insert(t.clone(), h.clone());
                }
            }
        }
    }

    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
    ui.horizontal_top(|ui| {
        // ===== Left: area selection map (composed with character icons) =====
        ui.vertical(|ui| {
            let composed = compose_map_image(
                &self.icons_dir().join("area-selection"),
                map_stem.as_str(),
                &self.icons_dir().join("characters"),
                &self.db,
                plan.as_ref(),
                positions.as_ref(),
            );

            if let Some(color_img) = composed {
                let handle = ctx.load_texture("area_map_composed", color_img, egui::TextureOptions::LINEAR);
                ui.add(
                    egui::Image::from_texture(&handle)
                        .max_width(750.0)
                        .max_height(600.0)
                        .maintain_aspect_ratio(true)
                );
            } else {
                ui.label(egui::RichText::new("地图加载失败").color(egui::Color32::RED));
            }
        });

        ui.separator();

        // ===== Right: talent + description + buttons =====
        ui.vertical(|ui| {
            // -- Talent selection --
            if locked {
                ui.label(egui::RichText::new(format!("方案 {}（已确认）", cur_plan_idx + 1)).color(YELLOW));
            } else {
                ui.label(egui::RichText::new("天赋选择").color(YELLOW));
            }

            if let Some(ref plan) = plan {
                for sel in &plan.selections {
                    ui.horizontal(|ui| {
                        if let Some(h) = char_textures.get(&sel.character) {
                            ui.add(egui::Image::from_texture(h).max_width(45.0).max_height(45.0).maintain_aspect_ratio(true));
                        } else {
                            ui.allocate_space(egui::vec2(45.0, 45.0));
                        }
                        ui.vertical(|ui| {
                            for t in &sel.talents {
                                if let Some(h) = talent_textures.get(t) {
                                    ui.add(egui::Image::from_texture(h).max_width(25.0).max_height(25.0).maintain_aspect_ratio(true));
                                    ui.add_space(1.0);
                                } else {
                                    ui.allocate_space(egui::vec2(25.0, 25.0));
                                }
                            }
                        });
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&sel.character).color(YELLOW).text_style(egui::TextStyle::Small));
                            ui.label(egui::RichText::new(sel.talents.join("、")).color(WHITE).text_style(egui::TextStyle::Small));
                            ui.label(egui::RichText::new(format!("{}号选点", sel.point)).color(GRAY).text_style(egui::TextStyle::Small));
                        });
                    });
                    ui.separator();
                }
            }

            // -- Description --
            ui.label(egui::RichText::new("分析与说明").color(YELLOW));
            if let Some(ref plan) = plan {
                ui.label(egui::RichText::new(&plan.description).color(WHITE).text_style(egui::TextStyle::Small));
            }

            // -- Pagination + confirm --
            ui.horizontal(|ui| {
                if ui.add_enabled(
                    self.current_plan_idx > 0,
                    egui::Button::new(egui::RichText::new("◀").color(WHITE))
                        .min_size(egui::vec2(50.0, 30.0)),
                ).clicked() && self.current_plan_idx > 0 {
                    self.current_plan_idx -= 1;
                }
                ui.label(egui::RichText::new(
                    format!("{} / {}", self.current_plan_idx + 1, self.plans.len())
                ).color(YELLOW).text_style(egui::TextStyle::Small));
                if ui.add_enabled(
                    self.current_plan_idx + 1 < self.plans.len(),
                    egui::Button::new(egui::RichText::new("▶").color(WHITE))
                        .min_size(egui::vec2(50.0, 30.0)),
                ).clicked() && self.current_plan_idx + 1 < self.plans.len() {
                    self.current_plan_idx += 1;
                }
            });

            if locked {
                ui.horizontal_centered(|ui| {
                    if yellow_button(ui, "对局结束") {
                        self.save_round_to_session();
                        self.step = Step::AskNext;
                    }
                });
            } else {
                ui.horizontal_centered(|ui| {
                    if yellow_button(ui, "确认使用") {
                        self.plan_confirmed = true;
                        self.step = Step::WaitingEnd;
                    }
                });
            }
        });
    });
}
}

// ======================== Image Composition ========================

fn compose_map_image(
    area_dir: &std::path::Path,
    map_stem: &str,
    char_dir: &std::path::Path,
    db: &crate::checker::ResourceDb,
    plan: Option<&crate::parser::Plan>,
    positions: Option<&crate::positions::MapPosition>,
) -> Option<egui::ColorImage> {
    // Load map
    let map_path = area_dir.join(format!("{map_stem}.png"));
    let map_img = image::open(&map_path).ok()?;
    let mut buf = map_img.to_rgba8();
    let map_w = buf.width();
    let map_h = buf.height();

    // If no positions or plan, return raw map
    let (pos, plan) = match (positions, plan) {
        (Some(p), Some(pl)) => (p, pl),
        _ => {
            let pixels: Vec<egui::Color32> = buf.pixels().cloned().map(|p| {
                egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])
            }).collect();
            return Some(egui::ColorImage::new([map_w as usize, map_h as usize], pixels));
        }
    };

    let icon_size = pos.icon_size.max(1) as u32;

    // For each character in the plan, composite onto the map
    for sel in &plan.selections {
        if let Some((cx, cy)) = pos.point_center(sel.point) {
            // Find character's English stem
            let stem = match db.survivors.iter().find(|e| e.zh == sel.character) {
                Some(e) => e.stem.as_str(),
                None => continue,
            };

            // Load character icon
            let icon_path = char_dir.join(format!("{stem}.png"));
            let icon_img = match image::open(&icon_path) {
                Ok(img) => img.to_rgba8(),
                Err(_) => continue,
            };

            // Resize icon to icon_size x icon_size if needed
            let icon_resized = if icon_img.width() != icon_size || icon_img.height() != icon_size {
                image::imageops::resize(&icon_img, icon_size, icon_size, image::imageops::FilterType::Lanczos3)
            } else {
                icon_img
            };

            // Composite: center icon at (cx, cy)
            let half = icon_size / 2;
            let start_x = cx.saturating_sub(half);
            let start_y = cy.saturating_sub(half);

            for iy in 0..icon_resized.height() {
                for ix in 0..icon_resized.width() {
                    let px = start_x + ix;
                    let py = start_y + iy;
                    if px >= map_w || py >= map_h {
                        continue;
                    }
                    let src = icon_resized.get_pixel(ix, iy);
                    if src[3] == 0 { continue; }  // skip transparent
                    let dst = buf.get_pixel_mut(px, py);
                    let alpha = src[3] as f32 / 255.0;
                    dst[0] = (src[0] as f32 * alpha + dst[0] as f32 * (1.0 - alpha)) as u8;
                    dst[1] = (src[1] as f32 * alpha + dst[1] as f32 * (1.0 - alpha)) as u8;
                    dst[2] = (src[2] as f32 * alpha + dst[2] as f32 * (1.0 - alpha)) as u8;
                    dst[3] = 255;
                }
            }
        }
    }

    let pixels: Vec<egui::Color32> = buf.pixels().cloned().map(|p| {
        egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])
    }).collect();
    Some(egui::ColorImage::new([map_w as usize, map_h as usize], pixels))
}
