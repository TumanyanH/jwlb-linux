use crate::app::{App, Loadable};
use crate::widgets;
use egui::{RichText, Ui};

impl App {
    pub fn ui_meetings(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        widgets::header(self, ui, "Meetings", None);
        let monday = self.meeting_week;
        let sunday = monday + chrono::Duration::days(6);
        ui.horizontal(|ui| {
            if ui.button("◀").on_hover_text("Previous week").clicked() {
                self.meeting_week = monday - chrono::Duration::days(7);
            }
            let label = format!("{} – {}", monday.format("%B %-d"), sunday.format("%B %-d, %Y"));
            ui.label(RichText::new(label).font(crate::fonts::bold(16.0)));
            if ui.button("▶").on_hover_text("Next week").clicked() {
                self.meeting_week = monday + chrono::Duration::days(7);
            }
            if ui.button("This week").clicked() {
                self.meeting_week = crate::app::monday_of(chrono::Local::now().date_naive());
            }
        });
        ui.add_space(8.0);
        self.load_meetings(monday);
        let k = format!("{}|{}", self.settings.lang, monday);
        let mut open: Option<(String, String)> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            match self.meetings.get(&k) {
                Some(Loadable::Ready(secs)) if !secs.is_empty() => {
                    let secs = secs.clone();
                    for s in secs {
                        widgets::section(ui, dark, &s.title);
                        for it in &s.items {
                            let sym = it.symbol.clone();
                            let mut pic_rect = None;
                            let resp = widgets::click_card(ui, dark, 760.0, |ui| {
                                ui.horizontal(|ui| {
                                    let (r, _) = ui.allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover());
                                    pic_rect = Some(r);
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(&it.line1).font(crate::fonts::bold(16.0)).color(crate::theme::text(dark)));
                                        ui.label(RichText::new(&it.line2).color(crate::theme::muted(dark)));
                                    });
                                });
                            });
                            if let Some(r) = pic_rect {
                                let th = it.thumb.clone();
                                widgets::picture(self, ui, r, if th.is_empty() { None } else { Some(th.as_str()) }, &sym, &sym);
                            }
                            if resp.clicked() {
                                open = Some((it.href.clone(), it.line1.clone()));
                            }
                            ui.add_space(6.0);
                        }
                    }
                    // the publications for the week, from the catalog (downloadable for offline use)
                    if self.catalog.ready() {
                        let d = monday.format("%Y-%m-%d").to_string();
                        let pubs: Vec<crate::catalog::PubInfo> = [106i64, 68]
                            .iter()
                            .filter_map(|c| self.catalog.dated(*c, &d))
                            .collect();
                        if !pubs.is_empty() {
                            widgets::section(ui, dark, "Publications for this week");
                            if let Some(i) = widgets::tile_row(self, ui, "meetpubs", &pubs, 104.0) {
                                let p = pubs[i].clone();
                                self.tab = crate::app::Tab::Publications;
                                self.open_pub(&p);
                            }
                        }
                    }
                }
                Some(Loadable::Ready(_)) => {
                    widgets::empty_state(ui, dark, "👥", "No meeting material", "There is no meeting material for this week yet.");
                }
                Some(Loadable::Failed(e)) => {
                    let e = e.clone();
                    widgets::empty_state(ui, dark, "📡", "Unable to load", &format!("Meeting material is available when you are online. ({e})"));
                    if ui.button("Try again").clicked() {
                        self.meetings.remove(&k);
                    }
                }
                _ => {
                    ui.spinner();
                }
            }
        });
        if let Some((href, title)) = open {
            let url = format!("{}{}", crate::net::WOL, href);
            self.open_online(&url, &title);
        }
    }
}
