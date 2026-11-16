use crate::app::{App, Loadable, Page, Tab};
use crate::catalog::PubInfo;
use crate::content::Kind;
use crate::widgets;
use egui::{RichText, Ui};

impl App {
    /// Home, laid out like the original (person's screenshot): a white band with the date and the
    /// day's scripture centred, then Favorites beside Frequently Used, Teaching Toolbox, What's New.
    pub fn ui_home(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let full_w = ui.available_width();
            // ----- Daily text band -----
            let date = self.daily_date;
            self.load_daily(date);
            let k = format!("{}|{}", self.settings.lang, date);
            let ready = matches!(self.daily.get(&k), Some(Loadable::Ready(_)));
            let mut open_full = false;
            egui::Frame::default().fill(crate::theme::content_bg(dark)).inner_margin(egui::Margin::symmetric(24, 18)).show(ui, |ui| {
                ui.set_width(full_w - 48.0);
                ui.vertical_centered(|ui| {
                    let date_txt = date.format("%A, %B %-d").to_string();
                    let font = crate::fonts::bold(21.0);
                    let acc = crate::theme::accent(dark);
                    let g = ui.painter().layout_no_wrap(date_txt, font, acc);
                    let w = g.size().x + 26.0 + 22.0;
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, g.size().y + 4.0), egui::Sense::click());
                    crate::icons::paint(ui.painter(), crate::icons::rect_at(egui::pos2(r.left() + 10.0, r.center().y), 18.0), crate::icons::Icon::Calendar, acc, None);
                    ui.painter().galley(egui::pos2(r.left() + 26.0, r.top() + 2.0), g, acc);
                    crate::icons::paint(ui.painter(), crate::icons::rect_at(egui::pos2(r.right() - 9.0, r.center().y), 16.0), crate::icons::Icon::Chevron, acc, None);
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Open the daily text").clicked() && ready {
                        open_full = true;
                    }
                    ui.add_space(4.0);
                    match self.daily.get(&k) {
                        Some(Loadable::Ready(blocks)) => {
                            let theme = blocks.iter().find(|b| b.kind == Kind::Theme).or_else(|| blocks.iter().find(|b| !matches!(b.kind, Kind::H1 | Kind::H2 | Kind::H3)));
                            if let Some(t) = theme {
                                let r = ui.add(egui::Label::new(RichText::new(&t.text).size(15.0).color(crate::theme::text(dark))).sense(egui::Sense::click()));
                                if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                    open_full = true;
                                }
                            }
                        }
                        Some(Loadable::Failed(_)) => {
                            ui.label(RichText::new("The daily text is available when you are online.").color(crate::theme::muted(dark)));
                            if ui.small_button("Try again").clicked() {
                                self.daily.remove(&k);
                            }
                        }
                        _ => {
                            ui.spinner();
                        }
                    }
                });
                // previous / next day, small and to the sides
                let band = ui.min_rect();
                for (dx, label, delta) in [(band.left() + 4.0, "‹", -1i64), (band.right() - 22.0, "›", 1)] {
                    let rr = egui::Rect::from_min_size(egui::pos2(dx, band.center().y - 14.0), egui::vec2(18.0, 28.0));
                    let resp = ui.interact(rr, egui::Id::new(("dayarrow", delta)), egui::Sense::click());
                    ui.painter().text(rr.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(24.0), if resp.hovered() { crate::theme::accent(dark) } else { crate::theme::muted(dark) });
                    if resp.on_hover_text(if delta < 0 { "Previous day" } else { "Next day" }).clicked() {
                        self.daily_date = date + chrono::Duration::days(delta);
                    }
                }
            });
            if open_full {
                if let Some(Loadable::Ready(blocks)) = self.daily.get(&k) {
                    let doc = crate::content::Doc { title: date.format("%A, %B %-d").to_string(), blocks: blocks.clone(), doc_id: None, pub_symbol: None };
                    let loc = crate::userdata::Loc { key: format!("daily:{}:{}", self.settings.lang, date), title: format!("Daily Text — {}", date.format("%B %-d, %Y")), ..Default::default() };
                    let rd = crate::reader::Reader::from_doc("Daily Text", doc, loc, &self.user);
                    self.push(Page::Reader(Box::new(rd)));
                }
            }

            egui::Frame::default().inner_margin(egui::Margin { left: 24, right: 16, top: 14, bottom: 24 }).show(ui, |ui| {
                ui.set_width(full_w - 40.0);
                let tile: f32 = 112.0;
                // ----- Meetings | Favorites (side by side) -----
                let favs = self.user.favorites();
                let mut open: Option<PubInfo> = None;
                let monday = crate::app::monday_of(chrono::Local::now().date_naive());
                self.load_meetings(monday);
                let mk = format!("{}|{}", self.settings.lang, monday);
                let mut open_meeting: Option<(String, String)> = None;
                let meetings = match self.meetings.get(&mk) {
                    Some(Loadable::Ready(secs)) => Some(secs.clone()),
                    _ => None,
                };
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        // same size as the other tiles on Home
                        let sq = tile;
                        ui.set_width((sq * 2.0 + 14.0).max(250.0));
                        // heading with the week's dates beside it
                        let sunday = monday + chrono::Duration::days(6);
                        let week = if monday.format("%m").to_string() == sunday.format("%m").to_string() {
                            format!("{} {}–{}", monday.format("%B"), monday.format("%-d"), sunday.format("%-d"))
                        } else {
                            format!("{} – {}", monday.format("%B %-d"), sunday.format("%B %-d"))
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Meetings").font(crate::fonts::bold(21.0)).color(crate::theme::text(dark)));
                            ui.add_space(6.0);
                            ui.label(RichText::new(week).size(14.0).color(crate::theme::muted(dark)));
                        });
                        ui.add_space(2.0);
                        match &meetings {
                            Some(secs) if !secs.is_empty() => {
                                // midweek and Watchtower pages side by side, as square boxes
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 14.0;
                                    for s in secs.iter().take(2) {
                                        let Some(it) = s.items.first() else { continue };
                                        let (r, resp) = ui.allocate_exact_size(egui::vec2(sq, sq), egui::Sense::click());
                                        let bg = if resp.hovered() { if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xf9f9f9) } } else { crate::theme::card(dark) };
                                        ui.painter().rect_filled(r, 12.0, bg);
                                        ui.painter().rect_stroke(r, 12.0, egui::Stroke::new(1.0_f32, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xe5e5e5) }), egui::StrokeKind::Inside);
                                        let pad = 8.0;
                                        let inner = r.width() - 2.0 * pad;
                                        let mut y = r.top() + pad;
                                        let g1 = ui.painter().layout(s.title.clone(), egui::FontId::proportional(10.5), crate::theme::accent(dark), inner);
                                        let h1 = g1.size().y;
                                        ui.painter().galley(egui::pos2(r.left() + pad, y), g1, crate::theme::accent(dark));
                                        y += h1 + 3.0;
                                        let g2 = ui.painter().layout(it.line1.clone(), crate::fonts::bold(12.0), crate::theme::text(dark), inner);
                                        let g3 = ui.painter().layout(it.line2.clone(), egui::FontId::proportional(9.5), crate::theme::muted(dark), inner);
                                        let _ = &g3;
                                        let h3 = -4.0; // the publication line shows on hover only
                                        let title_room = egui::Rect::from_min_max(egui::pos2(r.left() + pad, y), egui::pos2(r.right() - pad, r.bottom() - pad - h3 - 4.0));
                                        ui.painter().with_clip_rect(title_room).galley(title_room.min, g2, crate::theme::text(dark));
                                        let sub_rect = egui::Rect::from_min_max(egui::pos2(r.left() + pad, r.bottom() - pad - h3), egui::pos2(r.right() - pad, r.bottom() - pad));
                                        let _ = sub_rect;
                                        if resp.on_hover_text(format!("{}\n{}", it.line1, it.line2)).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                            open_meeting = Some((it.href.clone(), it.line1.clone()));
                                        }
                                    }
                                });
                            }
                            Some(_) => {
                                ui.label(RichText::new("No meeting material for this week yet.").size(12.5).color(crate::theme::muted(dark)));
                            }
                            None => {
                                ui.spinner();
                            }
                        }
                    });
                    ui.add_space(28.0);
                    ui.vertical(|ui| {
                        section_title(ui, dark, "Favorites", None);
                        if favs.is_empty() {
                            ui.set_max_width(260.0);
                            ui.label(RichText::new("Right-click a publication and choose “Add to Favorites”.").size(12.5).color(crate::theme::muted(dark)));
                        } else if let Some(i) = widgets::tile_row(self, ui, "favorites", &favs, tile) {
                            open = Some(favs[i].clone());
                        }
                    });
                });
                ui.add_space(14.0);
                // ----- Teaching Toolbox -----
                if self.catalog.ready() {
                    let tt = self.catalog.curated(2);
                    if !tt.is_empty() {
                        let lang_name = crate::lang::by_symbol(&self.settings.lang).map(|l| l.vernacular.clone()).unwrap_or_default();
                        if section_title(ui, dark, "Teaching Toolbox", Some(&lang_name)) {
                            self.tab = Tab::Settings;
                            self.show_lang_picker = true;
                        }
                        if let Some(i) = widgets::tile_row(self, ui, "toolbox", &tt, tile) {
                            open = Some(tt[i].clone());
                        }
                        ui.add_space(14.0);
                    }
                    // ----- What's New: wide cards -----
                    let new = self.catalog.whats_new(12);
                    if !new.is_empty() {
                        let lang_name = crate::lang::by_symbol(&self.settings.lang).map(|l| l.vernacular.clone()).unwrap_or_default();
                        if section_title(ui, dark, "What’s New", Some(&lang_name)) {
                            self.tab = Tab::Settings;
                            self.show_lang_picker = true;
                        }
                        egui::ScrollArea::horizontal().id_salt("whatsnew").show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                for p in &new {
                                    if wide_card(self, ui, p) {
                                        open = Some(p.clone());
                                    }
                                }
                            });
                        });
                        ui.add_space(14.0);
                    }
                } else {
                    section_title(ui, dark, "What’s New", None);
                    self.catalog_notice(ui);
                }
                // ----- Latest videos (the publisher's "LatestVideos" list) -----
                self.load_media("LatestVideos");
                let lk = format!("{}|LatestVideos", self.settings.lang);
                if let Some(Loadable::Ready(cat)) = self.media.get(&lk) {
                    let vids: Vec<crate::app::MediaItem> = cat.media.clone();
                    if !vids.is_empty() {
                        section_title(ui, dark, "Latest Videos", None);
                        egui::ScrollArea::horizontal().id_salt("latestvideos").show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                for m in &vids {
                                    if crate::ui_media::media_card(self, ui, m, 200.0) {
                                        self.media_popup = Some(m.clone());
                                    }
                                }
                            });
                        });
                        ui.add_space(14.0);
                    }
                }
                if let Some(p) = open {
                    self.tab = Tab::Publications;
                    self.open_pub(&p);
                }
                if let Some((href, title)) = open_meeting {
                    self.tab = Tab::Meetings;
                    self.open_online(&format!("{}{}", crate::net::WOL, href), &title);
                }
            });
        });
    }

    /// Publications opened most often (from reading history), newest first.
    #[allow(dead_code)]
    fn frequently_used(&self) -> Vec<PubInfo> {
        let mut out: Vec<PubInfo> = vec![];
        for l in self.user.recent(60) {
            if let Some(p) = l.pub_info() {
                if p.key_symbol.is_empty() || p.type_id == 1 || l.key.starts_with("bible:") {
                    continue;
                }
                let p = self.catalog.find(&p.key_symbol, p.issue).unwrap_or(p);
                if !out.iter().any(|x| x.key() == p.key()) {
                    out.push(p);
                }
            }
            if out.len() >= 12 {
                break;
            }
        }
        out
    }

    pub fn catalog_notice(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        widgets::card(ui, dark, |ui| {
            ui.set_width(ui.available_width());
            if let Some(p) = &self.catalog_job {
                let f = p.fraction();
                ui.label(RichText::new("Downloading the list of publications…").font(crate::fonts::bold(15.0)));
                ui.add(egui::ProgressBar::new(f).show_percentage());
                ui.label(RichText::new("This happens once per language (about 60 MB).").size(12.5).color(crate::theme::muted(dark)));
            } else {
                ui.label(RichText::new("The list of publications has not been downloaded yet.").font(crate::fonts::bold(15.0)));
                if let Some(e) = &self.catalog_error {
                    ui.label(RichText::new(format!("Last try failed: {e}")).color(egui::Color32::from_rgb(0xc0, 0x40, 0x40)));
                }
                if widgets::accent_button(ui, "Download publication list").clicked() {
                    self.start_catalog_update();
                }
            }
        });
    }
}

