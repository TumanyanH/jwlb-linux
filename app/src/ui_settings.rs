use crate::app::App;
use crate::settings::ThemeChoice;
use crate::widgets;
use egui::{RichText, Ui};

impl App {
    pub fn ui_settings(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        widgets::header(self, ui, "Settings", None);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(760.0_f32.min(ui.available_width() - 16.0));
            // Language
            widgets::card(ui, dark, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Language").font(crate::fonts::bold(16.0)));
                let cur = crate::lang::by_symbol(&self.settings.lang);
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Publications in: {}",
                        cur.map(|l| format!("{} ({})", l.vernacular, l.english)).unwrap_or_else(|| self.settings.lang.clone())
                    ));
                    if ui.button("Change…").clicked() {
                        self.show_lang_picker = !self.show_lang_picker;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Translate selected text into:");
                    let cur = crate::translate::name(&self.settings.translate_to);
                    egui::ComboBox::from_id_salt("translate_to").selected_text(cur).width(220.0).height(360.0).show_ui(ui, |ui| {
                        for (code, name) in crate::translate::LANGS {
                            if ui.selectable_label(self.settings.translate_to == code, name).clicked() {
                                self.settings.translate_to = code.to_string();
                                self.settings.save();
                            }
                        }
                    });
                });
                ui.label(
                    RichText::new("Translation uses Google Translate online (MyMemory as a backup); the selected text is sent to that service.")
                        .size(12.0)
                        .color(crate::theme::muted(dark)),
                );
                if self.show_lang_picker {
                    ui.add(egui::TextEdit::singleline(&mut self.lang_filter).hint_text("Type a language name").desired_width(300.0));
                    let f = self.lang_filter.to_lowercase();
                    let mut pick = None;
                    egui::ScrollArea::vertical().id_salt("langs").max_height(260.0).show(ui, |ui| {
                        for l in crate::lang::all().iter().filter(|l| {
                            f.is_empty() || l.english.to_lowercase().contains(&f) || l.vernacular.to_lowercase().contains(&f) || l.symbol.to_lowercase() == f
                        }) {
                            let label = format!("{}  —  {}{}", l.vernacular, l.english, if l.sign { "  (sign language)" } else { "" });
                            if ui.selectable_label(l.symbol == self.settings.lang, label).clicked() {
                                pick = Some(l.symbol.clone());
                            }
                        }
                    });
                    if let Some(s) = pick {
                        self.show_lang_picker = false;
                        self.change_language(&s);
                    }
                }
            });
            ui.add_space(10.0);
            // Appearance
            widgets::card(ui, dark, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Appearance").font(crate::fonts::bold(16.0)));
                let before = self.settings.theme;
                ui.horizontal(|ui| {
                    ui.label("Dark mode");
                    ui.add_space(12.0);
                    let mut on = self.dark;
                    if widgets::toggle(ui, &mut on).clicked() {
                        self.settings.theme = if on { ThemeChoice::Dark } else { ThemeChoice::Light };
                    }
                    ui.label(RichText::new(if on { "On" } else { "Off" }).color(crate::theme::muted(dark)));
                });
                let mut follow = self.settings.theme == ThemeChoice::System;
                if ui.checkbox(&mut follow, "Match the computer's light/dark setting").changed() {
                    self.settings.theme = if follow { ThemeChoice::System } else if self.dark { ThemeChoice::Dark } else { ThemeChoice::Light };
                }
                if before != self.settings.theme {
                    self.settings.save();
                    self.update_theme();
                }
                ui.horizontal(|ui| {
                    ui.label("Text size:");
                    if ui
                        .add(
                            egui::Slider::new(&mut self.settings.font_percent, crate::settings::FONT_PERCENT_MIN..=crate::settings::FONT_PERCENT_MAX)
                                .step_by(crate::settings::FONT_PERCENT_STEP as f64)
                                .suffix("%"),
                        )
                        .changed()
                    {
                        self.settings.save();
                    }
                });
                ui.label(RichText::new("In the beginning God created the heavens and the earth.").size(self.settings.text_size()));
                ui.horizontal(|ui| {
                    ui.label("Scrolling speed:");
                    if ui.add(egui::Slider::new(&mut self.settings.scroll_speed, 1.0..=6.0).step_by(0.5).suffix("×")).changed() {
                        self.settings.save();
                    }
                });
                if ui.checkbox(&mut self.settings.show_footnotes, "Show footnotes under the text").changed() {
                    self.settings.save();
                }
            });
            ui.add_space(10.0);
            // Media
            widgets::card(ui, dark, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Media").font(crate::fonts::bold(16.0)));
                ui.horizontal(|ui| {
                    ui.label("Video quality:");
                    for q in ["240p", "360p", "480p", "720p"] {
                        if ui.selectable_value(&mut self.settings.video_quality, q.to_string(), q).clicked() {
                            self.settings.save();
                        }
                    }
                });
                ui.label(RichText::new("Videos and audio play inside the app.").size(12.5).color(crate::theme::muted(dark)));
            });
            ui.add_space(10.0);
            // Library / storage
            widgets::card(ui, dark, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Library").font(crate::fonts::bold(16.0)));
                ui.horizontal(|ui| {
                    if let Some(u) = self.catalog.updated() {
                        ui.label(format!("Publication list updated {u}"));
                    } else {
                        ui.label("Publication list not downloaded");
                    }
                    if self.catalog_job.is_some() {
                        ui.spinner();
                    } else if ui.button("⟳ Check for updates").clicked() {
                        self.start_catalog_update();
                    }
                });
                let pubs = self.downloaded_pubs();
                let total: u64 = pubs.iter().filter_map(|p| std::fs::metadata(p.file_path()).ok()).map(|m| m.len()).sum();
                ui.label(format!("{} downloaded publications ({})", pubs.len(), widgets::fmt_size(total)));
                ui.horizontal(|ui| {
                    if ui.button("📂 Open data folder").clicked() {
                        crate::paths::open_external(&crate::paths::data_dir().to_string_lossy());
                    }
                    if ui.button("🧹 Clear picture cache").clicked() {
                        let _ = std::fs::remove_dir_all(crate::paths::cache_dir().join("img"));
                        self.toast("Picture cache cleared");
                    }
                });
            });
            ui.add_space(10.0);
            // About
            widgets::card(ui, dark, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("About").font(crate::fonts::bold(16.0)));
                ui.label(format!("Study Library {} for Linux", env!("CARGO_PKG_VERSION")));
                ui.label(RichText::new("Publications, the daily text, meeting material and media are downloaded from the publisher's official free services. This app is not made by the publisher.").size(12.5).color(crate::theme::muted(dark)));
                ui.horizontal(|ui| {
                    if ui.link("Terms of use").clicked() {
                        crate::paths::open_external("https://www.jw.org/app-terms-of-use/");
                    }
                    if ui.link("Privacy").clicked() {
                        crate::paths::open_external("https://www.jw.org/en/privacy-policy/global-policy-personal-data/");
                    }
                });
                ui.label(RichText::new("Built with Rust, egui, SQLite and other open-source software.").size(12.0).color(crate::theme::muted(dark)));
            });
            ui.add_space(30.0);
        });
    }
}
