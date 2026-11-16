use crate::app::App;
use crate::catalog::PubInfo;
use egui::{Color32, RichText, Sense, Ui};

/// Page title with a back arrow when there is somewhere to go back to.
/// The page title goes into the header bar (as in the original); a subtitle stays on the page.
pub fn header(app: &mut App, ui: &mut Ui, title: &str, subtitle: Option<&str>) {
    let dark = app.dark;
    app.page_title = title.to_string();
    if let Some(s) = subtitle {
        ui.label(RichText::new(s).color(crate::theme::muted(dark)));
    }
    ui.add_space(6.0);
}

pub fn section(ui: &mut Ui, dark: bool, text: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(text).font(crate::fonts::bold(18.0)).color(crate::theme::text(dark)));
    ui.add_space(4.0);
}

pub fn card<R>(ui: &mut Ui, dark: bool, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::default()
        .fill(crate::theme::card(dark))
        .stroke(egui::Stroke::new(1.0, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xe5e5e5) }))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::same(14))
        .show(ui, add)
        .inner
}

pub fn fmt_size(b: u64) -> String {
    if b >= 1 << 30 {
        format!("{:.1} GB", b as f64 / (1u64 << 30) as f64)
    } else if b >= 1 << 20 {
        format!("{:.1} MB", b as f64 / (1u64 << 20) as f64)
    } else {
        format!("{} KB", b / 1024)
    }
}

/// Paint a picture into a rect (cover crop), or a coloured placeholder with a label.
pub fn picture(app: &mut App, ui: &Ui, rect: egui::Rect, url: Option<&str>, placeholder: &str, seed: &str) {
    let tex = url.and_then(|u| app.image(u));
    match tex {
        Some(t) => {
            let [w, h] = t.size();
            let (w, h) = (w as f32, h as f32);
            let ra = rect.width() / rect.height();
            let ia = w / h.max(1.0);
            let uv = if ia > ra {
                let f = ra / ia;
                egui::Rect::from_min_max(egui::pos2((1.0 - f) / 2.0, 0.0), egui::pos2((1.0 + f) / 2.0, 1.0))
            } else {
                let f = ia / ra;
                egui::Rect::from_min_max(egui::pos2(0.0, (1.0 - f) / 2.0), egui::pos2(1.0, (1.0 + f) / 2.0))
            };
            ui.painter().image(t.id(), rect, uv, Color32::WHITE);
        }
        None => {
            let c = crate::theme::tile_color(seed);
            ui.painter().rect_filled(rect, 2.0, c);
            let galley = ui.painter().layout(
                placeholder.to_string(),
                crate::fonts::bold((rect.width() / 6.0).clamp(11.0, 22.0)),
                Color32::WHITE,
                rect.width() - 12.0,
            );
            let pos = rect.center() - galley.size() / 2.0;
            ui.painter().galley(pos, galley, Color32::WHITE);
        }
    }
}

pub enum TileAction {
    None,
    Open,
}

