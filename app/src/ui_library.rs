//! Library page, laid out like the original (person's screenshots media-page.png and
//! videos-tab.png): a white tab strip PUBLICATIONS | VIDEO | AUDIO | DOWNLOADED under the header
//! ("Library" with the language below); publications as a three-column grid of white cards with
//! outline icons; video and audio categories as picture banners with the name on the left.

use crate::app::{App, Loadable, MediaCategory, Page};
use crate::icons::Icon;
use crate::widgets;
use egui::{Color32, RichText, Ui};

/// The original's publication categories, in its order, with our catalog type ids.
const PUB_CATEGORIES: [(&str, &[i64], Icon); 11] = [
    ("Books", &[2], Icon::Book),
    ("Brochures and Booklets", &[4], Icon::Brochure),
    ("Tracts and Invitations", &[10], Icon::Tract),
    ("Article Series", &[22], Icon::Article),
    ("Watchtower", &[14], Icon::Tower),
    ("Awake!", &[13], Icon::Exclaim),
    ("Meeting Workbooks", &[30], Icon::Workbook),
    ("Kingdom Ministry", &[7], Icon::Ministry),
    ("Programs", &[31, 3], Icon::Clock),
    ("Index", &[5, 33], Icon::Index),
    ("Guidelines", &[6], Icon::List),
];

fn columns(width: f32, min: f32) -> usize {
    ((width / min).floor() as usize).clamp(1, 3)
}

