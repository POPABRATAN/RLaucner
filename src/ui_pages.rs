use crate::app::*;
use crate::config::*;
use crate::ui::*;
use eframe::egui::{self, Align, Color32, Frame, Layout, Margin, RichText, Stroke, Ui, Vec2};

impl App {
    pub fn draw_sidebar(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(6.0);
            ui.label(RichText::new("RLAUNCHER").size(26.0).strong().color(Color32::WHITE));
            ui.label(RichText::new("ЛАУНЧЕР ДЛЯ MINECRAFT").size(10.0).color(Color32::from_white_alpha(210)));
            ui.add_space(14.0);
            let sz = Vec2::new(ui.available_width(), 38.0);
            let fill = Color32::from_rgb(72, 160, 220);
            if flat_button(ui, "ПОМОЩЬ", fill, Color32::WHITE, sz).clicked() {
                self.show_help = true;
            }
            if flat_button(ui, "ПАПКА ВЕРСИИ", fill, Color32::WHITE, sz).clicked() {
                if let Some(p) = self.cfg.profile() {
                    let _ = std::fs::create_dir_all(&p.game_dir);
                    let _ = open::that(&p.game_dir);
                } else {
                    self.toast("Сначала создайте версию");
                }
            }
            if flat_button(ui, "СЕРВЕРА", fill, Color32::WHITE, sz).clicked() {
                self.show_add_server = true;
            }
            if flat_button(ui, "ВЕРСИИ", fill, Color32::WHITE, sz).clicked() {
                self.show_versions = true;
            }
            ui.add_space(8.0);
            if flat_button(ui, "RL MODS — КАТАЛОГ", ORANGE, Color32::WHITE, sz).clicked() {
                let ctx = ui.ctx().clone();
                self.open_catalog(&ctx);
            }
            if flat_button(ui, "НАСТРОЙКИ", fill, Color32::WHITE, sz).clicked() {
                self.screen = Screen::Settings;
            }
            ui.add_space(14.0);
            ui.label(RichText::new("RLauncher не связан с Mojang AB и Microsoft.\nMinecraft — торговая марка Mojang AB.").size(10.0).color(Color32::from_white_alpha(170)));
        });
    }

    pub fn draw_main(&mut self, ui: &mut Ui) {
        let w = ui.available_width() - 28.0;
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.set_width(w);
                // мои сервера
                Frame::none().fill(Color32::from_rgba_unmultiplied(236, 170, 44, 245)).inner_margin(12.0).show(ui, |ui| {
                    ui.set_width(w - 24.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Мои сервера").size(16.0).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if flat_button(ui, "Добавить сервер", ORANGE, Color32::WHITE, Vec2::new(0.0, 24.0)).clicked() {
                                self.show_add_server = true;
                            }
                        });
                    });
                    if self.cfg.servers.is_empty() {
                        ui.label("Серверов пока нет. Добавьте адрес — и кнопка «Играть» сразу зайдёт на него.");
                    }
                    let servers = self.cfg.servers.clone();
                    let mut del = None;
                    let mut play = None;
                    for (i, s) in servers.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&s.name).strong());
                            ui.label(RichText::new(&s.addr).color(Color32::from_gray(60)));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if flat_button(ui, "Удалить", Color32::from_rgb(190, 70, 60), Color32::WHITE, Vec2::new(0.0, 22.0)).clicked() {
                                    del = Some(i);
                                }
                                if flat_button(ui, "Играть", BTN_GREEN, Color32::WHITE, Vec2::new(0.0, 22.0)).clicked() {
                                    play = Some(s.addr.clone());
                                }
                            });
                        });
                    }
                    if let Some(i) = del {
                        self.cfg.servers.remove(i);
                        self.cfg.save();
                    }
                    if let Some(a) = play {
                        let ctx = ui.ctx().clone();
                        self.start_launch(&ctx, Some(a));
                    }
                });
                ui.add_space(10.0);
                // новости
                Frame::none().fill(Color32::from_rgba_unmultiplied(255, 255, 255, 238)).inner_margin(16.0).show(ui, |ui| {
                    ui.set_width(w - 32.0);
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        if let Some(m) = &self.manifest {
                            ui.label(RichText::new(format!("Версия Minecraft {} уже доступна!", m.latest.release)).size(20.0).strong().color(BLUE_DARK));
                            ui.label(format!("Последний снапшот: {}.  Всего версий в списке Mojang: {}.", m.latest.snapshot, m.versions.len()));
                        } else if let Some(e) = &self.manifest_err {
                            ui.colored_label(Color32::from_rgb(190, 40, 40), format!("Не удалось получить список версий: {e}"));
                        }
                        ui.add_space(8.0);
                        ui.label(RichText::new("Быстрый старт").size(16.0).strong().color(BLUE_DARK));
                        let acc = self.cfg.account().map(|a| a.name.clone());
                        let prof = self.cfg.profile().map(|p| p.name.clone());
                        ui.label(format!("1. Аккаунт: {}", acc.unwrap_or_else(|| "не выбран — нажмите «Аккаунты» внизу".into())));
                        ui.label(format!("2. Версия: {}", prof.unwrap_or_else(|| "не создана — нажмите «Версии» внизу → «Создать версию»".into())));
                        ui.label("3. Нажмите «Войти в игру». Нужная Java скачается автоматически.");
                        ui.add_space(8.0);
                        ui.label(RichText::new("RL Mods").size(16.0).strong().color(BLUE_DARK));
                        ui.label("Каталог модов, сборок, ресурс-паков и шейдеров с Modrinth. Устанавливается файл именно под версию игры и загрузчик выбранной версии, обязательные зависимости подтягиваются сами.");
                        ui.add_space(8.0);
                        ui.label(RichText::new("Офлайн-режим").size(16.0).strong().color(BLUE_DARK));
                        ui.label("С офлайн-аккаунтом можно играть в одиночной игре и на серверах с отключённой проверкой лицензии (online-mode=false). На лицензионные серверы пустит только вход через Microsoft.");
                    });
                });
            });
        });
    }

    pub fn draw_catalog(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        // ключ фильтров: при изменении — новый поиск
        let key = (self.cat.tab, self.cat.game_version.clone(), self.cat.sort, self.cat.use_loader, self.cfg.selected_profile);
        if self.cat.last_key.as_ref() != Some(&key) {
            self.cat.last_key = Some(key);
            self.start_search(ctx, false);
        }
        // верхняя синяя панель, строка 1
        Frame::none().fill(BLUE).inner_margin(Margin::symmetric(10.0, 8.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                if icon_button(ui, Icon::Back, Color32::WHITE, 36.0).on_hover_text("Назад").clicked() {
                    self.screen = Screen::Main;
                }
                // профиль-цель установки
                let ptxt = self.cfg.profile().map(|p| p.name.clone()).unwrap_or_else(|| "нет версии".into());
                ui.label(RichText::new("Установка в:").color(Color32::WHITE));
                egui::ComboBox::from_id_salt("cat_prof").width(170.0).selected_text(ptxt).show_ui(ui, |ui| {
                    let profs = self.cfg.profiles.clone();
                    for (i, p) in profs.iter().enumerate() {
                        if ui.selectable_label(self.cfg.selected_profile == Some(i), format!("{} · {} {}", p.name, p.loader.short(), p.mc_version)).clicked() {
                            self.cfg.selected_profile = Some(i);
                            self.cat.game_version = p.mc_version.clone();
                            self.cfg.save();
                        }
                    }
                });
                // фильтр по версии игры
                let vtxt = if self.cat.game_version.is_empty() { "Все версии".to_string() } else { self.cat.game_version.clone() };
                egui::ComboBox::from_id_salt("cat_ver").width(120.0).selected_text(format!("Версии: {vtxt}")).show_ui(ui, |ui| {
                    if ui.selectable_label(self.cat.game_version.is_empty(), "Все версии").clicked() {
                        self.cat.game_version.clear();
                    }
                    let vs: Vec<String> = self.manifest.as_ref().map(|m| m.versions.iter().filter(|v| v.kind == "release").map(|v| v.id.clone()).collect()).unwrap_or_default();
                    for v in vs {
                        if ui.selectable_label(self.cat.game_version == v, &v).clicked() {
                            self.cat.game_version = v;
                        }
                    }
                });
                if flat_button(ui, "Создать", BLUE_DARK, Color32::WHITE, Vec2::new(0.0, 28.0)).clicked() {
                    self.show_versions = true;
                }
                let r = ui.add(egui::TextEdit::singleline(&mut self.cat.query).hint_text("Поиск").desired_width(200.0));
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if icon_button(ui, Icon::Search, Color32::WHITE, 30.0).clicked() || enter {
                    self.start_search(ctx, false);
                }
                ui.label(RichText::new("Сортировать").color(Color32::WHITE));
                egui::ComboBox::from_id_salt("cat_sort").width(130.0).selected_text(self.cat.sort.label()).show_ui(ui, |ui| {
                    for s in Sort::ALL {
                        ui.selectable_value(&mut self.cat.sort, s, s.label());
                    }
                });
            });
        });
        // строка 2: вкладки
        Frame::none().fill(Color32::from_rgb(36, 140, 208)).inner_margin(Margin::symmetric(10.0, 4.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                if icon_button(ui, Icon::Gear, Color32::WHITE, 34.0).on_hover_text("Настройки лаунчера").clicked() {
                    self.screen = Screen::Settings;
                }
                for t in CatTab::ALL {
                    let sel = self.cat.tab == t;
                    let resp = ui.add(egui::Label::new(RichText::new(t.label()).size(15.0).color(Color32::WHITE).strong()).sense(egui::Sense::click()));
                    if sel {
                        let r = resp.rect;
                        ui.painter().line_segment([r.left_bottom() + Vec2::new(0.0, 4.0), r.right_bottom() + Vec2::new(0.0, 4.0)], Stroke::new(3.0, YELLOW));
                    }
                    if resp.clicked() {
                        self.cat.tab = t;
                    }
                    ui.add_space(18.0);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if flat_button(ui, "Фильтры", BLUE, Color32::WHITE, Vec2::new(120.0, 28.0)).clicked() {
                        self.cat.filters_open = !self.cat.filters_open;
                    }
                });
            });
            if self.cat.filters_open {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.cat.use_loader, RichText::new("Только для загрузчика выбранной версии (Fabric/Forge/…)").color(Color32::WHITE));
                });
            }
        });
        // список
        let mut to_install: Option<modrinth_hit::Hit> = None;
        let mut want_more = false;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            Frame::none().fill(Color32::from_rgba_unmultiplied(255, 255, 255, 245)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some(e) = &self.cat.error {
                    ui.colored_label(Color32::from_rgb(190, 40, 40), format!("Ошибка Modrinth: {e}"));
                }
                if self.cat.hits.is_empty() && !self.cat.loading && self.cat.error.is_none() {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| ui.label("Ничего не найдено. Попробуйте «Все версии» или отключите фильтр по загрузчику."));
                }
                for h in &self.cat.hits {
                    ui.add_space(4.0);
                    ui.horizontal_top(|ui| {
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            ui.set_width(104.0);
                            match &h.icon_url {
                                Some(u) if !u.is_empty() => {
                                    ui.add(egui::Image::new(u.as_str()).fit_to_exact_size(Vec2::splat(100.0)).rounding(4.0));
                                }
                                _ => {
                                    let (r, _) = ui.allocate_exact_size(Vec2::splat(100.0), egui::Sense::hover());
                                    ui.painter().rect_filled(r, 4.0, Color32::from_gray(220));
                                }
                            }
                            let installing = self.cat.installing.contains(&h.project_id);
                            let done = self.cat.done.contains(&h.project_id);
                            let (txt, fill) = if installing { ("Установка…", Color32::from_gray(150)) } else if done { ("Установлено", BLUE) } else { ("Установить", BTN_GREEN) };
                            if ui.add_enabled(!installing, egui::Button::new(RichText::new(txt).color(Color32::WHITE).size(13.0)).fill(fill).stroke(Stroke::NONE).rounding(2.0).min_size(Vec2::new(100.0, 24.0))).clicked() {
                                to_install = Some(h.clone());
                            }
                        });
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width() - 16.0);
                            ui.label(RichText::new(&h.title).size(18.0).strong());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Автор:").color(Color32::GRAY));
                                ui.label(RichText::new(&h.author).color(BLUE_DARK));
                            });
                            let mut d = h.description.clone();
                            if d.chars().count() > 260 {
                                d = d.chars().take(260).collect::<String>() + "…";
                            }
                            ui.label(d);
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Загрузки:");
                                ui.label(RichText::new(fmt_downloads(h.downloads)).color(BLUE_DARK));
                                ui.add_space(10.0);
                                ui.label("Обновлено:");
                                ui.label(RichText::new(fmt_date(&h.date_modified)).color(BLUE_DARK));
                                ui.add_space(10.0);
                                ui.label("Последняя версия игры:");
                                ui.label(RichText::new(h.versions.last().cloned().unwrap_or_else(|| "—".into())).color(BLUE_DARK));
                            });
                        });
                    });
                    ui.add_space(4.0);
                    ui.separator();
                }
                if self.cat.loading {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.spinner();
                        ui.add_space(8.0);
                    });
                } else if (self.cat.hits.len() as u64) < self.cat.total {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        let r = flat_button(ui, &format!("Показать ещё ({} из {})", self.cat.hits.len(), self.cat.total), BLUE, Color32::WHITE, Vec2::new(240.0, 30.0));
                        if r.clicked() || ui.is_rect_visible(r.rect) {
                            want_more = true;
                        }
                        ui.add_space(10.0);
                    });
                }
            });
        });
        if let Some(h) = to_install {
            self.install_hit(ctx, &h);
        }
        if want_more {
            self.start_search(ctx, true);
        }
    }

    pub fn draw_settings(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(26.0);
            ui.set_width(640.0);
            Frame::none().fill(BLUE).inner_margin(Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
                ui.set_width(640.0);
                ui.horizontal(|ui| {
                    if icon_button(ui, Icon::Back, Color32::WHITE, 30.0).clicked() {
                        self.screen = Screen::Main;
                    }
                    ui.add_space(120.0);
                    ui.label(RichText::new("Настройка/Дополнительно").size(17.0).strong().color(Color32::WHITE));
                });
                ui.horizontal(|ui| {
                    for (i, t) in ["Запуск Minecraft", "Настройки RLauncher", "Конфиденциальность"].iter().enumerate() {
                        let sel = self.settings_tab == i;
                        let fill = if sel { Color32::WHITE } else { Color32::from_rgb(36, 140, 208) };
                        let col = if sel { BLUE } else { Color32::WHITE };
                        if flat_button(ui, t, fill, col, Vec2::new(205.0, 28.0)).clicked() {
                            self.settings_tab = i;
                        }
                    }
                });
            });
            Frame::none().fill(Color32::from_rgba_unmultiplied(255, 255, 255, 245)).inner_margin(16.0).show(ui, |ui| {
                ui.set_width(640.0 - 32.0);
                match self.settings_tab {
                    0 => self.settings_launch(ui),
                    1 => self.settings_launcher(ui),
                    _ => self.settings_privacy(ui),
                }
            });
        });
    }

    fn settings_launch(&mut self, ui: &mut Ui) {
        egui::Grid::new("sg").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
            ui.label(RichText::new("Директория:").strong());
            ui.horizontal(|ui| {
                let mut s = self.cfg.root_dir.to_string_lossy().to_string();
                if ui.add(egui::TextEdit::singleline(&mut s).desired_width(330.0)).changed() {
                    self.cfg.root_dir = s.into();
                }
                #[cfg(windows)]
                if ui.button("Обзор…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        self.cfg.root_dir = p;
                    }
                }
            });
            ui.end_row();
            ui.label(RichText::new("Разрешение:").strong());
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.cfg.width).range(320..=7680));
                ui.label("X");
                ui.add(egui::DragValue::new(&mut self.cfg.height).range(240..=4320));
                ui.checkbox(&mut self.cfg.fullscreen, "Полноэкранный режим");
            });
            ui.end_row();
            ui.label(RichText::new("Список версий:").strong());
            ui.vertical(|ui| {
                ui.checkbox(&mut self.cfg.show_snapshots, "Отображать снапшоты (\"Snapshots\")");
                ui.checkbox(&mut self.cfg.show_modded, "Отображать модифицированные");
                ui.checkbox(&mut self.cfg.show_old, "Отображать старые релизы (до 1.5.2)");
                ui.checkbox(&mut self.cfg.show_beta, "Отображать Бета (2010-2011 гг.)");
                ui.checkbox(&mut self.cfg.show_alpha, "Отображать Альфа (2010 г.)");
            });
            ui.end_row();
            ui.label(RichText::new("Язык:").strong());
            ui.checkbox(&mut self.cfg.ru_lang, "Использовать язык RLauncher в Minecraft");
            ui.end_row();
            ui.label(RichText::new("Minecraft аргументы:").strong());
            ui.add(egui::TextEdit::singleline(&mut self.cfg.game_args).hint_text("Minecraft-аргументы (напр. --server 127.0.0.1)").desired_width(400.0));
            ui.end_row();
            ui.label(RichText::new("Выбор Java:").strong());
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.cfg.java_path).hint_text("По умолчанию (Java скачивается автоматически)").desired_width(320.0));
                #[cfg(windows)]
                if ui.button("Изменить").clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("java", &["exe"]).pick_file() {
                        self.cfg.java_path = p.to_string_lossy().to_string();
                    }
                }
            });
            ui.end_row();
            ui.label(RichText::new("Выделение памяти:").strong());
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut self.cfg.memory_mb, 512..=16384).step_by(256.0).show_value(false));
                ui.label(format!("{} МБ", self.cfg.memory_mb));
            });
            ui.end_row();
        });
        ui.label(RichText::new("Изменяйте только если знаете, что делаете").size(12.0).color(Color32::GRAY));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if flat_button(ui, "По умолчанию", BTN_GREEN, Color32::WHITE, Vec2::new(170.0, 30.0)).clicked() {
                let d = Config::default();
                self.cfg.width = d.width;
                self.cfg.height = d.height;
                self.cfg.fullscreen = false;
                self.cfg.memory_mb = d.memory_mb;
                self.cfg.game_args.clear();
                self.cfg.java_path.clear();
                self.cfg.root_dir = d.root_dir;
                self.cfg.show_snapshots = d.show_snapshots;
                self.cfg.show_modded = d.show_modded;
                self.cfg.show_old = d.show_old;
                self.cfg.show_beta = d.show_beta;
                self.cfg.show_alpha = d.show_alpha;
            }
            if flat_button(ui, "Сохранить", ORANGE, Color32::WHITE, Vec2::new(170.0, 30.0)).clicked() {
                self.cfg.save();
                self.toast("Настройки сохранены");
            }
        });
    }

    fn settings_launcher(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Вход через Microsoft").strong());
        ui.label("Для входа нужен Azure Client ID вашего приложения (тип «Personal Microsoft accounts», разрешён публичный клиентский поток). Mojang также должна одобрить его для Minecraft API (форма на aka.ms/AppRegInfo). Без этого работает офлайн-режим.");
        ui.add(egui::TextEdit::singleline(&mut self.cfg.ms_client_id).hint_text("Client ID (UUID)").desired_width(420.0));
        ui.add_space(10.0);
        ui.checkbox(&mut self.cfg.hide_on_launch, "Сворачивать лаунчер при запуске игры");
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Открыть папку лаунчера").clicked() {
                let d = launcher_dir();
                let _ = std::fs::create_dir_all(&d);
                let _ = open::that(d);
            }
            if ui.button("Открыть лог последнего запуска").clicked() {
                let p = crate::launch::log_path(&self.cfg.root_dir);
                if p.exists() {
                    let _ = open::that(p);
                } else {
                    self.toast("Лога пока нет — игра ещё не запускалась");
                }
            }
        });
        ui.add_space(10.0);
        if flat_button(ui, "Сохранить", ORANGE, Color32::WHITE, Vec2::new(170.0, 30.0)).clicked() {
            self.cfg.save();
            self.toast("Настройки сохранены");
        }
        ui.add_space(6.0);
        ui.label(RichText::new(format!("RLauncher {APP_VERSION}")).color(Color32::GRAY));
    }

    fn settings_privacy(&mut self, ui: &mut Ui) {
        ui.label("RLauncher не собирает статистику и ничего не отправляет на свои сервера. Сетевые запросы идут только к Mojang/Microsoft (версии, файлы игры, вход), Modrinth (каталог), Fabric/Quilt/Forge/NeoForge (загрузчики).");
        ui.add_space(8.0);
        ui.label(format!("Аккаунты и токены хранятся локально в файле:\n{}", Config::path().display()));
        ui.add_space(8.0);
        if flat_button(ui, "Выйти из всех аккаунтов", Color32::from_rgb(190, 70, 60), Color32::WHITE, Vec2::new(230.0, 30.0)).clicked() {
            self.cfg.accounts.clear();
            self.cfg.selected_account = 0;
            self.cfg.save();
            self.toast("Все аккаунты удалены из лаунчера");
        }
    }
}

mod modrinth_hit {
    pub use crate::modrinth::Hit;
}