/// A publication card: cover, title, download state; right-click for more.
pub fn pub_tile(app: &mut App, ui: &mut Ui, p: &PubInfo, w: f32) -> TileAction {
    let dark = app.dark;
    let h = w + 36.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
    if !ui.is_rect_visible(rect) {
        return TileAction::None;
    }
    let cover = egui::Rect::from_min_size(rect.min, egui::vec2(w, w));
    let sym = if p.issue > 0 { format!("{}\n{}", p.key_symbol, p.issue / 100 % 10000) } else { p.key_symbol.clone() };
    picture(app, ui, cover, p.image_url().as_deref(), &sym, &p.key_symbol);
    if resp.hovered() {
        ui.painter().rect_filled(cover, 4.0, Color32::from_black_alpha(30));
    }
    let downloaded = p.is_downloaded() || p.pdf_path().exists();
    let key = p.key();
    if let Some(prog) = app.downloads.get(&key) {
        let f = prog.fraction();
        let bar = egui::Rect::from_min_size(egui::pos2(cover.left() + 6.0, cover.bottom() - 12.0), egui::vec2(w - 12.0, 6.0));
        ui.painter().rect_filled(bar, 3.0, Color32::from_black_alpha(120));
        ui.painter().rect_filled(egui::Rect::from_min_size(bar.min, egui::vec2(bar.width() * f.max(0.03), 6.0)), 3.0, crate::theme::PRIMARY_LIGHT);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
    } else if !downloaded {
        let c = cover.right_bottom() - egui::vec2(18.0, 18.0);
        ui.painter().circle_filled(c, 13.0, Color32::from_black_alpha(140));
        ui.painter().text(c, egui::Align2::CENTER_CENTER, "⬇", egui::FontId::proportional(14.0), Color32::WHITE);
    }
    if app.user.is_favorite(&key) {
        let c = cover.right_top() + egui::vec2(-16.0, 16.0);
        ui.painter().circle_filled(c, 12.0, Color32::from_black_alpha(120));
        ui.painter().text(c, egui::Align2::CENTER_CENTER, "★", egui::FontId::proportional(14.0), Color32::from_rgb(255, 214, 90));
    }
    let title = if p.short_title.is_empty() || p.issue > 0 { p.display_title() } else { p.short_title.clone() };
    let galley = ui.painter().layout(title, egui::FontId::proportional(12.0), crate::theme::text(dark), w);
    let mut rows = galley.rows.len();
    let gpos = egui::pos2(rect.left(), cover.bottom() + 3.0);
    if rows > 2 {
        rows = 2;
    }
    let clip = egui::Rect::from_min_size(gpos, egui::vec2(w, rows as f32 * 15.0));
    ui.painter().with_clip_rect(clip).galley(gpos, galley, crate::theme::text(dark));
    let resp = resp.on_hover_text(p.display_title());
    let mut action = TileAction::None;
    if resp.clicked() {
        action = TileAction::Open;
    }
    let p2 = p.clone();
    resp.context_menu(|ui| {
        let fav = app.user.is_favorite(&key);
        if ui.button(if fav { "☆ Remove from Favorites" } else { "★ Add to Favorites" }).clicked() {
            app.user.toggle_favorite(&p2);
            ui.close_menu();
        }
        if downloaded {
            if ui.button("🗑 Delete download").clicked() {
                app.delete_pub(&p2);
                ui.close_menu();
            }
        } else if ui.button("⬇ Download").clicked() {
            app.download_pub(&p2);
            ui.close_menu();
        }
        if ui.button("🌐 Open on the web").clicked() {
            crate::paths::open_external(&format!(
                "https://www.jw.org/finder?wtlocale={}&pub={}{}",
                p2.lang,
                p2.key_symbol,
                crate::net::issue_param(p2.issue).map(|i| format!("&issue={i}")).unwrap_or_default()
            ));
            ui.close_menu();
        }
    });
    action
}

/// Lay out tiles in a wrapping grid. Returns the index of a clicked tile.
pub fn tile_grid(app: &mut App, ui: &mut Ui, pubs: &[PubInfo], tile_w: f32) -> Option<usize> {
    let mut clicked = None;
    let avail = ui.available_width();
    let gap = 16.0;
    let cols = (((avail + gap) / (tile_w + gap)).floor() as usize).max(1);
    for (row_i, row) in pubs.chunks(cols).enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (j, p) in row.iter().enumerate() {
                if let TileAction::Open = pub_tile(app, ui, p, tile_w) {
                    clicked = Some(row_i * cols + j);
                }
            }
        });
        ui.add_space(10.0);
    }
    clicked
}

/// One horizontal row of tiles (as on the original's Home page).
pub fn tile_row(app: &mut App, ui: &mut Ui, id: &str, pubs: &[PubInfo], tile_w: f32) -> Option<usize> {
    let mut clicked = None;
    egui::ScrollArea::horizontal().id_salt(id).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for (i, p) in pubs.iter().enumerate() {
                if let TileAction::Open = pub_tile(app, ui, p, tile_w) {
                    clicked = Some(i);
                }
            }
        });
    });
    clicked
}

