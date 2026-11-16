use crate::app::App;
use crate::widgets;
use egui::{RichText, Ui};

impl App {
    pub fn ui_search(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        widgets::header(self, ui, "Search", None);
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.search.query)
                    .hint_text("Search words, a scripture (e.g. Ps 23:1) or a title")
                    .desired_width(520.0)
                    .font(egui::FontId::proportional(17.0)),
            );
            if self.search.focus {
                r.request_focus();
                self.search.focus = false;
            }
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter || widgets::accent_button(ui, "🔍 Search").clicked() {
                // a scripture reference opens the Bible directly
                let mut names: Vec<(u8, String, String)> = crate::bible::BOOKS.iter().enumerate().map(|(i, b)| (i as u8 + 1, b.name.to_string(), b.abbr.to_string())).collect();
                if let Some((_, idx)) = self.bible_index() {
                    names.splice(0..0, idx.iter().map(|b| (b.num, b.name.clone(), b.abbr.clone())));
                }
                let q = self.search.query.clone();
                if q.chars().any(|c| c.is_ascii_digit()) {
                    if let Some((b, c, v)) = crate::bible::parse_reference(&q, &names) {
                        self.open_bible(b, c, v);
                        return;
                    }
                }
                self.start_search();
            }
            if self.search.running {
                ui.spinner();
            }
        });
        let scopes = ["All", "Bible", "Publications", "Notes", "Titles"];
        ui.horizontal(|ui| {
            for (i, s) in scopes.iter().enumerate() {
                ui.selectable_value(&mut self.search.scope, i, *s);
            }
        });
        ui.separator();
        let q = self.search.done_query.clone();
        if q.is_empty() {
            widgets::empty_state(ui, dark, "🔍", "Search your library", "Searches the Bible and every downloaded publication on this computer, your notes, and publication titles.");
            return;
        }
        let notes = if matches!(self.search.scope, 0 | 3) { self.user.notes(&q, None) } else { vec![] };
        let titles = if matches!(self.search.scope, 0 | 4) && self.catalog.ready() { self.catalog.search(&q) } else { vec![] };
        let hits: Vec<crate::app::SearchHit> = self
            .search
            .hits
            .iter()
            .filter(|h| match self.search.scope {
                1 => h.pubinfo.type_id == 1 || h.book_chapter.is_some(),
                2 => h.book_chapter.is_none(),
                3 | 4 => false,
                _ => true,
            })
            .cloned()
            .collect();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new(format!("{} matches in publications{}", hits.len(), if self.search.running { " (still searching…)" } else { "" })).color(crate::theme::muted(dark)));
            let mut open_hit = None;
            for h in &hits {
                widgets::card(ui, dark, |ui| {
                    ui.set_width(ui.available_width().min(860.0));
                    let title = match h.book_chapter {
                        Some(_) => h.title.clone(),
                        None => format!("{} — {}", h.title, h.pubinfo.short_title),
                    };
                    if ui.link(RichText::new(title).font(crate::fonts::bold(15.0))).clicked() {
                        open_hit = Some(h.clone());
                    }
                    // show the matched words in bold
                    let mut job = egui::text::LayoutJob::default();
                    let lower = h.snippet.to_lowercase();
                    let ql = q.to_lowercase();
                    let mut from = 0;
                    let fmt = |bold: bool| egui::TextFormat {
                        font_id: if bold { crate::fonts::bold(14.0) } else { egui::FontId::proportional(14.0) },
                        color: crate::theme::text(dark),
                        background: if bold { crate::theme::HIGHLIGHT_LIGHT[0].linear_multiply(if dark { 0.4 } else { 0.7 }) } else { egui::Color32::TRANSPARENT },
                        ..Default::default()
                    };
                    if lower.len() == h.snippet.len() {
                        while let Some(p) = lower[from..].find(&ql) {
                            let s = from + p;
                            job.append(&h.snippet[from..s], 0.0, fmt(false));
                            job.append(&h.snippet[s..s + ql.len()], 0.0, fmt(true));
                            from = s + ql.len();
                        }
                    }
                    job.append(&h.snippet[from..], 0.0, fmt(false));
                    job.wrap.max_width = ui.available_width();
                    ui.label(job);
                });
                ui.add_space(4.0);
            }
            if let Some(h) = open_hit {
                if let Some((b, c)) = h.book_chapter {
                    self.open_bible(b, c, None);
                } else {
                    self.open_doc(&h.pubinfo, &h.href);
                }
                if let Some(crate::app::Page::Reader(rd)) = self.stacks.get_mut(&self.tab).and_then(|s| s.last_mut()) {
                    rd_find(rd, &q);
                }
            }
            if !notes.is_empty() {
                widgets::section(ui, dark, &format!("Notes ({})", notes.len()));
                for n in &notes {
                    let t = if n.title.is_empty() { "(Untitled note)".to_string() } else { n.title.clone() };
                    if ui.link(format!("📝 {t}")).clicked() {
                        self.edit_note(n.clone());
                    }
                }
            }
            if !titles.is_empty() {
                widgets::section(ui, dark, &format!("Publication titles ({})", titles.len()));
                if let Some(i) = widgets::tile_grid(self, ui, &titles, 104.0) {
                    let p = titles[i].clone();
                    self.tab = crate::app::Tab::Publications;
                    self.open_pub(&p);
                }
            }
            if hits.is_empty() && notes.is_empty() && titles.is_empty() && !self.search.running {
                widgets::empty_state(ui, dark, "🔍", "No results", "Download more publications to search them.");
            }
        });
    }
}

fn rd_find(_rd: &mut crate::reader::Reader, _q: &str) {
    // The reader opens at the top; its own Find box (🔍) highlights the words.
}
