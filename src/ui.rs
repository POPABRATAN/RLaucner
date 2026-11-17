use crate::app::*;
use crate::config::*;
use eframe::egui::{self, Align, Color32, Frame, Layout, Margin, RichText, Rounding, Sense, Stroke, Ui, Vec2};

pub const BLUE: Color32 = Color32::from_rgb(43, 155, 224);
pub const BLUE_DARK: Color32 = Color32::from_rgb(28, 112, 176);
pub const GREEN: Color32 = Color32::from_rgb(112, 170, 64);
pub const YELLOW: Color32 = Color32::from_rgb(234, 188, 62);
pub const BTN_GREEN: Color32 = Color32::from_rgb(34, 177, 90);
pub const ORANGE: Color32 = Color32::from_rgb(232, 120, 52);

pub fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.selection.bg_fill = BLUE;
    v.hyperlink_color = BLUE_DARK;
    v.window_rounding = Rounding::same(2.0);
    ctx.set_visuals(v);
    // шрифт с кириллицей получше, если есть в системе
    let mut fonts = egui::FontDefinitions::default();
    for path in ["C:\\Windows\\Fonts\\segoeui.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert("sys".into(), egui::FontData::from_owned(bytes));
            fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "sys".into());
            break;
        }
    }
    ctx.set_fonts(fonts);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 5.0);
    });
}

#[derive(Clone, Copy)]
pub enum Icon {
    Gear,
    Folder,
    Refresh,
    Back,
    Search,
}

pub fn icon_button(ui: &mut Ui, icon: Icon, color: Color32, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, Color32::from_white_alpha(45));
    }
    let c = rect.center();
    let p = ui.painter();
    let s = Stroke::new(2.0, color);
    use std::f32::consts::TAU;
    match icon {
        Icon::Gear => {
            p.circle_stroke(c, size * 0.22, s);
            p.circle_stroke(c, size * 0.09, Stroke::new(1.5, color));
            for k in 0..8 {
                let a = k as f32 * TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + d * size * 0.27, c + d * size * 0.37], Stroke::new(3.0, color));
            }
        }
        Icon::Folder => {
            let r = egui::Rect::from_center_size(c + Vec2::new(0.0, 2.0), Vec2::new(size * 0.62, size * 0.42));
            p.rect_stroke(r, 2.0, s);
            p.line_segment([r.left_top(), r.left_top() + Vec2::new(0.0, -4.0)], s);
            p.line_segment([r.left_top() + Vec2::new(0.0, -4.0), r.left_top() + Vec2::new(size * 0.2, -4.0)], s);
        }
        Icon::Refresh => {
            let r = size * 0.28;
            let pts: Vec<egui::Pos2> = (0..=24)
                .map(|i| {
                    let a = (40.0 + i as f32 * 280.0 / 24.0).to_radians();
                    c + Vec2::new(a.cos(), a.sin()) * r
                })
                .collect();
            let a = 320.0f32.to_radians();
            let end = *pts.last().unwrap();
            let n = Vec2::new(a.cos(), a.sin());
            let t = Vec2::new(-a.sin(), a.cos());
            p.add(egui::Shape::line(pts, Stroke::new(2.5, color)));
            p.add(egui::Shape::convex_polygon(vec![end + t * 7.0, end + n * 5.0, end - n * 5.0], color, Stroke::NONE));
        }
        Icon::Back => {
            let w = size * 0.22;
            p.line_segment([c + Vec2::new(w, 0.0), c - Vec2::new(w, 0.0)], s);
            p.line_segment([c - Vec2::new(w, 0.0), c + Vec2::new(-w * 0.2, -w * 0.8)], s);
            p.line_segment([c - Vec2::new(w, 0.0), c + Vec2::new(-w * 0.2, w * 0.8)], s);
        }
        Icon::Search => {
            p.circle_stroke(c - Vec2::splat(2.0), size * 0.17, s);
            p.line_segment([c + Vec2::splat(size * 0.1), c + Vec2::splat(size * 0.27)], Stroke::new(3.0, color));
        }
    }
    resp
}

pub fn flat_button(ui: &mut Ui, text: &str, fill: Color32, color: Color32, min: Vec2) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(color).size(14.0))
            .fill(fill)
            .stroke(Stroke::NONE)
            .rounding(2.0)
            .min_size(min),
    )
}