pub fn empty_state(ui: &mut Ui, dark: bool, icon: &str, title: &str, text: &str) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(icon).size(48.0).color(crate::theme::muted(dark)));
        ui.add_space(6.0);
        ui.label(RichText::new(title).font(crate::fonts::bold(18.0)).color(crate::theme::text(dark)));
        ui.label(RichText::new(text).color(crate::theme::muted(dark)));
    });
    ui.add_space(20.0);
}

pub fn accent_button(ui: &mut Ui, text: &str) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    ui.add(
        egui::Button::new(RichText::new(text).color(if dark { Color32::BLACK } else { Color32::WHITE }).font(crate::fonts::bold(14.0)))
            .fill(crate::theme::accent(dark))
            .stroke(egui::Stroke::NONE)
            .corner_radius(4.0)
            .min_size(egui::vec2(0.0, 32.0)),
    )
}

/// A card that reacts to the pointer and can be clicked.
pub fn click_card(ui: &mut Ui, dark: bool, width: f32, add: impl FnOnce(&mut Ui)) -> egui::Response {
    let id = ui.next_auto_id();
    let hovered = ui.ctx().data(|d| d.get_temp::<bool>(id).unwrap_or(false));
    let fill = if hovered { if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xf9f9f9) } } else { crate::theme::card(dark) };
    let resp = egui::Frame::default()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xe5e5e5) }))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(width.min(ui.available_width()));
            ui.vertical(|ui| add(ui));
        })
        .response
        .interact(Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    ui.ctx().data_mut(|d| d.insert_temp(id, resp.hovered()));
    resp
}

/// The Library page's tabs (Publications | Media), drawn like the original's pivot headers:
/// grey tab text, the selected one in the default text colour with a purple underline.
pub fn library_pivot(app: &mut App, ui: &mut Ui) {
    let dark = app.dark;
    app.page_title = "Library".into();
    ui.horizontal(|ui| {
        for (t, label) in [(crate::app::Tab::Publications, "PUBLICATIONS"), (crate::app::Tab::Media, "MEDIA")] {
            let sel = app.tab == t;
            let g = ui.painter().layout_no_wrap(label.to_string(), crate::fonts::bold(14.0), if sel { crate::theme::text(dark) } else { crate::theme::muted(dark) });
            let (r, resp) = ui.allocate_exact_size(g.size() + egui::vec2(20.0, 14.0), Sense::click());
            let col = if sel { crate::theme::text(dark) } else if resp.hovered() { crate::theme::text(dark) } else { crate::theme::muted(dark) };
            ui.painter().galley(r.min + egui::vec2(10.0, 4.0), g, col);
            if sel {
                ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(r.left() + 10.0, r.bottom() - 3.0), egui::vec2(r.width() - 20.0, 3.0)), 1.5, crate::theme::accent(dark));
            }
            if resp.clicked() && !sel {
                app.tab = t;
            }
        }
    });
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, crate::theme::separator(dark).gamma_multiply(0.6));
    ui.add_space(6.0);
}

/// An on/off switch (like the original's WinUI toggle switches), purple when on.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let size = egui::vec2(40.0, 20.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let r = rect.height() / 2.0;
    if *on {
        ui.painter().rect_filled(rect, r, crate::theme::accent(dark));
    } else {
        ui.painter().rect_stroke(rect, r, egui::Stroke::new(1.0, crate::theme::icon(dark)), egui::StrokeKind::Inside);
    }
    let x = egui::lerp((rect.left() + r)..=(rect.right() - r), t);
    let knob = if *on { if dark { Color32::BLACK } else { Color32::WHITE } } else { crate::theme::icon(dark) };
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), r - 5.0, knob);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A menu that opens under a button and closes on the next click.
pub fn popup(ui: &mut Ui, resp: &egui::Response, id: &str, add: impl FnOnce(&mut Ui)) {
    let id = ui.make_persistent_id(id);
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    egui::popup::popup_below_widget(ui, id, resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(200.0);
        add(ui);
    });
}
