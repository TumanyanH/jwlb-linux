use crate::app::{App, Loadable, MediaCategory, MediaItem, Page};
use crate::widgets;
use egui::{RichText, Ui};

pub fn media_card(app: &mut App, ui: &mut Ui, m: &MediaItem, w: f32) -> bool {
    let dark = app.dark;
    let h = w * 9.0 / 16.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h + 40.0), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return false;
    }
    let pic = egui::Rect::from_min_size(rect.min, egui::vec2(w, h));
    let img = if m.image.is_empty() { None } else { Some(m.image.clone()) };
    widgets::picture(app, ui, pic, img.as_deref(), if m.kind == "audio" { "♪" } else { "▶" }, &m.title);
    if !m.duration.is_empty() {
        let g = ui.painter().layout_no_wrap(m.duration.clone(), egui::FontId::proportional(11.5), egui::Color32::WHITE);
        let r = egui::Rect::from_min_size(pic.right_bottom() - g.size() - egui::vec2(12.0, 8.0), g.size() + egui::vec2(8.0, 4.0));
        ui.painter().rect_filled(r, 3.0, egui::Color32::from_black_alpha(170));
        ui.painter().galley(r.min + egui::vec2(4.0, 2.0), g, egui::Color32::WHITE);
    }
    if resp.hovered() {
        ui.painter().rect_filled(pic, 4.0, egui::Color32::from_black_alpha(40));
        ui.painter().circle_filled(pic.center(), 20.0, egui::Color32::from_black_alpha(140));
        ui.painter().text(pic.center(), egui::Align2::CENTER_CENTER, "▶", egui::FontId::proportional(18.0), egui::Color32::WHITE);
    }
    let g = ui.painter().layout(m.title.clone(), egui::FontId::proportional(13.0), crate::theme::text(dark), w);
    let clip = egui::Rect::from_min_size(egui::pos2(rect.left(), pic.bottom() + 4.0), egui::vec2(w, 34.0));
    ui.painter().with_clip_rect(clip).galley(clip.min, g, crate::theme::text(dark));
    resp.on_hover_text(&m.title).clicked()
}

fn media_grid(app: &mut App, ui: &mut Ui, items: &[MediaItem], w: f32) {
    let avail = ui.available_width() - 8.0;
    let gap = 14.0;
    let cols = (((avail + gap) / (w + gap)).floor() as usize).max(1);
    for row in items.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for m in row {
                if media_card(app, ui, m, w) {
                    app.media_popup = Some(m.clone());
                }
            }
        });
        ui.add_space(10.0);
    }
}

