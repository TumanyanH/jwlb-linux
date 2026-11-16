//! Thin outline icons drawn in the style of the original's navigation icons
//! (nav_home, nav_bible, nav_library, nav_meetings, nav_personal_study ...): 1.5 px grey strokes,
//! purple strokes with a light purple fill when selected. Drawn here, not copied.

use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Home,
    Bible,
    Library,
    Meetings,
    Study,
    Search,
    Settings,
    Globe,
    OnlineLibrary,
    Broadcast,
    Donate,
    Menu,
    Calendar,
    Back,
    History,
    More,
    Chevron,
    Audio,
    CloudDownload,
    Check,
    Book,
    Brochure,
    Tract,
    Article,
    Tower,
    Exclaim,
    Workbook,
    Ministry,
    Clock,
    Index,
    List,
    Stadium,
    Bookmark,
    Star,
    ChevronLeft,
    Note,
}

pub fn paint(p: &Painter, r: Rect, icon: Icon, color: Color32, fill: Option<Color32>) {
    let s = r.width().min(r.height());
    let c = r.center();
    let u = s / 24.0; // design grid of 24 units
    let at = |x: f32, y: f32| -> Pos2 { pos2(c.x + (x - 12.0) * u, c.y + (y - 12.0) * u) };
    let st = Stroke::new(1.5 * u.max(0.75), color);
    let poly = |pts: Vec<Pos2>, closed: bool| {
        if closed {
            if let Some(f) = fill {
                p.add(Shape::convex_polygon(pts.clone(), f, Stroke::NONE));
            }
            p.add(Shape::closed_line(pts, st));
        } else {
            p.add(Shape::line(pts, st));
        }
    };
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32, filled: bool| {
        let rr = Rect::from_min_max(at(x0, y0), at(x1, y1));
        if filled {
            if let Some(f) = fill {
                p.rect_filled(rr, 1.0 * u, f);
            }
        }
        p.rect_stroke(rr, 1.0 * u, st, egui::StrokeKind::Middle);
    };
    match icon {
        Icon::Home => {
            poly(vec![at(4.0, 11.0), at(12.0, 4.0), at(20.0, 11.0), at(20.0, 20.0), at(4.0, 20.0)], true);
            poly(vec![at(16.5, 7.5), at(16.5, 4.5), at(18.5, 4.5), at(18.5, 9.3)], false);
            rect(10.0, 14.0, 14.0, 20.0, false);
        }
        Icon::Bible => {
            // an open book: two pages meeting at the spine
            poly(vec![at(12.0, 7.0), at(8.5, 5.5), at(3.0, 5.5), at(3.0, 18.0), at(8.5, 18.0), at(12.0, 19.5)], true);
            poly(vec![at(12.0, 7.0), at(15.5, 5.5), at(21.0, 5.5), at(21.0, 18.0), at(15.5, 18.0), at(12.0, 19.5)], true);
            poly(vec![at(12.0, 7.0), at(12.0, 19.5)], false);
            poly(vec![at(5.5, 9.0), at(9.5, 9.0)], false);
            poly(vec![at(5.5, 12.0), at(9.5, 12.0)], false);
            poly(vec![at(14.5, 9.0), at(18.5, 9.0)], false);
            poly(vec![at(14.5, 12.0), at(18.5, 12.0)], false);
        }
        Icon::Library => {
            rect(3.5, 7.0, 15.5, 17.0, true);
            poly(vec![at(6.0, 7.0), at(6.0, 4.5), at(18.0, 4.5), at(18.0, 12.0)], false);
            poly(vec![at(8.0, 9.5), at(8.0, 14.5), at(12.0, 12.0)], true);
            p.circle_stroke(at(16.5, 19.0), 1.6 * u, st);
            p.circle_stroke(at(20.5, 18.0), 1.6 * u, st);
            poly(vec![at(18.1, 19.0), at(18.1, 13.0), at(22.1, 12.0), at(22.1, 18.0)], false);
        }
        Icon::Meetings => {
            if let Some(f) = fill {
                p.circle_filled(at(9.0, 8.0), 3.0 * u, f);
            }
            p.circle_stroke(at(9.0, 8.0), 3.0 * u, st);
            poly(vec![at(3.5, 19.5), at(4.5, 15.0), at(7.0, 13.0), at(11.0, 13.0), at(13.5, 15.0), at(14.5, 19.5)], false);
            p.circle_stroke(at(16.5, 8.5), 2.5 * u, st);
            poly(vec![at(15.5, 13.0), at(18.0, 13.0), at(20.0, 14.5), at(21.0, 18.5)], false);
        }
        Icon::Study => {
            rect(5.0, 4.0, 17.0, 20.0, true);
            poly(vec![at(8.0, 8.5), at(14.0, 8.5)], false);
            poly(vec![at(8.0, 11.5), at(14.0, 11.5)], false);
            poly(vec![at(8.0, 14.5), at(11.5, 14.5)], false);
            poly(vec![at(20.5, 7.0), at(14.5, 17.0), at(13.5, 19.5), at(15.8, 18.0), at(21.8, 8.0)], false);
        }
        Icon::Search => {
            p.circle_stroke(at(10.5, 10.5), 6.0 * u, st);
            poly(vec![at(15.0, 15.0), at(20.5, 20.5)], false);
        }
        Icon::Settings => {
            p.circle_stroke(at(12.0, 12.0), 3.0 * u, st);
            let mut pts = vec![];
            for i in 0..16 {
                let a = i as f32 * std::f32::consts::TAU / 16.0;
                let rad = if (i / 2) % 2 == 0 { 8.5 } else { 6.5 };
                pts.push(at(12.0 + rad * a.cos(), 12.0 + rad * a.sin()));
            }
            p.add(Shape::closed_line(pts, st));
        }
        Icon::Globe | Icon::OnlineLibrary => {
            p.circle_stroke(at(12.0, 12.0), 8.0 * u, st);
            poly(vec![at(4.0, 12.0), at(20.0, 12.0)], false);
            p.add(Shape::ellipse_stroke(at(12.0, 12.0), vec2(3.5 * u, 8.0 * u), st));
            if icon == Icon::OnlineLibrary {
                poly(vec![at(5.5, 8.0), at(18.5, 8.0)], false);
                poly(vec![at(5.5, 16.0), at(18.5, 16.0)], false);
            }
        }
        Icon::Broadcast => {
            rect(3.5, 6.0, 20.5, 17.0, true);
            poly(vec![at(8.0, 20.0), at(16.0, 20.0)], false);
            poly(vec![at(10.0, 9.0), at(10.0, 14.0), at(14.5, 11.5)], true);
        }
        Icon::Donate => {
            poly(
                vec![at(12.0, 19.5), at(4.5, 12.0), at(4.0, 8.5), at(6.0, 5.5), at(9.5, 5.0), at(12.0, 7.5), at(14.5, 5.0), at(18.0, 5.5), at(20.0, 8.5), at(19.5, 12.0)],
                false,
            );
            poly(vec![at(19.5, 12.0), at(12.0, 19.5)], false);
        }
        Icon::Menu => {
            for y in [7.0, 12.0, 17.0] {
                poly(vec![at(4.0, y), at(20.0, y)], false);
            }
        }
        Icon::Calendar => {
            rect(4.0, 5.5, 20.0, 20.0, true);
            poly(vec![at(4.0, 9.5), at(20.0, 9.5)], false);
            poly(vec![at(8.5, 3.5), at(8.5, 7.0)], false);
            poly(vec![at(15.5, 3.5), at(15.5, 7.0)], false);
        }
        Icon::History => {
            p.add(Shape::line((0..=20).map(|i| { let a = -2.4 + i as f32 * 0.24; at(12.0 + 8.0 * a.cos(), 12.0 + 8.0 * a.sin()) }).collect(), st));
            poly(vec![at(3.0, 6.5), at(4.6, 10.0), at(8.0, 8.8)], false);
            poly(vec![at(12.0, 8.0), at(12.0, 12.5), at(15.0, 14.5)], false);
        }
        Icon::More => {
            for x in [6.0, 12.0, 18.0] {
                p.circle_filled(at(x, 12.0), 1.4 * u, color);
            }
        }
        Icon::Chevron => {
            poly(vec![at(9.0, 5.0), at(16.0, 12.0), at(9.0, 19.0)], false);
        }
        Icon::Audio => {
            poly(vec![at(4.0, 15.0), at(4.0, 12.0)], false);
            p.add(Shape::line((0..=16).map(|i| { let a = std::f32::consts::PI + i as f32 * std::f32::consts::PI / 16.0; at(12.0 + 8.0 * a.cos(), 13.0 + 8.0 * a.sin()) }).collect(), st));
            rect(3.0, 14.0, 6.5, 20.0, false);
            rect(17.5, 14.0, 21.0, 20.0, false);
        }
        Icon::CloudDownload => {
            p.add(Shape::line(
                vec![at(7.0, 16.0), at(4.5, 15.0), at(3.0, 12.5), at(4.0, 9.5), at(7.0, 8.5), at(9.0, 5.5), at(12.5, 4.5), at(16.0, 6.0), at(17.5, 9.0), at(20.0, 9.8), at(21.0, 12.5), at(20.0, 15.0), at(17.0, 16.0)],
                st,
            ));
            poly(vec![at(12.0, 10.0), at(12.0, 20.0)], false);
            poly(vec![at(9.0, 17.0), at(12.0, 20.0), at(15.0, 17.0)], false);
        }
        Icon::Check => {
            poly(vec![at(5.0, 12.5), at(10.0, 17.5), at(19.0, 7.0)], false);
        }
        Icon::Book => {
            rect(5.0, 3.0, 17.0, 21.0, false);
            poly(vec![at(17.0, 5.0), at(19.5, 5.0), at(19.5, 19.0), at(17.0, 19.0)], false);
            for y in [7.0, 9.5] {
                poly(vec![at(8.5, y), at(13.5, y)], false);
            }
        }
        Icon::Brochure => {
            rect(5.0, 3.0, 19.0, 21.0, false);
            poly(vec![at(8.0, 6.0), at(16.0, 6.0)], false);
            rect(8.0, 9.0, 16.0, 15.0, false);
            p.circle_stroke(at(12.0, 12.0), 1.6 * u, st);
            poly(vec![at(8.0, 18.0), at(16.0, 18.0)], false);
        }
        Icon::Tract => {
            rect(4.0, 4.0, 20.0, 20.0, false);
            poly(vec![at(4.0, 9.0), at(20.0, 9.0)], false);
            poly(vec![at(4.0, 15.0), at(20.0, 15.0)], false);
            poly(vec![at(8.0, 12.0), at(16.0, 12.0)], false);
        }
        Icon::Article => {
            rect(3.0, 6.0, 19.0, 21.0, false);
            poly(vec![at(6.0, 6.0), at(6.0, 3.5), at(21.0, 3.5), at(21.0, 18.0), at(19.0, 18.0)], false);
            rect(6.0, 10.0, 11.0, 15.0, false);
            poly(vec![at(13.0, 11.0), at(16.5, 11.0)], false);
            poly(vec![at(13.0, 14.0), at(16.5, 14.0)], false);
        }
        Icon::Tower => {
            poly(vec![at(5.0, 4.0), at(8.0, 4.0), at(8.0, 6.0), at(10.5, 6.0), at(10.5, 4.0), at(13.5, 4.0), at(13.5, 6.0), at(16.0, 6.0), at(16.0, 4.0), at(19.0, 4.0), at(19.0, 9.0), at(17.0, 10.0), at(17.0, 20.0), at(7.0, 20.0), at(7.0, 10.0), at(5.0, 9.0), at(5.0, 4.0)], false);
            rect(10.5, 15.0, 13.5, 20.0, false);
        }
        Icon::Exclaim => {
            poly(vec![at(9.0, 3.0), at(15.0, 3.0), at(13.5, 15.0), at(10.5, 15.0), at(9.0, 3.0)], false);
            rect(10.5, 17.5, 13.5, 20.5, false);
        }
        Icon::Workbook => {
            rect(4.0, 3.0, 18.0, 21.0, false);
            poly(vec![at(18.0, 5.0), at(20.0, 5.0), at(20.0, 21.0), at(6.0, 21.0)], false);
            p.circle_stroke(at(11.0, 9.5), 2.2 * u, st);
            poly(vec![at(7.0, 17.5), at(8.0, 14.0), at(14.0, 14.0), at(15.0, 17.5)], false);
        }
        Icon::Ministry => {
            rect(4.0, 3.0, 20.0, 21.0, false);
            p.circle_stroke(at(9.5, 9.0), 1.8 * u, st);
            p.circle_stroke(at(14.5, 9.0), 1.8 * u, st);
            poly(vec![at(7.0, 17.5), at(7.5, 13.0), at(11.5, 13.0), at(12.0, 17.5)], false);
            poly(vec![at(12.0, 17.5), at(12.5, 13.0), at(16.5, 13.0), at(17.0, 17.5)], false);
        }
        Icon::Clock => {
            p.circle_stroke(at(12.0, 12.0), 8.5 * u, st);
            poly(vec![at(12.0, 7.0), at(12.0, 12.0), at(15.5, 14.0)], false);
        }
        Icon::Index => {
            rect(6.0, 3.0, 18.0, 9.0, false);
            rect(4.0, 9.0, 16.0, 15.0, false);
            rect(6.0, 15.0, 18.0, 21.0, false);
        }
        Icon::List => {
            for y in [6.0, 12.0, 18.0] {
                poly(vec![at(4.0, y), at(5.5, y + 1.5), at(8.0, y - 1.5)], false);
                poly(vec![at(10.5, y), at(20.0, y)], false);
            }
        }
        Icon::Stadium => {
            p.add(Shape::ellipse_stroke(at(12.0, 9.0), vec2(9.0 * u, 3.5 * u), st));
            poly(vec![at(3.0, 9.0), at(3.0, 15.0)], false);
            poly(vec![at(21.0, 9.0), at(21.0, 15.0)], false);
            p.add(Shape::line((0..=16).map(|i| { let a = i as f32 * std::f32::consts::PI / 16.0; at(12.0 - 9.0 * a.cos(), 15.0 + 3.5 * a.sin()) }).collect(), st));
        }
        Icon::Bookmark => {
            poly(vec![at(7.0, 3.5), at(17.0, 3.5), at(17.0, 20.5), at(12.0, 16.0), at(7.0, 20.5), at(7.0, 3.5)], false);
        }
        Icon::Star => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                    let rad = if i % 2 == 0 { 8.5 } else { 3.6 };
                    at(12.0 + rad * a.cos(), 12.5 + rad * a.sin())
                })
                .collect();
            if let Some(f) = fill {
                // fill with triangles from the centre (the star isn't convex)
                for i in 0..10 {
                    p.add(Shape::convex_polygon(vec![at(12.0, 12.5), pts[i], pts[(i + 1) % 10]], f, Stroke::NONE));
                }
            }
            p.add(Shape::closed_line(pts, st));
        }
        Icon::ChevronLeft => {
            poly(vec![at(15.0, 5.0), at(8.0, 12.0), at(15.0, 19.0)], false);
        }
        Icon::Note => {
            rect(4.0, 4.0, 20.0, 20.0, false);
            poly(vec![at(8.0, 9.0), at(16.0, 9.0)], false);
            poly(vec![at(8.0, 12.5), at(16.0, 12.5)], false);
            poly(vec![at(8.0, 16.0), at(12.5, 16.0)], false);
        }
        Icon::Back => {
            poly(vec![at(19.0, 12.0), at(5.0, 12.0)], false);
            poly(vec![at(11.0, 6.0), at(5.0, 12.0), at(11.0, 18.0)], false);
        }
    }
}

/// An icon-only button (hover shows a light background, like WinUI app-bar buttons).
pub fn button(ui: &mut egui::Ui, icon: Icon, size: f32, tip: &str) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let (r, resp) = ui.allocate_exact_size(vec2(size + 12.0, size + 12.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xe4e4e4) });
    }
    paint(ui.painter(), Rect::from_center_size(r.center(), vec2(size, size)), icon, crate::theme::icon(dark), None);
    resp.on_hover_text(tip)
}

/// An icon button that can be drawn filled (for example a favourite star).
pub fn button_filled(ui: &mut egui::Ui, icon: Icon, size: f32, tip: &str, fill: Option<Color32>) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let (r, resp) = ui.allocate_exact_size(vec2(size + 12.0, size + 12.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xe4e4e4) });
    }
    let col = if fill.is_some() { crate::theme::accent(dark) } else { crate::theme::icon(dark) };
    paint(ui.painter(), Rect::from_center_size(r.center(), vec2(size, size)), icon, col, fill);
    resp.on_hover_text(tip)
}

pub fn rect_at(c: Pos2, size: f32) -> Rect {
    Rect::from_center_size(c, vec2(size, size))
}