/// Фон: небо и пиксельный рельеф (нарисовано кодом, оригинальное).
pub fn paint_bg(p: &egui::Painter, r: egui::Rect) {
    let top = Color32::from_rgb(62, 124, 190);
    let bot = Color32::from_rgb(168, 208, 236);
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bot);
    mesh.colored_vertex(r.left_bottom(), bot);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
    let bs = 32.0;
    for (cx, cy, w) in [(0.12, 0.14, 4), (0.45, 0.08, 5), (0.78, 0.2, 3)] {
        for k in 0..w {
            let x = r.left() + r.width() * cx + k as f32 * bs;
            let y = r.top() + r.height() * cy;
            p.rect_filled(egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(bs, bs * 0.7)), 0.0, Color32::from_white_alpha(200));
            if k % 2 == 0 {
                p.rect_filled(egui::Rect::from_min_size(egui::pos2(x + 6.0, y - bs * 0.5), Vec2::new(bs, bs * 0.5)), 0.0, Color32::from_white_alpha(200));
            }
        }
    }
    let cols = (r.width() / bs).ceil() as i32 + 1;
    for i in 0..cols {
        let h = (((i as f32 * 0.37).sin() * 2.0 + (i as f32 * 0.13).cos() * 3.0 + 5.0).round() as i32).max(2);
        for j in 0..h {
            let y = r.bottom() - (j + 1) as f32 * bs;
            let depth = h - 1 - j;
            let col = if depth == 0 {
                Color32::from_rgb(88, 160, 58)
            } else if depth < 3 {
                Color32::from_rgb(121, 85, 52)
            } else {
                Color32::from_rgb(112, 112, 118)
            };
            let shade = if (i + j) % 2 == 0 { 0 } else { 10 };
            let col = Color32::from_rgb(col.r().saturating_sub(shade), col.g().saturating_sub(shade), col.b().saturating_sub(shade));
            p.rect_filled(egui::Rect::from_min_size(egui::pos2(r.left() + i as f32 * bs, y), Vec2::splat(bs)), 0.0, col);
        }
    }
    p.rect_filled(r, 0.0, Color32::from_black_alpha(50));
}

pub fn fmt_downloads(n: u64) -> String {
    if n >= 1_000_000 {
        let s = format!("{:.1}", n as f64 / 1_000_000.0);
        format!("{} млн.", s.trim_end_matches('0').trim_end_matches('.'))
    } else if n >= 1000 {
        format!("{} тыс.", n / 1000)
    } else {
        n.to_string()
    }
}

pub fn fmt_date(s: &str) -> String {
    let months = ["янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек"];
    let p: Vec<&str> = s.get(..10).unwrap_or("").split('-').collect();
    if p.len() == 3 {
        if let (Ok(m), Ok(d)) = (p[1].parse::<usize>(), p[2].parse::<u32>()) {
            if (1..=12).contains(&m) {
                return format!("{d:02} {} {}", months[m - 1], p[0]);
            }
        }
    }
    s.to_string()
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        self.pump(ctx);
        if self.screen == Screen::Splash {
            self.draw_splash(ctx);
            return;
        }
        if !self.first_run_checked {
            self.first_run_checked = true;
            if self.cfg.accounts.is_empty() {
                self.show_accounts = true;
            } else if self.cfg.profiles.is_empty() {
                self.show_versions = true;
            }
        }
        self.draw_bottom(ctx);
        if self.screen == Screen::Main {
            egui::SidePanel::right("side")
                .exact_width(236.0)
                .resizable(false)
                .frame(Frame::none().fill(Color32::from_rgb(30, 100, 160)).inner_margin(Margin::same(12.0)))
                .show(ctx, |ui| self.draw_sidebar(ui));
        }
        egui::CentralPanel::default().frame(Frame::none()).show(ctx, |ui| {
            paint_bg(ui.painter(), ui.max_rect());
            match self.screen {
                Screen::Main => self.draw_main(ui),
                Screen::Catalog => self.draw_catalog(ui, ctx),
                Screen::Settings => self.draw_settings(ui),
                Screen::Splash => {}
            }
        });
        self.draw_dialogs(ctx);
        self.draw_toast(ctx);
        if self.busy || self.cat.loading || self.ms_running || self.cv.fetching {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }
}