impl App {
    pub fn ui_library(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        self.page_title = "Library".into();
        self.page_subtitle = crate::lang::by_symbol(&self.settings.lang).map(|l| l.english.clone()).unwrap_or_default();
        // tab strip
        let (strip, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::hover());
        ui.painter().rect_filled(strip, 0.0, crate::theme::content_bg(dark));
        let mut x = strip.left() + 14.0;
        for (i, label) in ["PUBLICATIONS", "VIDEO", "AUDIO", "DOWNLOADED"].iter().enumerate() {
            let sel = self.library_view as usize == i;
            let col = if sel { crate::theme::accent(dark) } else { crate::theme::text(dark) };
            let g = ui.painter().layout_no_wrap(label.to_string(), if sel { crate::fonts::bold(13.0) } else { egui::FontId::proportional(13.0) }, col);
            let r = egui::Rect::from_min_size(egui::pos2(x - 4.0, strip.top()), egui::vec2(g.size().x + 8.0, strip.height()));
            let resp = ui.interact(r, egui::Id::new(("libtab", i)), egui::Sense::click());
            let gw = g.size().x;
            ui.painter().galley(egui::pos2(x, strip.center().y - g.size().y / 2.0), g, col);
            if sel {
                ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, strip.bottom() - 3.0), egui::vec2(gw, 3.0)), 0.0, crate::theme::accent(dark));
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.library_view = i as u8;
                self.tab = crate::app::Tab::Publications;
            }
            x += gw + 22.0;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(14.0);
            egui::Frame::default().inner_margin(egui::Margin { left: 16, right: 16, top: 0, bottom: 20 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                match self.library_view {
                    0 => self.library_publications(ui),
                    1 => self.library_media(ui, "VideoOnDemand"),
                    2 => self.library_media(ui, "Audio"),
                    _ => self.library_downloaded(ui),
                }
            });
        });
    }

    fn library_publications(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        if !self.catalog.ready() {
            self.catalog_notice(ui);
            return;
        }
        let counts = self.catalog.type_counts();
        let cats: Vec<&(&str, &[i64], Icon)> = PUB_CATEGORIES.iter().filter(|(_, ids, _)| ids.iter().any(|t| counts.iter().any(|(c, n)| c == t && *n > 0))).collect();
        let w = ui.available_width();
        let cols = columns(w, 230.0);
        let gap = 2.0;
        let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        let mut open: Option<i64> = None;
        ui.spacing_mut().item_spacing.y = 0.0;
        for row in cats.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (name, ids, icon) in row {
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(cw, 64.0), egui::Sense::click());
                    let bg = if resp.hovered() { if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xf7f7f7) } } else { crate::theme::card(dark) };
                    ui.painter().rect_filled(r, 0.0, bg);
                    crate::icons::paint(ui.painter(), crate::icons::rect_at(egui::pos2(r.left() + 30.0, r.center().y), 30.0), *icon, crate::theme::text(dark), None);
                    ui.painter().text(egui::pos2(r.left() + 62.0, r.center().y), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(14.0), crate::theme::text(dark));
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        open = ids.iter().copied().find(|t| counts.iter().any(|(c, n)| c == t && *n > 0));
                    }
                }
            });
            ui.add_space(gap);
        }
        if let Some(t) = open {
            self.push(Page::PubList(t));
        }
        if let Some(u) = self.catalog.updated() {
            ui.add_space(10.0);
            ui.label(RichText::new(format!("Publication list updated {u}")).size(11.5).color(crate::theme::muted(dark)));
        }
    }

    fn library_media(&mut self, ui: &mut Ui, root: &str) {
        let dark = self.dark;
        self.load_media(root);
        let k = format!("{}|{}", self.settings.lang, root);
        let subs: Vec<MediaCategory> = match self.media.get(&k) {
            Some(Loadable::Ready(c)) => c.subcats.clone(),
            Some(Loadable::Failed(e)) => {
                let e = e.clone();
                widgets::empty_state(ui, dark, "📡", "Unable to load", &format!("Videos and audio are listed when you are online. ({e})"));
                if ui.button("Try again").clicked() {
                    self.media.remove(&k);
                }
                return;
            }
            _ => {
                ui.spinner();
                return;
            }
        };
        let w = ui.available_width();
        let cols = columns(w, 240.0);
        let gap = 2.0;
        let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        let ch = (cw / 4.2).clamp(60.0, 104.0);
        let mut open: Option<(String, String)> = None;
        ui.spacing_mut().item_spacing.y = 0.0;
        for row in subs.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for c in row {
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(cw, ch), egui::Sense::click());
                    if !ui.is_rect_visible(r) {
                        continue;
                    }
                    ui.painter().rect_filled(r, 0.0, crate::theme::hex(0x161616));
                    // picture on the right ~70 %, fading into black on the left
                    let pic = egui::Rect::from_min_max(egui::pos2(r.left() + r.width() * 0.3, r.top()), r.max);
                    let url = if !c.banner.is_empty() { Some(c.banner.clone()) } else if !c.image.is_empty() { Some(c.image.clone()) } else { None };
                    if let Some(u) = url {
                        if let Some(t) = self.image(&u) {
                            let [iw, ih] = t.size();
                            let ra = pic.width() / pic.height();
                            let ia = iw as f32 / ih.max(1) as f32;
                            let uv = if ia > ra {
                                let f = ra / ia;
                                egui::Rect::from_min_max(egui::pos2(1.0 - f, 0.0), egui::pos2(1.0, 1.0))
                            } else {
                                let f = ia / ra;
                                egui::Rect::from_min_max(egui::pos2(0.0, (1.0 - f) / 2.0), egui::pos2(1.0, (1.0 + f) / 2.0))
                            };
                            ui.painter().image(t.id(), pic, uv, Color32::WHITE);
                        }
                    }
                    let fade = egui::Rect::from_min_max(pic.left_top(), egui::pos2(pic.left() + pic.width() * 0.45, pic.bottom()));
                    let mut mesh = egui::Mesh::default();
                    let a = Color32::from_rgb(0x16, 0x16, 0x16);
                    let b = Color32::from_rgba_unmultiplied(0x16, 0x16, 0x16, 0);
                    mesh.colored_vertex(fade.left_top(), a);
                    mesh.colored_vertex(fade.right_top(), b);
                    mesh.colored_vertex(fade.right_bottom(), b);
                    mesh.colored_vertex(fade.left_bottom(), a);
                    mesh.add_triangle(0, 1, 2);
                    mesh.add_triangle(0, 2, 3);
                    ui.painter().add(egui::Shape::mesh(mesh));
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 0.0, Color32::from_white_alpha(12));
                    }
                    let g = ui.painter().layout(c.name.clone(), egui::FontId::proportional(15.0), Color32::WHITE, r.width() * 0.45);
                    ui.painter().galley(egui::pos2(r.left() + 14.0, r.center().y - g.size().y / 2.0), g, Color32::WHITE);
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        open = Some((c.key.clone(), c.name.clone()));
                    }
                }
            });
            ui.add_space(gap);
        }
        if let Some((k, n)) = open {
            self.push(Page::MediaCategory(k, n));
        }
    }

    fn library_downloaded(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let pubs = self.downloaded_pubs();
        let local: Vec<std::path::PathBuf> = std::fs::read_dir(crate::paths::media_dir())
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|e| e != "part").unwrap_or(false)).collect())
            .unwrap_or_default();
        if pubs.is_empty() && local.is_empty() {
            widgets::empty_state(ui, dark, "⬇", "Nothing downloaded yet", "Publications and media you download appear here.");
            return;
        }
        if !pubs.is_empty() {
            widgets::section(ui, dark, "Publications");
            if let Some(i) = widgets::tile_grid(self, ui, &pubs, 112.0) {
                let p = pubs[i].clone();
                self.open_pub(&p);
            }
        }
        if !local.is_empty() {
            widgets::section(ui, dark, "Media");
            let mut play: Option<(String, String, bool)> = None;
            for p in &local {
                ui.horizontal(|ui| {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    if ui.button("▶").clicked() {
                        let audio = name.ends_with(".mp3") || name.ends_with(".m4a");
                        play = Some((name.clone(), p.to_string_lossy().into_owned(), audio));
                    }
                    ui.label(&name);
                    if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                        let _ = std::fs::remove_file(p);
                    }
                });
            }
            if let Some((t, u, a)) = play {
                self.play(vec![(t, u, String::new())], a);
            }
        }
    }
}
