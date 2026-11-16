//! In-app media player: a large player over the page, which can shrink to a mini player bar.
//! Plays with the built-in engine (media.rs), with no outside player. Skip buttons jump back 5 s
//! and forward 15 s, as the original's player icons (mediaplayer_skip_back_5 /
//! mediaplayer_skip_forward_15) show.

use crate::app::App;
use crate::media::Playback;
use egui::{Color32, RichText};

pub const SKIP_BACK_S: f64 = 5.0;
pub const SKIP_FORWARD_S: f64 = 15.0;

pub struct Player {
    /// (title, address, picture)
    pub items: Vec<(String, String, String)>,
    pub idx: usize,
    pub pb: Playback,
    pub tex: Option<egui::TextureHandle>,
    pub expanded: bool,
    pub audio_only: bool,
    pub seek_drag: Option<f64>,
    pub volume: f32,
}

impl Player {
    fn load(&mut self, i: usize) {
        self.idx = i;
        self.pb = Playback::start(&self.items[i].1, 0.0, !self.audio_only);
        self.pb.set_volume(self.volume);
        self.tex = None;
    }
    fn title(&self) -> String {
        self.items.get(self.idx).map(|x| x.0.clone()).unwrap_or_default()
    }
    fn image(&self) -> String {
        self.items.get(self.idx).map(|x| x.2.clone()).unwrap_or_default()
    }
}

impl App {
    /// Play one or more items (title, address, picture) inside the app.
    pub fn play(&mut self, items: Vec<(String, String, String)>, audio: bool) {
        if items.is_empty() {
            return;
        }
        let audio = audio || items[0].1.ends_with(".mp3") || items[0].1.ends_with(".m4a");
        let pb = Playback::start(&items[0].1, 0.0, !audio);
        let vol = self.player.as_ref().map(|p| p.volume).unwrap_or(1.0);
        pb.set_volume(vol);
        self.player = Some(Player { items, idx: 0, pb, tex: None, expanded: !audio, audio_only: audio, seek_drag: None, volume: vol });
    }
}

fn fmt_t(s: f64) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn controls(app: &mut App, ui: &mut egui::Ui, light: bool) -> bool {
    let mut close = false;
    let Some(p) = app.player.as_mut() else { return false };
    let pos = p.pb.position();
    let dur = p.pb.duration();
    let paused = p.pb.paused();
    let count = p.items.len();
    let fg = if light { Color32::WHITE } else { ui.visuals().text_color() };
    ui.horizontal(|ui| {
        ui.label(RichText::new(fmt_t(p.seek_drag.unwrap_or(pos))).color(fg).monospace());
        let mut v = p.seek_drag.unwrap_or(pos).min(dur.max(1.0));
        let w = ui.available_width() - 70.0;
        ui.spacing_mut().slider_width = w.max(80.0);
        let r = ui.add(egui::Slider::new(&mut v, 0.0..=dur.max(1.0)).show_value(false));
        if r.dragged() {
            p.seek_drag = Some(v);
        }
        if r.drag_stopped() || (r.changed() && !r.dragged()) {
            p.pb.seek(v);
            p.seek_drag = None;
        }
        ui.label(RichText::new(fmt_t(dur)).color(fg).monospace());
    });
    let mut go: Option<usize> = None;
    ui.horizontal(|ui| {
        let b = |t: &str| egui::Button::new(RichText::new(t).size(20.0).color(fg)).frame(false);
        if count > 1 && ui.add(b("⏮")).on_hover_text("Previous").clicked() && p.idx > 0 {
            go = Some(p.idx - 1);
        }
        if ui.add(b("↺5")).on_hover_text("Back 5 seconds").clicked() {
            p.pb.seek(pos - SKIP_BACK_S);
        }
        if ui.add(b(if paused { "▶" } else { "⏸" })).on_hover_text(if paused { "Play" } else { "Pause" }).clicked() {
            p.pb.set_paused(!paused);
        }
        if ui.add(b("15↻")).on_hover_text("Forward 15 seconds").clicked() {
            p.pb.seek(pos + SKIP_FORWARD_S);
        }
        if count > 1 && ui.add(b("⏭")).on_hover_text("Next").clicked() && p.idx + 1 < count {
            go = Some(p.idx + 1);
        }
        if count > 1 {
            ui.label(RichText::new(format!("{} / {}", p.idx + 1, count)).color(fg));
        }
        ui.add_space(12.0);
        ui.label(RichText::new("🔊").color(fg));
        let mut vol = p.volume * 100.0;
        ui.spacing_mut().slider_width = 90.0;
        if ui.add(egui::Slider::new(&mut vol, 0.0..=100.0).show_value(false)).changed() {
            p.volume = vol / 100.0;
            p.pb.set_volume(p.volume);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(b("🗙")).on_hover_text("Close").clicked() {
                close = true;
            }
            if p.expanded {
                if ui.add(b("🗕")).on_hover_text("Minimize").clicked() {
                    p.expanded = false;
                }
                let fs = ui.ctx().input(|i| i.viewport().fullscreen.unwrap_or(false));
                if ui.add(b(if fs { "🗗" } else { "⛶" })).on_hover_text("Full screen").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
                }
            } else if !p.audio_only && ui.add(b("⬆")).on_hover_text("Show player").clicked() {
                p.expanded = true;
            }
        });
    });
    if let Some(i) = go {
        p.load(i);
    }
    close
}