impl App {
    pub fn ui_media_root(&mut self, ui: &mut Ui) {
        widgets::library_pivot(self, ui);
        let dark = self.dark;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // downloaded files
            let local: Vec<std::path::PathBuf> = std::fs::read_dir(crate::paths::media_dir())
                .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|e| e != "part").unwrap_or(false)).collect())
                .unwrap_or_default();
            let mut play_local: Option<(String, String, bool)> = None;
            if !local.is_empty() {
                widgets::section(ui, dark, "Downloaded Media");
                for p in &local {
                    ui.horizontal(|ui| {
                        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        if ui.button("▶").clicked() {
                            let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                            let audio = n.ends_with(".mp3") || n.ends_with(".m4a");
                            play_local = Some((n, p.to_string_lossy().into_owned(), audio));
                        }
                        ui.label(name);
                        if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                            let _ = std::fs::remove_file(p);
                        }
                    });
                }
            }
            if let Some((t, u, a)) = play_local.take() {
                self.play(vec![(t, u, String::new())], a);
            }
            for (key, title) in [("VideoOnDemand", "Videos"), ("Audio", "Audio")] {
                widgets::section(ui, dark, title);
                self.load_media(key);
                let k = format!("{}|{}", self.settings.lang, key);
                let subs: Option<Vec<MediaCategory>> = match self.media.get(&k) {
                    Some(Loadable::Ready(c)) => Some(c.subcats.clone()),
                    Some(Loadable::Failed(e)) => {
                        ui.label(RichText::new(format!("Media is available when you are online. ({e})")).color(crate::theme::muted(dark)));
                        if ui.button("Try again").clicked() {
                            self.media.remove(&k);
                        }
                        None
                    }
                    _ => {
                        ui.spinner();
                        None
                    }
                };
                if let Some(subs) = subs {
                    self.category_tiles(ui, &subs);
                }
            }
        });
    }

    fn category_tiles(&mut self, ui: &mut Ui, subs: &[MediaCategory]) {
        let dark = self.dark;
        let avail = ui.available_width() - 8.0;
        let gap = 14.0;
        let w = 220.0;
        let cols = (((avail + gap) / (w + gap)).floor() as usize).max(1);
        let w = (avail - gap * (cols as f32 - 1.0)) / cols as f32;
        for row in subs.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for c in row {
                    let h = w * 9.0 / 16.0;
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h + 28.0), egui::Sense::click());
                    if ui.is_rect_visible(rect) {
                        let pic = egui::Rect::from_min_size(rect.min, egui::vec2(w, h));
                        let img = if c.image.is_empty() { None } else { Some(c.image.clone()) };
                        widgets::picture(self, ui, pic, img.as_deref(), &c.name, &c.key);
                        if resp.hovered() {
                            ui.painter().rect_filled(pic, 4.0, egui::Color32::from_black_alpha(40));
                        }
                        ui.painter().text(egui::pos2(rect.left(), pic.bottom() + 6.0), egui::Align2::LEFT_TOP, &c.name, crate::fonts::bold(14.0), crate::theme::text(dark));
                    }
                    if resp.clicked() {
                        self.push(Page::MediaCategory(c.key.clone(), c.name.clone()));
                    }
                }
            });
            ui.add_space(8.0);
        }
    }

    pub fn ui_media_category(&mut self, ui: &mut Ui, key: &str, name: &str) {
        let dark = self.dark;
        self.load_media(key);
        let k = format!("{}|{}", self.settings.lang, key);
        let failed = match self.media.get(&k) {
            Some(Loadable::Failed(e)) => Some(e.clone()),
            _ => None,
        };
        if let Some(e) = failed {
            widgets::header(self, ui, name, None);
            widgets::empty_state(ui, dark, "📡", "Unable to load", &e);
            return;
        }
        let cat = match self.media.get(&k) {
            Some(Loadable::Ready(c)) => Some(c.clone()),
            _ => None,
        };
        let Some(cat) = cat else {
            widgets::header(self, ui, name, None);
            ui.spinner();
            return;
        };
        let desc = cat.description.clone();
        widgets::header(self, ui, &cat.name, if desc.is_empty() { None } else { Some(&desc) });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if !cat.media.is_empty() {
                media_grid(self, ui, &cat.media, 220.0);
            }
            let with_media: Vec<&MediaCategory> = cat.subcats.iter().filter(|s| !s.media.is_empty()).collect();
            let without: Vec<MediaCategory> = cat.subcats.iter().filter(|s| s.media.is_empty()).cloned().collect();
            for s in with_media {
                ui.horizontal(|ui| {
                    widgets::section(ui, dark, &s.name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if s.media.len() > 8 && ui.link(format!("See all ({})", s.media.len())).clicked() {
                            self.push(Page::MediaCategory(s.key.clone(), s.name.clone()));
                        }
                    });
                });
                let items: Vec<MediaItem> = s.media.iter().take(8).cloned().collect();
                egui::ScrollArea::horizontal().id_salt(("row", &s.key)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        for m in &items {
                            if media_card(self, ui, m, 220.0) {
                                self.media_popup = Some(m.clone());
                            }
                        }
                    });
                });
            }
            if !without.is_empty() {
                widgets::section(ui, dark, "More");
                self.category_tiles(ui, &without);
            }
            ui.add_space(20.0);
        });
    }
}

fn pick_file<'a>(m: &'a MediaItem, pref: &str) -> Option<&'a (String, String, u64)> {
    let want: u32 = pref.trim_end_matches('p').parse().unwrap_or(720);
    let mut best: Option<&(String, String, u64)> = None;
    let mut best_q = 0u32;
    for f in &m.files {
        let q: u32 = f.0.trim_end_matches('p').parse().unwrap_or(0);
        if f.1.ends_with(".3gp") {
            continue;
        }
        if (q <= want && q >= best_q) || best.is_none() {
            best = Some(f);
            best_q = q;
        }
    }
    best.or(m.files.first())
}