impl App {
    fn draw_splash(&mut self, ctx: &egui::Context) {
        let t = self.started.elapsed().as_secs_f32();
        egui::CentralPanel::default().frame(Frame::none().fill(BLUE)).show(ctx, |ui| {
            let r = ui.max_rect();
            let p = ui.painter();
            p.text(r.center() - Vec2::new(0.0, 14.0), egui::Align2::CENTER_CENTER, "RLAUNCHER", egui::FontId::proportional(40.0), Color32::WHITE);
            p.text(r.center() + Vec2::new(0.0, 22.0), egui::Align2::CENTER_CENTER, "THE LAUNCHER FOR MINECRAFT", egui::FontId::proportional(11.0), Color32::from_white_alpha(220));
            let badge = egui::Rect::from_min_size(egui::pos2(r.right() - 64.0, r.top()), Vec2::new(64.0, 18.0));
            p.rect_filled(badge, 0.0, BLUE_DARK);
            p.text(badge.center(), egui::Align2::CENTER_CENTER, APP_VERSION, egui::FontId::proportional(11.0), Color32::WHITE);
            let track = egui::Rect::from_min_size(egui::pos2(r.left(), r.bottom() - 22.0), Vec2::new(r.width(), 22.0));
            p.rect_filled(track, 0.0, Color32::from_gray(225));
            let ready = self.manifest_done && t > 1.6;
            let frac = if ready { 1.0 } else { (t / 1.8).min(0.92) };
            p.rect_filled(egui::Rect::from_min_size(track.min, Vec2::new(track.width() * frac, track.height())), 0.0, Color32::from_rgb(20, 160, 220));
            if ready {
                self.screen = Screen::Main;
                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(1050.0, 690.0)));
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(Vec2::new(900.0, 600.0)));
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("RLauncher {APP_VERSION}")));
                if let Some(cmd) = egui::ViewportCommand::center_on_screen(ctx) {
                    ctx.send_viewport_cmd(cmd);
                }
            }
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }

    fn draw_bottom(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bar")
            .exact_height(78.0)
            .frame(Frame::none().fill(GREEN).inner_margin(Margin::symmetric(18.0, 8.0)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // аккаунты
                    ui.vertical(|ui| {
                        let txt = self.cfg.account().map(|a| a.name.clone()).unwrap_or_else(|| "Аккаунты".into());
                        egui::ComboBox::from_id_salt("acc").width(246.0).selected_text(txt).show_ui(ui, |ui| {
                            let accs = self.cfg.accounts.clone();
                            for (i, a) in accs.iter().enumerate() {
                                let tag = if a.kind == AccountKind::Microsoft { " (Microsoft)" } else { " (офлайн)" };
                                if ui.selectable_label(self.cfg.selected_account == i, format!("{}{tag}", a.name)).clicked() {
                                    self.cfg.selected_account = i;
                                    self.cfg.save();
                                }
                            }
                            ui.separator();
                            if ui.button("＋ Добавить / управлять аккаунтами…").clicked() {
                                self.show_accounts = true;
                            }
                        });
                        let mut sa = self.show_accounts;
                        if ui.checkbox(&mut sa, RichText::new("Аккаунты").color(Color32::WHITE).size(12.0)).changed() {
                            self.show_accounts = sa;
                        }
                    });
                    ui.add_space(8.0);
                    // версии
                    ui.vertical(|ui| {
                        let txt = self.cfg.profile().map(|p| p.name.clone()).unwrap_or_else(|| "Версии".into());
                        egui::ComboBox::from_id_salt("ver").width(246.0).selected_text(txt).show_ui(ui, |ui| {
                            let profs = self.cfg.profiles.clone();
                            for (i, p) in profs.iter().enumerate() {
                                if ui.selectable_label(self.cfg.selected_profile == Some(i), format!("{}  ·  {} {}", p.name, p.loader.short(), p.mc_version)).clicked() {
                                    self.cfg.selected_profile = Some(i);
                                    self.cfg.save();
                                }
                            }
                            ui.separator();
                            if ui.button("＋ Создать версию / управлять…").clicked() {
                                self.show_versions = true;
                            }
                        });
                        ui.checkbox(&mut self.force_update, RichText::new("Обновить клиент").color(Color32::WHITE).size(12.0));
                    });
                    ui.add_space(14.0);
                    // играть
                    let label = if self.busy { self.busy_label.clone() } else { "Войти в игру".to_string() };
                    let btn = egui::Button::new(RichText::new(label).size(20.0).strong().color(Color32::WHITE))
                        .fill(YELLOW)
                        .stroke(Stroke::NONE)
                        .rounding(2.0)
                        .min_size(Vec2::new(250.0, 54.0));
                    if ui.add_enabled(!self.busy, btn).clicked() {
                        self.start_launch(ctx, None);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, Icon::Gear, Color32::WHITE, 40.0).on_hover_text("Настройки лаунчера").clicked() {
                            self.screen = if self.screen == Screen::Settings { Screen::Main } else { Screen::Settings };
                        }
                        if icon_button(ui, Icon::Folder, Color32::WHITE, 40.0).on_hover_text("Открыть папку игры").clicked() {
                            let _ = std::fs::create_dir_all(&self.cfg.root_dir);
                            let _ = open::that(&self.cfg.root_dir);
                        }
                        if icon_button(ui, Icon::Refresh, Color32::WHITE, 40.0).on_hover_text("Обновить список версий").clicked() {
                            self.manifest_done = false;
                            let (http, tx, c2) = (self.http.clone(), self.tx.clone(), ctx.clone());
                            self.rt.spawn(async move {
                                let r = crate::mojang::fetch_manifest(&http).await.map_err(|e| format!("{e:#}"));
                                let _ = tx.send(Msg::Manifest(r));
                                c2.request_repaint();
                            });
                            self.toast("Список версий обновляется…");
                        }
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(52.0, 52.0), Sense::click());
                        let p = ui.painter();
                        if resp.hovered() {
                            p.rect_filled(rect, 4.0, Color32::from_white_alpha(45));
                        }
                        p.text(rect.center() - Vec2::new(0.0, 9.0), egui::Align2::CENTER_CENTER, "RL", egui::FontId::proportional(26.0), Color32::WHITE);
                        p.text(rect.center() + Vec2::new(0.0, 13.0), egui::Align2::CENTER_CENTER, "MODS", egui::FontId::proportional(12.0), Color32::from_rgb(200, 30, 30));
                        if resp.on_hover_text("RL Mods — каталог Modrinth").clicked() {
                            self.open_catalog(ctx);
                        }
                    });
                });
            });
        if self.busy {
            let pr = self.prog.lock().map(|g| g.clone()).unwrap_or_default();
            egui::TopBottomPanel::bottom("prog")
                .exact_height(26.0)
                .frame(Frame::none().fill(Color32::from_rgb(40, 40, 46)).inner_margin(Margin::symmetric(14.0, 4.0)))
                .show(ctx, |ui| {
                    let text = if pr.total > 0 { format!("{}  {}/{}", pr.text, pr.done, pr.total) } else { pr.text.clone() };
                    let frac = if pr.total > 0 { pr.done as f32 / pr.total as f32 } else { 0.0 };
                    ui.add(egui::ProgressBar::new(frac).desired_width(ui.available_width()).fill(BTN_GREEN).text(RichText::new(text).size(12.0).color(Color32::WHITE)));
                });
        }
    }

    pub fn open_catalog(&mut self, ctx: &egui::Context) {
        if let Some(p) = self.cfg.profile() {
            if self.cat.game_version.is_empty() {
                self.cat.game_version = p.mc_version.clone();
            }
        }
        self.cat.last_key = None;
        self.screen = Screen::Catalog;
        ctx.request_repaint();
    }

    fn draw_toast(&mut self, ctx: &egui::Context) {
        let Some((msg, t)) = self.toast.clone() else { return };
        let secs = 5.0 + (msg.len() as f32 / 60.0);
        if t.elapsed().as_secs_f32() > secs {
            self.toast = None;
            return;
        }
        egui::Area::new("toast".into()).anchor(egui::Align2::LEFT_BOTTOM, [16.0, -100.0]).order(egui::Order::Foreground).show(ctx, |ui| {
            Frame::none().fill(Color32::from_rgb(35, 38, 45)).inner_margin(12.0).rounding(4.0).show(ui, |ui| {
                ui.set_max_width(560.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(msg).color(Color32::WHITE).size(13.0));
                    if ui.small_button("OK").clicked() {
                        self.toast = None;
                    }
                });
            });
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}