pub fn player_ui(app: &mut App, ctx: &egui::Context) {
    let Some(p) = app.player.as_mut() else { return };
    // next item when one ends
    if p.pb.finished() && p.idx + 1 < p.items.len() {
        let i = p.idx + 1;
        p.load(i);
    }
    if let Some(img) = p.pb.frame() {
        match &mut p.tex {
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => p.tex = Some(ctx.load_texture("player", img, egui::TextureOptions::LINEAR)),
        }
    }
    if let Some(e) = p.pb.error() {
        if p.tex.is_none() && p.pb.position() < 0.5 {
            // the built-in engine could not play this: offer the separate window as a last resort
            let items = p.items.clone();
            app.player = None;
            app.media_fallback = Some((items, e));
            return;
        }
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(10));
    let title = p.title();
    let mut close = false;
    if p.expanded {
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("player_full"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                ui.painter().rect_filled(screen, 0.0, Color32::from_rgb(12, 12, 12));
                ui.set_min_size(screen.size());
                ui.allocate_ui_at_rect(screen.shrink(18.0), |ui| {
                    ui.label(RichText::new(&title).font(crate::fonts::bold(18.0)).color(Color32::WHITE));
                    let ctrl_h = 92.0;
                    let area = egui::vec2(ui.available_width(), (ui.available_height() - ctrl_h).max(100.0));
                    let (r, resp) = ui.allocate_exact_size(area, egui::Sense::click());
                    if let Some(t) = &app.player.as_ref().unwrap().tex {
                        let [w, h] = t.size();
                        let s = (r.width() / w as f32).min(r.height() / h as f32);
                        let vr = egui::Rect::from_center_size(r.center(), egui::vec2(w as f32 * s, h as f32 * s));
                        ui.painter().image(t.id(), vr, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                    } else {
                        ui.put(egui::Rect::from_center_size(r.center(), egui::vec2(40.0, 40.0)), egui::Spinner::new().size(36.0).color(Color32::from_gray(160)));
                    }
                    if resp.clicked() {
                        let pp = app.player.as_mut().unwrap();
                        let pa = pp.pb.paused();
                        pp.pb.set_paused(!pa);
                    }
                    if resp.double_clicked() {
                        let fs = ui.ctx().input(|i| i.viewport().fullscreen.unwrap_or(false));
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
                    }
                    close = controls(app, ui, true);
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            if let Some(p) = app.player.as_mut() {
                let pa = p.pb.paused();
                p.pb.set_paused(!pa);
            }
        }
    } else {
        egui::TopBottomPanel::bottom("miniplayer").exact_height(84.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(112.0, 63.0), egui::Sense::click());
                let tex = app.player.as_ref().unwrap().tex.clone();
                match tex {
                    Some(t) => {
                        ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        let img = app.player.as_ref().unwrap().image();
                        crate::widgets::picture(app, ui, r, if img.is_empty() { None } else { Some(img.as_str()) }, "♪", &title);
                    }
                }
                if resp.clicked() {
                    if let Some(p) = app.player.as_mut() {
                        p.expanded = !p.audio_only;
                    }
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new(&title).strong());
                    close = controls(app, ui, false);
                });
            });
        });
    }
    if close {
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        app.player = None;
    }
}