pub fn media_popup(app: &mut App, ctx: &egui::Context) {
    let Some(m) = app.media_popup.clone() else { return };
    let mut open = true;
    let mut play_req: Option<(String, String, String, bool)> = None;
    let dark = app.dark;
    egui::Window::new(&m.title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            let w = 500.0;
            let (r, _) = ui.allocate_exact_size(egui::vec2(w, w * 9.0 / 16.0), egui::Sense::hover());
            let img = if m.image.is_empty() { None } else { Some(m.image.clone()) };
            widgets::picture(app, ui, r, img.as_deref(), "▶", &m.title);
            if !m.duration.is_empty() {
                ui.label(RichText::new(format!("Length: {}", m.duration)).color(crate::theme::muted(dark)));
            }
            let pref = app.settings.video_quality.clone();
            ui.horizontal(|ui| {
                if let Some(f) = pick_file(&m, &pref) {
                    if widgets::accent_button(ui, "▶ Play").clicked() {
                        let local = crate::paths::media_dir().join(f.1.rsplit('/').next().unwrap_or("media"));
                        let src = if local.exists() { local.to_string_lossy().into_owned() } else { f.1.clone() };
                        play_req = Some((m.title.clone(), src, m.image.clone(), m.kind == "audio"));
                    }
                }
                if ui.button("➕ Add to Playlist").clicked() {
                    let f = pick_file(&m, &pref).map(|f| f.1.clone()).unwrap_or_default();
                    app.playlist_picker = Some(crate::userdata::PlaylistItem { id: 0, title: m.title.clone(), url: f, image: m.image.clone(), duration: m.duration.clone() });
                }
                if !m.key.is_empty() && ui.button("🌐 Share link").on_hover_text("Copy a link to this media").clicked() {
                    ui.ctx().copy_text(format!("https://www.jw.org/finder?srcid=jwlshare&wtlocale={}&lank={}", app.settings.lang, m.key));
                    app.toast("Link copied");
                }
            });
            ui.separator();
            ui.label(RichText::new("Download").font(crate::fonts::bold(14.0)));
            ui.horizontal_wrapped(|ui| {
                for f in &m.files {
                    if f.1.ends_with(".3gp") {
                        continue;
                    }
                    let name = f.1.rsplit('/').next().unwrap_or("media").to_string();
                    let dest = crate::paths::media_dir().join(&name);
                    let key = format!("media:{name}");
                    if let Some(p) = app.downloads.get(&key) {
                        ui.add(egui::ProgressBar::new(p.fraction()).desired_width(140.0).show_percentage());
                    } else if dest.exists() {
                        ui.label(format!("✔ {} downloaded", if f.0.is_empty() { "File" } else { &f.0 }));
                    } else {
                        let label = format!("⬇ {} ({})", if f.0.is_empty() { "File" } else { &f.0 }, widgets::fmt_size(f.2));
                        if ui.button(label).clicked() {
                            let prog = crate::app::Progress::new();
                            let (d, t) = (prog.done.clone(), prog.total.clone());
                            let url = f.1.clone();
                            let tx = app.tx.clone();
                            let ctx2 = ui.ctx().clone();
                            let k2 = key.clone();
                            std::thread::spawn(move || {
                                let r = crate::net::download(&url, &dest, d, t).map(|_| "media".to_string());
                                let _ = tx.send(crate::app::Msg::Downloaded(k2, r));
                                ctx2.request_repaint();
                            });
                            app.downloads.insert(key, prog);
                        }
                    }
                }
            });
        });
    if let Some((t, u, i, audio)) = play_req {
        app.media_popup = None;
        app.play(vec![(t, u, i)], audio);
        return;
    }
    if !open {
        app.media_popup = None;
    }
}

pub fn playlist_picker(app: &mut App, ctx: &egui::Context) {
    let Some(item) = app.playlist_picker.clone() else { return };
    let mut open = true;
    let mut done = false;
    egui::Window::new("Add to Playlist").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
        ui.label(RichText::new(&item.title).strong());
        ui.separator();
        for p in app.user.playlists() {
            if ui.button(format!("🎵 {}  ({})", p.name, p.items.len())).clicked() {
                app.user.add_to_playlist(p.id, &item);
                app.toast(format!("Added to “{}”", p.name));
                done = true;
            }
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut app.new_playlist).hint_text("New playlist name").desired_width(200.0));
            if ui.button("Create").clicked() && !app.new_playlist.trim().is_empty() {
                let id = app.user.create_playlist(app.new_playlist.trim());
                app.user.add_to_playlist(id, &item);
                app.toast(format!("Added to “{}”", app.new_playlist.trim()));
                app.new_playlist.clear();
                done = true;
            }
        });
    });
    if !open || done {
        app.playlist_picker = None;
    }
}