/// Section heading (bold, as in the original); an optional purple language link on the right.
/// Returns true when the link was clicked.
fn section_title(ui: &mut Ui, dark: bool, text: &str, link: Option<&str>) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).font(crate::fonts::bold(21.0)).color(crate::theme::text(dark)));
        if let Some(l) = link {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if ui.add(egui::Label::new(RichText::new(l).size(13.0).color(crate::theme::accent(dark))).sense(egui::Sense::click())).clicked() {
                    clicked = true;
                }
            });
        }
    });
    ui.add_space(2.0);
    clicked
}

/// A few tiles in a row without scrolling (Favorites keeps its own width).
#[allow(dead_code)]
fn tiles_inline(app: &mut App, ui: &mut Ui, pubs: &[PubInfo], w: f32) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (i, p) in pubs.iter().take(4).enumerate() {
            if let widgets::TileAction::Open = widgets::pub_tile(app, ui, p, w) {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// What's New card: picture on the left, category and title on the right (white card).
fn wide_card(app: &mut App, ui: &mut Ui, p: &PubInfo) -> bool {
    let dark = app.dark;
    let size = egui::vec2(300.0, 112.0);
    let (r, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if !ui.is_rect_visible(r) {
        return false;
    }
    ui.painter().rect_filled(r, 0.0, if resp.hovered() { if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xfafafa) } } else { crate::theme::card(dark) });
    // square picture, like the Teaching Toolbox tiles
    let pic = egui::Rect::from_min_size(r.min, egui::vec2(r.height(), r.height()));
    let url = p.image_url().or_else(|| if !p.wide_image.is_empty() { Some(format!("{}/{}", crate::net::CATALOG_BASE, p.wide_image)) } else { None });
    widgets::picture(app, ui, pic, url.as_deref(), &p.key_symbol, &p.key_symbol);
    let x = pic.right() + 10.0;
    ui.painter().text(egui::pos2(x, r.top() + 10.0), egui::Align2::LEFT_TOP, crate::catalog::type_name(p.type_id), egui::FontId::proportional(11.5), crate::theme::muted(dark));
    let g = ui.painter().layout(p.display_title(), egui::FontId::proportional(14.0), crate::theme::text(dark), r.right() - x - 10.0);
    ui.painter().with_clip_rect(egui::Rect::from_min_max(egui::pos2(x, r.top() + 28.0), r.max - egui::vec2(6.0, 6.0))).galley(egui::pos2(x, r.top() + 28.0), g, crate::theme::text(dark));
    resp.on_hover_text(p.display_title()).clicked()
}