/// Is this program running inside a Flatpak sandbox (for example a preview inside another app)?
fn in_flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

fn host_has(cmd: &str) -> bool {
    if in_flatpak() {
        std::process::Command::new("flatpak-spawn")
            .args(["--host", "sh", "-c", &format!("command -v {cmd}")])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else {
        std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
            .unwrap_or(false)
    }
}

/// Play in a separate player window: mpv if it exists, then other common players, then the
/// desktop's default handler. Returns the player used.
pub fn play_external(urls: &[String], title: &str) -> Option<String> {
    let players: [(&str, &[&str]); 5] = [
        ("mpv", &["--force-window=yes", "--keep-open=yes"]),
        ("celluloid", &[]),
        ("vlc", &["--play-and-exit"]),
        ("totem", &[]),
        ("smplayer", &[]),
    ];
    for (p, extra) in players {
        if host_has(p) {
            let mut args: Vec<String> = extra.iter().map(|s| s.to_string()).collect();
            if p == "mpv" {
                args.push(format!("--title={title}"));
            }
            args.extend(urls.iter().cloned());
            let r = if in_flatpak() {
                let mut a = vec!["--host".to_string(), p.to_string()];
                a.extend(args);
                std::process::Command::new("flatpak-spawn").args(a).spawn()
            } else {
                std::process::Command::new(p).args(args).spawn()
            };
            if r.is_ok() {
                return Some(p.to_string());
            }
        }
    }
    if let Some(u) = urls.first() {
        crate::paths::open_external(u);
        return Some("the default player".into());
    }
    None
}

/// Shown when the in-app player can't start: explains why and offers a separate player window.
pub fn fallback_window(app: &mut App, ctx: &egui::Context) {
    let Some((items, err)) = app.media_fallback.clone() else { return };
    let dark = app.dark;
    let mut open = true;
    let mut close = false;
    egui::Window::new("Play video")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(RichText::new(&items[0].0).strong());
            ui.add_space(4.0);
            ui.label(RichText::new("Playing inside the app needs the free mpv player on this computer.").color(crate::theme::text(dark)));
            if in_flatpak() {
                ui.label(RichText::new("This copy is running inside a sandbox (a preview), which can't use it. The installed app can.").size(12.5).color(crate::theme::muted(dark)));
            } else {
                ui.label(RichText::new("Install the package named “mpv” (or “libmpv2” / “mpv-libs”) with your software centre, then play again.").size(12.5).color(crate::theme::muted(dark)));
            }
            ui.label(RichText::new(err).size(11.5).color(crate::theme::muted(dark)));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if crate::widgets::accent_button(ui, "▶ Play in a separate window").clicked() {
                    let urls: Vec<String> = items.iter().map(|i| i.1.clone()).collect();
                    match play_external(&urls, &items[0].0) {
                        Some(p) => app.toast(format!("Playing with {p}")),
                        None => app.toast("No video player was found"),
                    }
                    close = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
    if !open || close {
        app.media_fallback = None;
    }
}
