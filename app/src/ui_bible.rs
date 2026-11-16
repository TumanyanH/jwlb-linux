//! Bible tab: the chosen translation's books and chapters, and a manager to add Bibles in any
//! language, switch between them and download them for offline reading.

use crate::app::{App, Loadable, Page};
use crate::widgets;
use crate::wol::{BibleRef, BookIdx};
use egui::{RichText, Ui};

impl App {
    pub fn current_bible(&self) -> Option<BibleRef> {
        self.settings.bibles.get(self.settings.bible_current).or(self.settings.bibles.first()).cloned()
    }

    pub fn load_books(&mut self, b: &BibleRef) {
        let k = b.key();
        if self.bible_books.contains_key(&k) {
            return;
        }
        self.bible_books.insert(k.clone(), Loadable::Loading);
        let b = b.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = crate::wol::books(&b);
            let _ = tx.send(crate::app::Msg::Books(k, r));
            ctx.request_repaint();
        });
    }

    pub fn load_bible_list(&mut self, lang: &str) {
        if self.bible_lists.contains_key(lang) {
            return;
        }
        self.bible_lists.insert(lang.to_string(), Loadable::Loading);
        let l = lang.to_string();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = crate::wol::bibles(&l);
            let _ = tx.send(crate::app::Msg::Bibles(l, r));
            ctx.request_repaint();
        });
    }

    /// The current Bible and its books, once known.
    pub fn bible_index(&mut self) -> Option<(BibleRef, Vec<BookIdx>)> {
        let b = self.current_bible()?;
        self.load_books(&b);
        match self.bible_books.get(&b.key()) {
            Some(Loadable::Ready(v)) => Some((b, v.clone())),
            _ => None,
        }
    }

    pub fn open_bible(&mut self, book: u8, chapter: u16, verse: Option<u16>) {
        let Some((b, books)) = self.bible_index() else {
            self.tab = crate::app::Tab::Bible;
            self.toast("Choose a Bible first");
            return;
        };
        let w = match crate::net::wol_locale(&b.lang) {
            Ok(w) => w,
            Err(e) => {
                self.toast(e);
                return;
            }
        };
        let url = crate::wol::chapter_url(&w, &b, book, chapter);
        let id = self.load_online(url);
        let rd = crate::reader::Reader::bible(b, books, book, chapter, verse, id);
        self.push_on(crate::app::Tab::Bible, Page::Reader(Box::new(rd)));
    }

    pub fn add_bible(&mut self, b: BibleRef) {
        if let Some(i) = self.settings.bibles.iter().position(|x| x.key() == b.key()) {
            self.settings.bible_current = i;
        } else {
            self.settings.bibles.push(b);
            self.settings.bible_current = self.settings.bibles.len() - 1;
        }
        self.settings.save();
    }

    pub fn bible_downloaded(b: &BibleRef) -> bool {
        crate::paths::sub("bibles").join(format!("{}.done", crate::paths::safe_name(&b.key()))).exists()
    }

    pub fn download_bible(&mut self, b: &BibleRef) {
        let key = format!("bible:{}", b.key());
        if self.downloads.contains_key(&key) {
            return;
        }
        let prog = crate::app::Progress::new();
        let (d, t) = (prog.done.clone(), prog.total.clone());
        let b = b.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let k = key.clone();
        std::thread::spawn(move || {
            let r = crate::wol::download_bible(&b, d, t).map(|_| {
                let _ = std::fs::write(crate::paths::sub("bibles").join(format!("{}.done", crate::paths::safe_name(&b.key()))), b.title.as_bytes());
                "bible".to_string()
            });
            let _ = tx.send(crate::app::Msg::Downloaded(k, r));
            ctx.request_repaint();
        });
        self.downloads.insert(key, prog);
    }

    fn bible_header(&mut self, ui: &mut Ui, b: &BibleRef) {
        let dark = self.dark;
        // the Bible's name is on the header button at the top right
        self.page_title = "Bible".into();
        let _ = &b.title;
        // the original's tab strip under the header (BOOKS selected)
        let (strip, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::hover());
        ui.painter().rect_filled(strip, 0.0, crate::theme::content_bg(dark));
        let g = ui.painter().layout_no_wrap("BOOKS".into(), crate::fonts::bold(13.5), crate::theme::accent(dark));
        let tpos = strip.left_center() + egui::vec2(18.0, -g.size().y / 2.0);
        let gw = g.size().x;
        ui.painter().galley(tpos, g, crate::theme::accent(dark));
        ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(tpos.x, strip.bottom() - 3.0), egui::vec2(gw, 3.0)), 0.0, crate::theme::accent(dark));
        // only a download progress line remains on the page; choosing Bibles is the header button
        let key = format!("bible:{}", b.key());
        if let Some(p) = self.downloads.get(&key) {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.add(egui::ProgressBar::new(p.fraction()).desired_width(220.0).text(format!("Downloading {:.0}%", p.fraction() * 100.0)));
            });
        }
        ui.add_space(8.0);
    }

    /// Top-right header button: the current Bible and its language; opens a menu to switch
    /// Bibles, add one in any language, or download the current one.
    pub fn bible_menu_button(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let Some(b) = self.current_bible() else { return };
        let lang_name = crate::lang::by_symbol(&b.lang).map(|l| l.english.clone()).unwrap_or_default();
        let label = format!("{}  ·  {}", b.title, lang_name);
        let font = egui::FontId::proportional(13.5);
        let g = ui.painter().layout_no_wrap(label, font, crate::theme::text(dark));
        let size = egui::vec2(g.size().x + 26.0 + 22.0, 32.0);
        let (r, resp) = ui.allocate_exact_size(size, egui::Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xe4e4e4) });
        }
        crate::icons::paint(ui.painter(), crate::icons::rect_at(egui::pos2(r.left() + 13.0, r.center().y), 16.0), crate::icons::Icon::Bible, crate::theme::icon(dark), None);
        ui.painter().galley(egui::pos2(r.left() + 26.0, r.center().y - g.size().y / 2.0), g, crate::theme::text(dark));
        let c = egui::pos2(r.right() - 11.0, r.center().y);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![c + egui::vec2(-4.0, -2.0), c + egui::vec2(4.0, -2.0), c + egui::vec2(0.0, 3.0)],
            crate::theme::icon(dark),
            egui::Stroke::NONE,
        ));
        let resp = resp.on_hover_text("Choose Bible and language");
        let mut add = false;
        let mut download = false;
        crate::widgets::popup(ui, &resp, "bible_menu", |ui| {
            ui.set_min_width(300.0);
            let list = self.settings.bibles.clone();
            for (i, x) in list.iter().enumerate() {
                let ln = crate::lang::by_symbol(&x.lang).map(|l| l.english.clone()).unwrap_or_default();
                if ui.selectable_label(i == self.settings.bible_current, format!("{}  ·  {}", x.title, ln)).clicked() {
                    self.settings.bible_current = i;
                    self.settings.save();
                }
            }
            ui.separator();
            if ui.button("➕ Add Bible…").clicked() {
                add = true;
            }
            let key = format!("bible:{}", b.key());
            if self.downloads.contains_key(&key) {
                ui.label(RichText::new("Downloading…").color(crate::theme::muted(dark)));
            } else if App::bible_downloaded(&b) {
                ui.label(RichText::new("✔ This Bible is on this computer").color(crate::theme::muted(dark)));
            } else if ui.button("⬇ Download for offline").clicked() {
                download = true;
            }
        });
        if add {
            self.tab = crate::app::Tab::Bible;
            self.push(Page::BibleSelect);
        }
        if download {
            self.download_bible(&b);
        }
    }

    pub fn ui_bible_books(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        if self.settings.bibles.is_empty() {
            self.ui_bible_select(ui, false);
            return;
        }
        let b = self.current_bible().unwrap();
        self.bible_header(ui, &b);
        self.load_books(&b);
        let books = match self.bible_books.get(&b.key()) {
            Some(Loadable::Ready(v)) => v.clone(),
            Some(Loadable::Failed(e)) => {
                let e = e.clone();
                widgets::empty_state(ui, dark, "📡", "Unable to open this Bible", &format!("Connect to the internet once to load it. ({e})"));
                if ui.button("Try again").clicked() {
                    self.bible_books.remove(&b.key());
                }
                return;
            }
            _ => {
                ui.spinner();
                return;
            }
        };
        // Quick reference box
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let id = egui::Id::new("bible_ref");
            let mut s: String = ui.ctx().data_mut(|d| d.get_temp::<String>(id).unwrap_or_default());
            let dark = self.dark;
            // a 38 px field with rounded corners and a matching purple button
            let field = egui::Frame::default()
                .fill(crate::theme::card(dark))
                .stroke(egui::Stroke::new(1.0_f32, if dark { crate::theme::hex(0x4f4f4f) } else { crate::theme::hex(0xd0d0d0) }))
                .corner_radius(8.0)
                .inner_margin(egui::Margin { left: 10, right: 10, top: 0, bottom: 0 })
                .show(ui, |ui| {
                    ui.set_height(38.0);
                    ui.horizontal_centered(|ui| {
                        crate::icons::paint(ui.painter(), crate::icons::rect_at(ui.cursor().left_center() + egui::vec2(8.0, 0.0), 16.0), crate::icons::Icon::Search, crate::theme::icon(dark), None);
                        ui.add_space(24.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut s)
                                .hint_text("Go to… e.g. John 3:16")
                                .desired_width(250.0)
                                .frame(false)
                                .font(egui::FontId::proportional(15.0)),
                        )
                    })
                    .inner
                });
            let r = field.inner;
            ui.add_space(6.0);
            let go_btn = ui.add(
                egui::Button::new(RichText::new("Go").font(crate::fonts::bold(15.0)).color(if dark { egui::Color32::BLACK } else { egui::Color32::WHITE }))
                    .fill(crate::theme::accent(dark))
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(8.0)
                    .min_size(egui::vec2(64.0, 40.0)),
            );
            let go = (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || go_btn.clicked();
            if go {
                let mut all: Vec<(u8, String, String)> = books.iter().map(|b| (b.num, b.name.clone(), b.abbr.clone())).collect();
                all.extend(crate::bible::BOOKS.iter().enumerate().map(|(i, b)| (i as u8 + 1, b.name.to_string(), b.abbr.to_string())));
                if let Some((bk, c, v)) = crate::bible::parse_reference(&s, &all) {
                    self.open_bible(bk, c, v);
                } else {
                    self.toast("Reference not recognised");
                }
            }
            ui.ctx().data_mut(|d| d.insert_temp(id, s));
        });
        ui.add_space(10.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // Two blocks side by side (4 and 3 columns) as in the original; stacked when narrow.
            let avail = ui.available_width() - 32.0;
            let gap = 2.0;
            let side_by_side = avail >= 900.0;
            let heb: Vec<BookIdx> = books.iter().filter(|b| b.num <= 39).cloned().collect();
            let grk: Vec<BookIdx> = books.iter().filter(|b| b.num >= 40).cloned().collect();
            let (w_heb, w_grk) = if side_by_side { (avail * 4.0 / 7.0 - 12.0, avail * 3.0 / 7.0 - 12.0) } else { (avail, avail) };
            let mut open: Option<u8> = None;
            // the audio mark only on books that have recordings
            let audio_books: std::collections::HashSet<u8> = self.audio_for(&b).map(|m| m.keys().map(|(bk, _)| *bk).collect()).unwrap_or_default();
            let mut block = |ui: &mut Ui, title: &str, list: &[BookIdx], cols: usize, width: f32| {
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.label(RichText::new(title.to_uppercase()).font(crate::fonts::bold(19.0)).color(crate::theme::text(dark)));
                    ui.add_space(4.0);
                    let tw = (width - gap * (cols as f32 - 1.0)) / cols as f32;
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for row in list.chunks(cols) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for bk in row {
                                let group = crate::bible::BOOKS.get(bk.num as usize - 1).map(|x| x.group).unwrap_or(0);
                                let (r, resp) = ui.allocate_exact_size(egui::vec2(tw, 48.0), egui::Sense::click());
                                let mut c = crate::theme::book_group_color(group, dark);
                                if resp.hovered() {
                                    c = c.gamma_multiply(1.18);
                                }
                                ui.painter().rect_filled(r, 0.0, c);
                                // full name when it fits, otherwise the abbreviation
                                let font = egui::FontId::proportional(13.0);
                                let name = if ui.fonts(|f| f.layout_no_wrap(bk.name.clone(), font.clone(), egui::Color32::WHITE).size().x) < tw - 26.0 { bk.name.clone() } else { bk.abbr.clone() };
                                ui.painter().text(r.left_center() + egui::vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, name, font, egui::Color32::WHITE);
                                if audio_books.contains(&bk.num) {
                                    crate::icons::paint(ui.painter(), crate::icons::rect_at(r.right_top() + egui::vec2(-10.0, 9.0), 11.0), crate::icons::Icon::Audio, egui::Color32::from_white_alpha(150), None);
                                }
                                if resp.on_hover_text(&bk.name).clicked() {
                                    open = Some(bk.num);
                                }
                            }
                        });
                        ui.add_space(gap);
                    }
                });
            };
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                if side_by_side {
                    ui.horizontal_top(|ui| {
                        block(ui, "Hebrew-Aramaic Scriptures", &heb, 4, w_heb);
                        ui.add_space(24.0);
                        block(ui, "Christian Greek Scriptures", &grk, 3, w_grk);
                    });
                } else {
                    ui.vertical(|ui| {
                        if !heb.is_empty() {
                            block(ui, "Hebrew-Aramaic Scriptures", &heb, 4, w_heb);
                            ui.add_space(16.0);
                        }
                        block(ui, "Christian Greek Scriptures", &grk, 3, w_grk);
                    });
                }
            });
            ui.add_space(20.0);
            if let Some(n) = open {
                if crate::wol::chapter_count(n) == 1 {
                    self.open_bible(n, 1, None);
                } else {
                    self.push(Page::BibleChapters(n));
                }
            }
        });
    }

    pub fn ui_bible_chapters(&mut self, ui: &mut Ui, book: u8) {
        let Some((b, books)) = self.bible_index() else {
            if self.settings.bibles.is_empty() {
                self.request_back();
            } else {
                ui.spinner();
            }
            return;
        };
        let name = books.iter().find(|x| x.num == book).map(|x| x.name.clone()).unwrap_or_default();
        widgets::header(self, ui, &name, Some(&b.title));
        let dark = self.dark;
        let n = crate::wol::chapter_count(book) as usize;
        let has_audio = self.audio_for(&b).map(|m| m.keys().any(|(bk, _)| *bk == book)).unwrap_or(false);
        if has_audio {
            ui.horizontal(|ui| {
                if crate::widgets::accent_button(ui, "▶ Listen").on_hover_text("Play this book's recordings").clicked() {
                    self.play_bible_audio(&b, book, 1);
                }
            });
            ui.add_space(8.0);
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // Square grey tiles with 2 px gaps, 5 or 10 per row (as in the original's chapter grid)
            let gap = 3.0;
            let avail = ui.available_width() - 4.0;
            let cols = if avail >= 10.0 * 64.0 + 9.0 * gap { 10 } else { 5 };
            let tw = ((avail - gap * (cols as f32 - 1.0)) / cols as f32).min(90.0);
            let tile = if dark { crate::theme::hex(0x4f4f4f) } else { crate::theme::hex(0x757575) };
            let chapters: Vec<usize> = (1..=n).collect();
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in chapters.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for ch in row {
                        let (r, resp) = ui.allocate_exact_size(egui::vec2(tw, tw), egui::Sense::click());
                        let col = if resp.hovered() { tile.gamma_multiply(1.2) } else { tile };
                        ui.painter().rect_filled(r, 0.0, col);
                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, ch.to_string(), egui::FontId::proportional((tw * 0.3).clamp(14.0, 24.0)), egui::Color32::WHITE);
                        if resp.clicked() {
                            self.open_bible(book, *ch as u16, None);
                        }
                    }
                });
                ui.add_space(gap);
            }
        });
    }
}

