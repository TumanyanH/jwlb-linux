use crate::app::{App, Page};
use crate::catalog::{type_name, PubInfo, TYPE_ORDER};
use crate::widgets;
use egui::{RichText, Ui};

impl App {
    pub fn ui_pub_categories(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        widgets::library_pivot(self, ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let downloaded = self.downloaded_pubs();
            if !downloaded.is_empty() {
                widgets::section(ui, dark, "Downloaded");
                if let Some(i) = widgets::tile_row(self, ui, "downloaded", &downloaded, 104.0) {
                    let p = downloaded[i].clone();
                    self.open_pub(&p);
                }
            }
            widgets::section(ui, dark, "Categories");
            if !self.catalog.ready() {
                self.catalog_notice(ui);
                return;
            }
            let counts = self.catalog.type_counts();
            let mut types: Vec<(i64, i64)> = TYPE_ORDER
                .iter()
                .filter_map(|t| counts.iter().find(|(id, _)| id == t).copied())
                .collect();
            for c in &counts {
                if !types.iter().any(|t| t.0 == c.0) && type_name(c.0) != "Other" {
                    types.push(*c);
                }
            }
            let avail = ui.available_width() - 8.0;
            let gap = 12.0;
            let tw = 230.0;
            let cols = (((avail + gap) / (tw + gap)).floor() as usize).max(1);
            let tw = (avail - gap * (cols as f32 - 1.0)) / cols as f32;
            for row in types.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (t, n) in row {
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(tw, 64.0), egui::Sense::click());
                        let bg = if resp.hovered() { if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xf9f9f9) } } else { crate::theme::card(dark) };
                        ui.painter().rect_filled(r, 8.0, bg);
                        ui.painter().rect_stroke(r, 8.0, egui::Stroke::new(1.0, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xe5e5e5) }), egui::StrokeKind::Inside);
                        let icon = match t {
                            1 => "📖",
                            2 => "📕",
                            13 => "🌍",
                            14 => "🗼",
                            30 => "🗓",
                            4 => "📘",
                            10 => "📄",
                            22 => "🌐",
                            _ => "📚",
                        };
                        ui.painter().text(r.left_center() + egui::vec2(16.0, 0.0), egui::Align2::LEFT_CENTER, icon, egui::FontId::proportional(24.0), crate::theme::accent(dark));
                        ui.painter().text(r.left_center() + egui::vec2(54.0, -8.0), egui::Align2::LEFT_CENTER, type_name(*t), crate::fonts::bold(15.0), crate::theme::text(dark));
                        ui.painter().text(r.left_center() + egui::vec2(54.0, 12.0), egui::Align2::LEFT_CENTER, format!("{n} publications"), egui::FontId::proportional(12.0), crate::theme::muted(dark));
                        if resp.clicked() {
                            self.push(Page::PubList(*t));
                        }
                    }
                });
                ui.add_space(gap);
            }
            if let Some(u) = self.catalog.updated() {
                ui.add_space(8.0);
                ui.label(RichText::new(format!("Publication list updated {u}")).size(12.0).color(crate::theme::muted(dark)));
            }
        });
    }

    pub fn ui_pub_list(&mut self, ui: &mut Ui, t: i64) {
        let dark = self.dark;
        widgets::header(self, ui, type_name(t), None);
        let id = egui::Id::new(("publist_filter", t));
        let mut filter: String = ui.ctx().data_mut(|d| d.get_temp(id).unwrap_or_default());
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Filter by title or year").desired_width(280.0));
        });
        ui.ctx().data_mut(|d| d.insert_temp(id, filter.clone()));
        let all = self.catalog.by_type(t);
        let f = filter.to_lowercase();
        let pubs: Vec<PubInfo> = all
            .into_iter()
            .filter(|p| f.is_empty() || p.display_title().to_lowercase().contains(&f) || p.year.to_string().contains(&f) || p.key_symbol.to_lowercase() == f)
            .collect();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if pubs.is_empty() {
                widgets::empty_state(ui, dark, "📚", "Nothing here", "No publications match.");
                return;
            }
            // Periodicals are grouped by year like the original
            let by_year = matches!(t, 13 | 14 | 30 | 7);
            if by_year {
                let mut years: Vec<i64> = pubs.iter().map(|p| p.year).collect();
                years.dedup();
                for y in years {
                    let group: Vec<PubInfo> = pubs.iter().filter(|p| p.year == y).cloned().collect();
                    widgets::section(ui, dark, &y.to_string());
                    if let Some(i) = widgets::tile_grid(self, ui, &group, 112.0) {
                        let p = group[i].clone();
                        self.open_pub(&p);
                    }
                }
            } else if let Some(i) = widgets::tile_grid(self, ui, &pubs, 112.0) {
                let p = pubs[i].clone();
                self.open_pub(&p);
            }
        });
    }

    pub fn ui_toc(&mut self, ui: &mut Ui, p: &PubInfo) {
        let dark = self.dark;
        let e = crate::epub::Epub::open(&p.file_path());
        let Ok(e) = e else {
            widgets::header(self, ui, &p.display_title(), None);
            widgets::empty_state(ui, dark, "⚠", "Unable to open", "The download seems damaged. Delete it and download again.");
            if ui.button("🗑 Delete download").clicked() {
                self.delete_pub(p);
                self.request_back();
            }
            return;
        };
        ui.horizontal(|ui| {
            let cover = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(84.0, 84.0));
            ui.allocate_exact_size(egui::vec2(84.0, 84.0), egui::Sense::hover());
            let url = p.image_url();
            widgets::picture(self, ui, cover, url.as_deref(), &p.key_symbol, &p.key_symbol);
            ui.vertical(|ui| {
                ui.label(RichText::new(p.display_title()).font(crate::fonts::bold(22.0)).color(crate::theme::text(dark)));
                if !p.issue_title.is_empty() && p.issue_title != p.title {
                    ui.label(RichText::new(&p.title).color(crate::theme::muted(dark)));
                }
                ui.label(RichText::new(format!("{}  ·  {}", p.key_symbol, if p.year > 0 { p.year.to_string() } else { String::new() })).size(12.5).color(crate::theme::muted(dark)));
                ui.horizontal(|ui| {
                    let fav = self.user.is_favorite(&p.key());
                    if ui.button(if fav { "★ Favorite" } else { "☆ Add to Favorites" }).clicked() {
                        self.user.toggle_favorite(p);
                    }
                    if ui.button("🔖 Bookmarks").clicked() {
                        let bms = self.user.bookmarks(&p.key());
                        if let Some(bm) = bms.into_iter().flatten().next() {
                            self.open_loc(&bm.loc, Some((bm.btype, bm.ident)));
                        } else {
                            self.toast("No bookmarks in this publication yet");
                        }
                    }
                    if ui.button("🗑 Delete").clicked() {
                        self.delete_pub(p);
                        self.request_back();
                    }
                    // recordings, where the publisher has them
                    if let Some(tracks) = self.audio_for_pub(p) {
                        if !tracks.is_empty() && widgets::accent_button(ui, &format!("▶ Play audio ({})", tracks.len())).clicked() {
                            let items: Vec<(String, String, String)> = tracks.iter().map(|(t, u)| (t.clone(), u.clone(), String::new())).collect();
                            self.play(items, true);
                        }
                    }
                });
            });
        });
        if let Some(tracks) = self.audio_for_pub(p) {
            if !tracks.is_empty() {
                egui::CollapsingHeader::new(RichText::new("🎧 Audio recordings").strong()).id_salt(("audio", p.key())).show(ui, |ui| {
                    for (i, (t, _)) in tracks.iter().enumerate() {
                        if ui.add(egui::Button::new(format!("▶  {t}")).frame(false)).clicked() {
                            let items: Vec<(String, String, String)> = tracks[i..].iter().map(|(a, u)| (a.clone(), u.clone(), String::new())).collect();
                            self.play(items, true);
                        }
                    }
                });
            }
        }
        ui.separator();
        let toc = e.toc.clone();
        let spine = e.spine.clone();
        drop(e);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(860.0);
            let entries: Vec<(String, String, usize)> = if toc.is_empty() {
                spine.iter().filter(|s| !s.contains("nav")).enumerate().map(|(i, s)| (format!("Part {}", i + 1), s.clone(), 0)).collect()
            } else {
                toc.iter().map(|t| (t.title.clone(), t.href.clone(), t.level)).collect()
            };
            for (title, href, level) in entries {
                if href.ends_with("cover.xhtml") {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.add_space(level as f32 * 22.0);
                    let font = if level == 0 { crate::fonts::bold(15.5) } else { egui::FontId::proportional(15.0) };
                    let r = ui.add(egui::Button::new(RichText::new(&title).font(font).color(crate::theme::text(dark))).frame(false).wrap());
                    if r.clicked() {
                        self.open_doc(p, &href);
                    }
                });
                ui.add(egui::Separator::default().spacing(2.0));
            }
        });
    }

    /// Contents of a publication kept as online-library pages.
    pub fn ui_wol_list(&mut self, ui: &mut Ui, url: &str, title: &str) {
        let dark = self.dark;
        widgets::header(self, ui, title, None);
        self.load_wol_list(url);
        let cards = match self.wol_lists.get(url) {
            Some(crate::app::Loadable::Ready(c)) => c.clone(),
            Some(crate::app::Loadable::Failed(e)) => {
                let e = e.clone();
                widgets::empty_state(ui, dark, "📡", "Unable to open", &e);
                return;
            }
            _ => {
                ui.spinner();
                return;
            }
        };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(860.0);
            for c in &cards {
                let full = crate::wol::full_url(&c.href);
                let r = widgets::click_card(ui, dark, 820.0, |ui| {
                    ui.label(RichText::new(&c.title).font(crate::fonts::bold(15.0)).color(crate::theme::text(dark)));
                    if !c.sub.is_empty() {
                        ui.label(RichText::new(&c.sub).size(12.5).color(crate::theme::muted(dark)));
                    }
                });
                if r.clicked() {
                    if c.is_doc {
                        self.open_online(&full, &c.title);
                    } else {
                        self.push(Page::WolList(full, c.title.clone()));
                    }
                }
                ui.add_space(4.0);
            }
        });
    }
}
