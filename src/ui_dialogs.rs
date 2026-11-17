use crate::app::*;
use crate::config::*;
use crate::ui::*;
use eframe::egui::{self, Align2, Color32, RichText, Vec2};

impl App {
    pub fn draw_dialogs(&mut self, ctx: &egui::Context) {
        self.dialog_versions(ctx);
        self.dialog_accounts(ctx);
        self.dialog_server(ctx);
        self.dialog_help(ctx);
    }

    fn dialog_versions(&mut self, ctx: &egui::Context) {
        if !self.show_versions {
            return;
        }
        let mut open = true;
        egui::Window::new("Версии").open(&mut open).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).fixed_size([760.0, 470.0]).show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                // левая колонка: установленные
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    ui.label(RichText::new("Мои версии").strong().size(16.0));
                    egui::ScrollArea::vertical().id_salt("vlist").max_height(400.0).show(ui, |ui| {
                        if self.cfg.profiles.is_empty() {
                            ui.label("Пока пусто. Создайте первую версию справа →");
                        }
                        let profs = self.cfg.profiles.clone();
                        let mut del = None;
                        for (i, p) in profs.iter().enumerate() {
                            let sel = self.cfg.selected_profile == Some(i);
                            egui::Frame::none().fill(if sel { Color32::from_rgb(214, 236, 250) } else { Color32::from_gray(245) }).inner_margin(8.0).show(ui, |ui| {
                                ui.set_width(270.0);
                                ui.label(RichText::new(&p.name).strong());
                                let lv = if p.loader_version.is_empty() { String::new() } else { format!(" {}", p.loader_version) };
                                ui.label(RichText::new(format!("{}{lv} · Minecraft {}", p.loader.short(), p.mc_version)).size(12.0).color(Color32::GRAY));
                                ui.horizontal(|ui| {
                                    if ui.small_button(if sel { "Выбрана" } else { "Выбрать" }).clicked() {
                                        self.cfg.selected_profile = Some(i);
                                        self.cfg.save();
                                    }
                                    if ui.small_button("Папка").clicked() {
                                        let _ = std::fs::create_dir_all(&p.game_dir);
                                        let _ = open::that(&p.game_dir);
                                    }
                                    if ui.small_button("Убрать из списка").on_hover_text("Файлы на диске останутся").clicked() {
                                        del = Some(i);
                                    }
                                });
                            });
                            ui.add_space(4.0);
                        }
                        if let Some(i) = del {
                            self.cfg.profiles.remove(i);
                            self.cfg.selected_profile = if self.cfg.profiles.is_empty() { None } else { Some(0) };
                            self.cfg.save();
                        }
                    });
                });
                ui.separator();
                // правая колонка: создание
                ui.vertical(|ui| {
                    ui.set_width(400.0);
                    ui.label(RichText::new("Создать версию").strong().size(16.0));
                    ui.add_space(4.0);
                    ui.label("Название:");
                    ui.add(egui::TextEdit::singleline(&mut self.cv.name).hint_text(format!("{} {}", self.cv.loader.short(), self.cv.mc)).desired_width(380.0));
                    ui.label("Тип (загрузчик):");
                    egui::ComboBox::from_id_salt("cv_loader").width(380.0).selected_text(self.cv.loader.label()).show_ui(ui, |ui| {
                        for l in Loader::ALL {
                            ui.selectable_value(&mut self.cv.loader, l, l.label());
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Версия Minecraft:");
                        if ui.small_button(if self.cv.newest_first { "новые → старые" } else { "старые → новые" }).clicked() {
                            self.cv.newest_first = !self.cv.newest_first;
                        }
                    });
                    ui.add(egui::TextEdit::singleline(&mut self.cv.filter).hint_text("Фильтр, например 1.20").desired_width(380.0));
                    let vers = self.visible_versions();
                    egui::ComboBox::from_id_salt("cv_mc").width(380.0).selected_text(if self.cv.mc.is_empty() { "выберите…".to_string() } else { self.cv.mc.clone() }).show_ui(ui, |ui| {
                        let f = self.cv.filter.trim().to_lowercase();
                        for v in vers.iter().filter(|v| f.is_empty() || v.id.to_lowercase().contains(&f)) {
                            let kind = match v.kind.as_str() {
                                "snapshot" => "  (снапшот)",
                                "old_beta" => "  (beta)",
                                "old_alpha" => "  (alpha)",
                                _ => "",
                            };
                            if ui.selectable_label(self.cv.mc == v.id, format!("{}{kind}", v.id)).clicked() {
                                self.cv.mc = v.id.clone();
                            }
                        }
                    });
                    if self.cv.loader != Loader::Vanilla {
                        ui.label(format!("Версия {}:", self.cv.loader.short()));
                        let key = (self.cv.loader, self.cv.mc.clone());
                        if self.cv.fetched.as_ref() != Some(&key) && !self.cv.fetching && !self.cv.mc.is_empty() {
                            self.fetch_loader_versions(ctx);
                        }
                        if self.cv.fetching {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("загрузка списка…");
                            });
                        } else if self.cv.lvs.is_empty() {
                            ui.colored_label(Color32::from_rgb(190, 40, 40), self.cv.fetch_err.clone().unwrap_or_else(|| format!("Для Minecraft {} нет версий {}.", self.cv.mc, self.cv.loader.short())));
                        } else {
                            egui::ComboBox::from_id_salt("cv_lv").width(380.0).selected_text(self.cv.lv.clone()).show_ui(ui, |ui| {
                                for v in self.cv.lvs.clone() {
                                    ui.selectable_value(&mut self.cv.lv, v.clone(), v);
                                }
                            });
                        }
                    }
                    ui.add_space(8.0);
                    let can = !self.busy && !self.cv.mc.is_empty() && (self.cv.loader == Loader::Vanilla || !self.cv.lv.is_empty());
                    let btn = egui::Button::new(RichText::new(if self.busy { "Идёт установка…" } else { "Создать и установить" }).color(Color32::WHITE).size(15.0)).fill(BTN_GREEN).min_size(Vec2::new(380.0, 34.0));
                    if ui.add_enabled(can, btn).clicked() {
                        self.create_version(ctx);
                    }
                    ui.label(RichText::new("Будут скачаны игра, библиотеки, ресурсы и выбранный загрузчик.").size(12.0).color(Color32::GRAY));
                });
            });
        });
        self.show_versions = open;
    }

    fn dialog_accounts(&mut self, ctx: &egui::Context) {
        if !self.show_accounts {
            return;
        }
        let mut open = true;
        egui::Window::new("Аккаунты").open(&mut open).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).fixed_size([520.0, 440.0]).show(ctx, |ui| {
            ui.label(RichText::new("Ваши аккаунты").strong().size(16.0));
            let accs = self.cfg.accounts.clone();
            let mut del = None;
            egui::ScrollArea::vertical().id_salt("alist").max_height(130.0).show(ui, |ui| {
                if accs.is_empty() {
                    ui.label("Аккаунтов нет. Добавьте офлайн-ник или войдите через Microsoft.");
                }
                for (i, a) in accs.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let tag = if a.kind == AccountKind::Microsoft { "Microsoft" } else { "офлайн" };
                        if ui.selectable_label(self.cfg.selected_account == i, format!("{}  ({tag})", a.name)).clicked() {
                            self.cfg.selected_account = i;
                            self.cfg.save();
                        }
                        if ui.small_button("Удалить").clicked() {
                            del = Some(i);
                        }
                    });
                }
            });
            if let Some(i) = del {
                self.cfg.accounts.remove(i);
                self.cfg.selected_account = 0;
                self.cfg.save();
            }
            ui.separator();
            ui.label(RichText::new("Офлайн-аккаунт").strong());
            ui.label(RichText::new("Достаточно ника. Работает в одиночной игре и на серверах без проверки лицензии.").size(12.0).color(Color32::GRAY));
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.acc_name).hint_text("Ник (3–16: латиница, цифры, _)").desired_width(300.0));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if flat_button(ui, "Добавить", BTN_GREEN, Color32::WHITE, Vec2::new(100.0, 26.0)).clicked() || enter {
                    let n = self.acc_name.trim().to_string();
                    self.add_offline(&n);
                    if auth_ok(&n) {
                        self.acc_name.clear();
                    }
                }
            });
            ui.separator();
            ui.label(RichText::new("Аккаунт Microsoft (лицензия)").strong());
            if let Some(dc) = self.ms_device.clone() {
                ui.label("Откройте страницу (она уже открылась в браузере) и введите код:");
                ui.label(RichText::new(&dc.user_code).size(30.0).strong().color(BLUE_DARK));
                ui.horizontal(|ui| {
                    if ui.button("Открыть страницу входа").clicked() {
                        let _ = open::that(&dc.verification_uri);
                    }
                    if ui.button("Копировать код").clicked() {
                        ctx.copy_text(dc.user_code.clone());
                    }
                });
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Ожидание входа…");
                });
            } else {
                if self.cfg.ms_client_id.trim().is_empty() {
                    ui.colored_label(Color32::from_rgb(190, 90, 20), "Нужен Azure Client ID: Настройки → Настройки RLauncher.");
                }
                if ui.add_enabled(!self.ms_running, egui::Button::new(RichText::new("Войти через Microsoft").color(Color32::WHITE)).fill(BLUE).min_size(Vec2::new(240.0, 30.0))).clicked() {
                    self.start_ms_login(ctx);
                }
            }
        });
        self.show_accounts = open;
    }

    fn dialog_server(&mut self, ctx: &egui::Context) {
        if !self.show_add_server {
            return;
        }
        let mut open = true;
        egui::Window::new("Добавить сервер").open(&mut open).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).fixed_size([360.0, 160.0]).show(ctx, |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.srv_name).hint_text("Название").desired_width(330.0));
            ui.add(egui::TextEdit::singleline(&mut self.srv_addr).hint_text("Адрес, например play.example.net:25565").desired_width(330.0));
            if flat_button(ui, "Сохранить", BTN_GREEN, Color32::WHITE, Vec2::new(330.0, 30.0)).clicked() {
                let (n, a) = (self.srv_name.trim().to_string(), self.srv_addr.trim().to_string());
                if a.is_empty() {
                    self.toast("Укажите адрес сервера");
                } else {
                    self.cfg.servers.push(ServerEntry { name: if n.is_empty() { a.clone() } else { n }, addr: a });
                    self.cfg.save();
                    self.srv_name.clear();
                    self.srv_addr.clear();
                    self.show_add_server = false;
                }
            }
        });
        if !open {
            self.show_add_server = false;
        }
    }

    fn dialog_help(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let mut open = true;
        egui::Window::new("Помощь").open(&mut open).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).fixed_size([520.0, 300.0]).show(ctx, |ui| {
            ui.label("• «Аккаунты» → добавьте офлайн-ник или войдите через Microsoft.");
            ui.label("• «Версии» → «Создать версию»: имя, загрузчик (Vanilla/Forge/Fabric/Quilt/NeoForge) и версия игры от старой до новой.");
            ui.label("• «Войти в игру» скачает всё нужное, включая Java, и запустит Minecraft.");
            ui.label("• «RL MODS» — каталог Modrinth. Выберите версию слева внизу, откройте каталог и нажмите «Установить»: файл подберётся под версию игры и загрузчик.");
            ui.label("• «Обновить клиент» — перепроверить и докачать файлы выбранной версии.");
            ui.label("• Если игра закрылась с ошибкой — лог: Настройки → Настройки RLauncher → «Открыть лог».");
        });
        self.show_help = open;
    }
}

fn auth_ok(n: &str) -> bool {
    crate::auth::valid_nick(n)
}