/// "Add Bible" window: choose a language, then a translation.
pub fn add_bible_window(app: &mut App, ctx: &egui::Context) {
    let Some(mut filter) = app.bible_add.clone() else { return };
    let dark = app.dark;
    let mut open = true;
    let mut close = false;
    egui::Window::new("Add Bible")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([560.0, 520.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            let lang = app.bible_add_lang.clone().unwrap_or_else(|| app.settings.lang.clone());
            ui.horizontal(|ui| {
                ui.label("Language:");
                ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Type to find a language").desired_width(240.0));
                let cur = crate::lang::by_symbol(&lang).map(|l| format!("{} ({})", l.vernacular, l.english)).unwrap_or_default();
                ui.label(RichText::new(cur).strong());
            });
            if !filter.is_empty() {
                let f = filter.to_lowercase();
                egui::ScrollArea::vertical().id_salt("addbible_langs").max_height(160.0).show(ui, |ui| {
                    for l in crate::lang::all().iter().filter(|l| !l.sign && (l.english.to_lowercase().contains(&f) || l.vernacular.to_lowercase().contains(&f))).take(60) {
                        if ui.selectable_label(l.symbol == lang, format!("{} — {}", l.vernacular, l.english)).clicked() {
                            app.bible_add_lang = Some(l.symbol.clone());
                            filter.clear();
                        }
                    }
                });
            }
            ui.separator();
            app.load_bible_list(&lang);
            match app.bible_lists.get(&lang) {
                Some(Loadable::Ready(list)) if !list.is_empty() => {
                    let list = list.clone();
                    egui::ScrollArea::vertical().id_salt("addbible_list").show(ui, |ui| {
                        for (sym, title) in list {
                            let r = BibleRef { lang: lang.clone(), symbol: sym.clone(), title: title.clone() };
                            let have = app.settings.bibles.iter().any(|x| x.key() == r.key());
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&title).font(crate::fonts::bold(14.5)));
                                    ui.label(RichText::new(&sym).size(12.0).color(crate::theme::muted(dark)));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if have {
                                        if ui.button("Use").clicked() {
                                            app.add_bible(r.clone());
                                            close = true;
                                        }
                                    } else if widgets::accent_button(ui, "➕ Add").clicked() {
                                        app.add_bible(r.clone());
                                        app.download_bible(&r);
                                        close = true;
                                    }
                                });
                            });
                            ui.separator();
                        }
                    });
                }
                Some(Loadable::Ready(_)) => {
                    ui.label("No Bible is available in this language.");
                }
                Some(Loadable::Failed(e)) => {
                    ui.label(format!("Could not load the list of Bibles: {e}"));
                    if ui.button("Try again").clicked() {
                        app.bible_lists.remove(&lang);
                    }
                }
                _ => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Looking for Bibles in this language…");
                    });
                }
            }
        });
    if !open || close {
        app.bible_add = None;
        if close {
            app.tab = crate::app::Tab::Bible;
        }
    } else {
        app.bible_add = Some(filter);
    }
}

impl App {
    /// "Select a Bible": every language's Bibles from the publication list, as in the original
    /// (person's screenshot): a centred white panel, language search, rows with language and
    /// name, a download icon with the size, and "Later".
    pub fn ui_bible_select(&mut self, ui: &mut Ui, can_close: bool) {
        let dark = self.dark;
        self.page_title = "Bible".into();
        let list = self.catalog.all_bibles();
        if list.is_none() && self.catalog.ready() && !self.bibles_refresh_asked && self.catalog_job.is_none() {
            // this copy of the publication list predates the Bible list: fetch it again once
            self.bibles_refresh_asked = true;
            self.start_catalog_update();
        }
        let avail = ui.available_size();
        let panel_w = 440.0_f32.min(avail.x - 24.0);
        let panel_h = (avail.y - 24.0).max(200.0);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(((avail.x - panel_w) / 2.0).max(0.0));
            egui::Frame::default()
                .fill(crate::theme::card(dark))
                .stroke(egui::Stroke::new(1.0, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xe0e0e0) }))
                .inner_margin(egui::Margin { left: 14, right: 14, top: 14, bottom: 12 })
                .show(ui, |ui| { ui.vertical(|ui| {
                    ui.set_width(panel_w - 28.0);
                    ui.set_height(panel_h - 26.0);
                    ui.label(RichText::new("Select a Bible").size(19.0).color(crate::theme::text(dark)));
                    ui.add_space(6.0);
                    let Some(mut rows) = list else {
                        if self.catalog_job.is_some() || !self.catalog.ready() {
                            self.catalog_notice(ui);
                        }
                        // until the list arrives: the online library's Bibles in this language
                        let lang = self.settings.lang.clone();
                        self.load_bible_list(&lang);
                        if let Some(Loadable::Ready(v)) = self.bible_lists.get(&lang) {
                            for (sym, title) in v.clone() {
                                if ui.button(&title).clicked() {
                                    let r = BibleRef { lang: lang.clone(), symbol: sym, title };
                                    self.add_bible(r.clone());
                                    self.download_bible(&r);
                                    if can_close {
                                        self.request_back();
                                    }
                                }
                            }
                        }
                        return;
                    };
                    let langs: std::collections::BTreeSet<i64> = rows.iter().map(|r| r.0).collect();
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.bible_filter)
                            .hint_text(format!("Search languages ({})", langs.len()))
                            .desired_width(f32::INFINITY),
                    );
                    crate::icons::paint(ui.painter(), crate::icons::rect_at(r.rect.right_center() - egui::vec2(12.0, 0.0), 14.0), crate::icons::Icon::Search, crate::theme::icon(dark), None);
                    ui.add_space(4.0);
                    // order: the app's language first, then by language name; Bibles by edition
                    let me = crate::lang::meps_id(&self.settings.lang);
                    let order = ["nwtsty", "nwt", "Rbi8", "int", "by", "bi22", "bi10", "rh"];
                    let lname = |id: i64| crate::lang::by_id(id).map(|l| l.english.clone()).unwrap_or_default();
                    rows.sort_by(|a, b| {
                        (a.0 != me)
                            .cmp(&(b.0 != me))
                            .then(lname(a.0).cmp(&lname(b.0)))
                            .then(order.iter().position(|s| *s == a.1).unwrap_or(99).cmp(&order.iter().position(|s| *s == b.1).unwrap_or(99)))
                            .then(b.4.cmp(&a.4))
                    });
                    let f = self.bible_filter.to_lowercase();
                    if !f.is_empty() {
                        rows.retain(|r| {
                            let l = crate::lang::by_id(r.0);
                            l.map(|l| l.english.to_lowercase().contains(&f) || l.vernacular.to_lowercase().contains(&f)).unwrap_or(false)
                                || r.2.to_lowercase().contains(&f)
                        });
                    }
                    let mut pick: Option<BibleRef> = None;
                    let list_h = ui.available_height() - 44.0;
                    egui::ScrollArea::vertical().id_salt("select_bible").max_height(list_h).auto_shrink([false, false]).show_rows(ui, 52.0, rows.len(), |ui, range| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for row in &rows[range] {
                            let Some(l) = crate::lang::by_id(row.0) else { continue };
                            let short = if row.2.is_empty() { row.3.clone() } else { row.2.clone() };
                            let bref = BibleRef { lang: l.symbol.clone(), symbol: row.1.clone(), title: short.clone() };
                            let have = self.settings.bibles.iter().any(|x| x.key() == bref.key());
                            let (rr, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::click());
                            if resp.hovered() {
                                ui.painter().rect_filled(rr, 0.0, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xd8d8d8) });
                            }
                            ui.painter().text(rr.left_top() + egui::vec2(4.0, 10.0), egui::Align2::LEFT_TOP, &l.english, egui::FontId::proportional(12.0), crate::theme::text(dark));
                            ui.painter().text(rr.left_top() + egui::vec2(4.0, 27.0), egui::Align2::LEFT_TOP, &short, egui::FontId::proportional(13.0), crate::theme::text(dark));
                            let ic = egui::pos2(rr.right() - 26.0, rr.top() + 18.0);
                            if have {
                                crate::icons::paint(ui.painter(), crate::icons::rect_at(ic, 18.0), crate::icons::Icon::Check, crate::theme::accent(dark), None);
                            } else {
                                crate::icons::paint(ui.painter(), crate::icons::rect_at(ic, 20.0), crate::icons::Icon::CloudDownload, crate::theme::icon(dark), None);
                            }
                            let mb = (row.5 as f64 / 1_048_576.0).round().max(1.0);
                            ui.painter().text(egui::pos2(ic.x, rr.top() + 32.0), egui::Align2::CENTER_TOP, format!("{mb:.0} MB"), egui::FontId::proportional(10.0), crate::theme::muted(dark));
                            if resp.on_hover_text(&row.3).clicked() {
                                pick = Some(bref);
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 190.0).max(0.0));
                        let later = ui.add(egui::Button::new("Later").min_size(egui::vec2(186.0, 30.0)));
                        if later.clicked() {
                            if can_close {
                                self.request_back();
                            } else {
                                self.tab = crate::app::Tab::Home;
                            }
                        }
                    });
                    if let Some(b) = pick {
                        self.add_bible(b.clone());
                        if !App::bible_downloaded(&b) {
                            self.download_bible(&b);
                        }
                        self.bible_filter.clear();
                        if can_close {
                            self.request_back();
                        }
                    }
                }); });
        });
    }
}
