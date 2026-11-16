use crate::app::App;
use crate::userdata::Note;
use crate::widgets;
use egui::{RichText, Ui};

const TABS: [&str; 6] = ["Notes", "Tags", "Highlights", "Bookmarks", "Playlists", "Backup"];

fn note_card(app: &mut App, ui: &mut Ui, n: &Note) -> (bool, bool) {
    let dark = app.dark;
    let mut open_note = false;
    let mut open_loc = false;
    let resp = widgets::card(ui, dark, |ui| {
        ui.set_width(ui.available_width().min(820.0));
        let t = if n.title.is_empty() { "(Untitled note)".to_string() } else { n.title.clone() };
        ui.label(RichText::new(t).font(crate::fonts::bold(15.5)).color(crate::theme::text(dark)));
        if !n.content.is_empty() {
            let short: String = n.content.chars().take(300).collect();
            ui.label(RichText::new(short).color(crate::theme::text(dark)));
        }
        ui.horizontal_wrapped(|ui| {
            for t in &n.tags {
                ui.label(RichText::new(format!("#{t}")).size(12.5).color(crate::theme::accent(dark)));
            }
            if let Some(l) = &n.loc {
                let lt = if l.title.is_empty() { "Open location".to_string() } else { l.title.clone() };
                if ui.link(RichText::new(format!("📄 {lt}")).size(12.5)).clicked() {
                    open_loc = true;
                }
            }
            if let Some(d) = n.modified.get(..10) {
                ui.label(RichText::new(d).size(12.0).color(crate::theme::muted(dark)));
            }
        });
    });
    let _ = resp;
    if ui.small_button("Edit").clicked() {
        open_note = true;
    }
    ui.add_space(4.0);
    (open_note, open_loc)
}

impl App {
    pub fn ui_study(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        widgets::header(self, ui, "Personal Study", None);
        ui.horizontal(|ui| {
            for (i, t) in TABS.iter().enumerate() {
                let sel = self.study_tab == i;
                let txt = RichText::new(*t).font(if sel { crate::fonts::bold(14.5) } else { egui::FontId::proportional(14.5) });
                let b = egui::Button::new(if sel { txt.color(egui::Color32::WHITE) } else { txt })
                    .fill(if sel { crate::theme::PRIMARY } else { crate::theme::card(dark) })
                    .corner_radius(16.0);
                if ui.add(b).clicked() {
                    self.study_tab = i;
                    self.study_tag = None;
                    self.study_playlist = None;
                }
            }
        });
        ui.add_space(8.0);
        match self.study_tab {
            0 => self.study_notes(ui, None),
            1 => {
                if let Some(t) = self.study_tag.clone() {
                    ui.horizontal(|ui| {
                        if ui.button("⬅ All tags").clicked() {
                            self.study_tag = None;
                        }
                        ui.label(RichText::new(format!("#{t}")).font(crate::fonts::bold(18.0)));
                    });
                    self.study_notes(ui, Some(t));
                } else {
                    self.study_tags(ui);
                }
            }
            2 => self.study_highlights(ui),
            3 => self.study_bookmarks(ui),
            4 => self.study_playlists(ui),
            _ => self.study_backup(ui),
        }
    }

    fn study_notes(&mut self, ui: &mut Ui, tag: Option<String>) {
        let dark = self.dark;
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.study_filter).hint_text("Search notes").desired_width(280.0));
            if widgets::accent_button(ui, "➕ New note").clicked() {
                let tags = tag.clone().map(|t| vec![t]).unwrap_or_default();
                self.edit_note(Note { tags, ..Default::default() });
            }
        });
        ui.add_space(6.0);
        let notes = self.user.notes(&self.study_filter, tag.as_deref());
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if notes.is_empty() {
                widgets::empty_state(ui, dark, "📝", "No notes", "Select text while reading and choose Note, or create one here.");
            }
            for n in &notes {
                let (edit, go) = note_card(self, ui, n);
                if edit {
                    self.edit_note(n.clone());
                }
                if go {
                    if let Some(l) = n.loc.clone() {
                        let target = if l.key.starts_with("bible:") { crate::app::Tab::Bible } else { crate::app::Tab::Publications };
                        self.tab = target;
                        self.open_loc(&l, Some((n.btype, n.ident)));
                    }
                }
            }
        });
    }

    fn study_tags(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let tags = self.user.tags();
        ui.horizontal(|ui| {
            let id = egui::Id::new("newtag");
            let mut s: String = ui.ctx().data_mut(|d| d.get_temp(id).unwrap_or_default());
            ui.add(egui::TextEdit::singleline(&mut s).hint_text("New tag").desired_width(220.0));
            if ui.button("➕ Add tag").clicked() && !s.trim().is_empty() {
                self.user.tag_id(s.trim());
                s.clear();
            }
            ui.ctx().data_mut(|d| d.insert_temp(id, s));
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if tags.is_empty() {
                widgets::empty_state(ui, dark, "🏷", "No tags", "Tags group your notes by topic.");
            }
            for (name, count) in tags {
                ui.horizontal(|ui| {
                    if let Some((old, new)) = &mut self.rename_tag {
                        if *old == name {
                            ui.add(egui::TextEdit::singleline(new).desired_width(200.0));
                            if ui.button("Save").clicked() {
                                let (o, n) = (old.clone(), new.clone());
                                if !n.trim().is_empty() {
                                    self.user.rename_tag(&o, n.trim());
                                }
                                self.rename_tag = None;
                            }
                            return;
                        }
                    }
                    if ui.add(egui::Button::new(RichText::new(format!("🏷 {name}")).font(crate::fonts::bold(15.0))).frame(false)).clicked() {
                        self.study_tag = Some(name.clone());
                    }
                    ui.label(RichText::new(format!("{count} notes")).color(crate::theme::muted(dark)));
                    if ui.small_button("Rename").clicked() {
                        self.rename_tag = Some((name.clone(), name.clone()));
                    }
                    if ui.small_button("Delete").clicked() {
                        self.user.delete_tag(&name);
                    }
                });
                ui.separator();
            }
        });
    }

    fn study_highlights(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let marks = self.user.all_marks();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if marks.is_empty() {
                widgets::empty_state(ui, dark, "🖍", "No highlights", "Select text while reading and pick a colour.");
            }
            let mut go = None;
            for (l, m) in &marks {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 7.0, crate::theme::highlight(m.color, dark));
                    let t = if l.title.is_empty() { l.key.clone() } else { l.title.clone() };
                    let what = m.ranges.first().map(|r| if r.btype == 2 { format!("verse {}", r.ident) } else { format!("paragraph {}", r.ident) }).unwrap_or_default();
                    if ui.link(format!("{t}  ·  {what}")).clicked() {
                        go = Some((l.clone(), m.ranges.first().map(|r| (r.btype, r.ident))));
                    }
                });
            }
            if let Some((l, id)) = go {
                self.tab = if l.key.starts_with("bible:") { crate::app::Tab::Bible } else { crate::app::Tab::Publications };
                self.open_loc(&l, id);
            }
        });
    }

    fn study_bookmarks(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let bms = self.user.all_bookmarks();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if bms.is_empty() {
                widgets::empty_state(ui, dark, "🔖", "No bookmarks", "Use the bookmark button while reading. Each publication has 10 slots.");
            }
            let mut go = None;
            for b in &bms {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 22.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 1.0, crate::theme::bookmark(b.slot as usize, dark));
                    if ui.link(RichText::new(&b.title).strong()).clicked() {
                        go = Some(b.clone());
                    }
                    ui.label(RichText::new(b.snippet.chars().take(90).collect::<String>()).color(crate::theme::muted(dark)));
                    if ui.small_button("🗙").on_hover_text("Remove").clicked() {
                        self.user.remove_bookmark(&b.pub_key, b.slot);
                    }
                });
            }
            if let Some(b) = go {
                self.tab = if b.loc.key.starts_with("bible:") { crate::app::Tab::Bible } else { crate::app::Tab::Publications };
                self.open_loc(&b.loc, Some((b.btype, b.ident)));
            }
        });
    }

    fn study_playlists(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        let lists = self.user.playlists();
        if let Some(pid) = self.study_playlist {
            let Some(pl) = lists.iter().find(|p| p.id == pid).cloned() else {
                self.study_playlist = None;
                return;
            };
            ui.horizontal(|ui| {
                if ui.button("⬅ Playlists").clicked() {
                    self.study_playlist = None;
                }
                ui.label(RichText::new(&pl.name).font(crate::fonts::bold(18.0)));
                if !pl.items.is_empty() && widgets::accent_button(ui, "▶ Play all").clicked() {
                    let items: Vec<(String, String, String)> = pl.items.iter().map(|it| (it.title.clone(), it.url.clone(), it.image.clone())).collect();
                    self.play(items, false);
                }
                if ui.button("🗑 Delete playlist").clicked() {
                    self.user.delete_playlist(pl.id);
                    self.study_playlist = None;
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if pl.items.is_empty() {
                    widgets::empty_state(ui, dark, "🎵", "Empty playlist", "Add videos or audio from the Media tab.");
                }
                for it in &pl.items {
                    ui.horizontal(|ui| {
                        if ui.button("▶").clicked() {
                            self.play(vec![(it.title.clone(), it.url.clone(), it.image.clone())], it.url.ends_with(".mp3"));
                        }
                        ui.label(RichText::new(&it.title).strong());
                        ui.label(RichText::new(&it.duration).color(crate::theme::muted(dark)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("🗙").clicked() {
                                self.user.remove_playlist_item(it.id);
                            }
                            if ui.small_button("▼").clicked() {
                                self.user.move_playlist_item(pl.id, it.id, false);
                            }
                            if ui.small_button("▲").clicked() {
                                self.user.move_playlist_item(pl.id, it.id, true);
                            }
                        });
                    });
                    ui.separator();
                }
            });
            return;
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.new_playlist).hint_text("New playlist name").desired_width(240.0));
            if ui.button("➕ Create").clicked() && !self.new_playlist.trim().is_empty() {
                self.user.create_playlist(self.new_playlist.trim());
                self.new_playlist.clear();
            }
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if lists.is_empty() {
                widgets::empty_state(ui, dark, "🎵", "No playlists", "Create a playlist, then add media to it from the Media tab.");
            }
            for p in &lists {
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(RichText::new(format!("🎵 {}", p.name)).font(crate::fonts::bold(15.0))).frame(false)).clicked() {
                        self.study_playlist = Some(p.id);
                    }
                    ui.label(RichText::new(format!("{} items", p.items.len())).color(crate::theme::muted(dark)));
                });
                ui.separator();
            }
        });
    }

    fn study_backup(&mut self, ui: &mut Ui) {
        let dark = self.dark;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            widgets::card(ui, dark, |ui| {
                ui.set_width(ui.available_width().min(760.0));
                ui.label(RichText::new("Your study data").font(crate::fonts::bold(16.0)));
                ui.label(format!(
                    "{} notes · {} highlights · {} bookmarks · {} tags",
                    self.user.note_count(),
                    self.user.highlight_count(),
                    self.user.all_bookmarks().len(),
                    self.user.tags().len()
                ));
            });
            ui.add_space(10.0);
            widgets::card(ui, dark, |ui| {
                ui.set_width(ui.available_width().min(760.0));
                ui.label(RichText::new("Back up or restore").font(crate::fonts::bold(16.0)));
                ui.label(RichText::new("File location (a backup is written here, or read from here):").color(crate::theme::muted(dark)));
                ui.add(egui::TextEdit::singleline(&mut self.backup_path).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    if widgets::accent_button(ui, "💾 Create backup").clicked() {
                        let p = std::path::PathBuf::from(&self.backup_path);
                        let r = self.user.export_json(&p);
                        self.toast(r.unwrap_or_else(|e| format!("Backup failed: {e}")));
                    }
                    if ui.button("♻ Restore / import").on_hover_text("Reads a Study Library backup (.studylib) or a backup from the original app (.jwlibrary)").clicked() {
                        let p = std::path::PathBuf::from(&self.backup_path);
                        let r = if p.extension().map(|e| e == "jwlibrary").unwrap_or(false) {
                            self.user.import_jwlibrary(&p)
                        } else {
                            self.user.import_json(&p).or_else(|_| self.user.import_jwlibrary(&p))
                        };
                        self.toast(r.unwrap_or_else(|e| format!("Restore failed: {e}")));
                    }
                });
                ui.add_space(6.0);
                ui.label(RichText::new("Backups from the original app on Windows, Android or iPhone (files ending in .jwlibrary) can be imported here: notes, tags, highlights, bookmarks and favorites come across.").size(12.5).color(crate::theme::muted(dark)));
                // offer backup files found in the usual folders
                let mut found = vec![];
                for d in [dirs::download_dir(), dirs::document_dir(), dirs::home_dir(), dirs::desktop_dir()].into_iter().flatten() {
                    if let Ok(rd) = std::fs::read_dir(&d) {
                        for e in rd.flatten() {
                            let p = e.path();
                            if p.extension().map(|x| x == "jwlibrary" || x == "studylib").unwrap_or(false) {
                                found.push(p);
                            }
                        }
                    }
                }
                if !found.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Backup files found on this computer:").strong());
                    for p in found.iter().take(12) {
                        if ui.link(p.to_string_lossy()).clicked() {
                            self.backup_path = p.to_string_lossy().into_owned();
                        }
                    }
                }
            });
        });
    }
}

pub fn note_editor_window(app: &mut App, ctx: &egui::Context) {
    let Some(ed) = &mut app.note_editor else { return };
    let mut open = true;
    let mut save = false;
    let mut delete = false;
    let mut cancel = false;
    let all_tags: Vec<String> = app.user.tags().into_iter().map(|t| t.0).collect();
    let dark = app.dark;
    egui::Window::new(if ed.note.id > 0 { "Edit Note" } else { "New Note" })
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            if let Some(l) = &ed.note.loc {
                ui.label(RichText::new(format!("📄 {}", l.title)).color(crate::theme::muted(dark)));
            }
            ui.add(egui::TextEdit::singleline(&mut ed.note.title).hint_text("Title").font(crate::fonts::bold(17.0)).desired_width(f32::INFINITY));
            ui.add(egui::TextEdit::multiline(&mut ed.note.content).hint_text("Note").desired_rows(10).desired_width(f32::INFINITY));
            ui.horizontal_wrapped(|ui| {
                ui.label("Tags:");
                let mut remove = None;
                for (i, t) in ed.note.tags.iter().enumerate() {
                    if ui.add(egui::Button::new(format!("#{t}  🗙")).corner_radius(12.0)).clicked() {
                        remove = Some(i);
                    }
                }
                if let Some(i) = remove {
                    ed.note.tags.remove(i);
                }
                let r = ui.add(egui::TextEdit::singleline(&mut ed.tag_input).hint_text("Add tag").desired_width(130.0));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !ed.tag_input.trim().is_empty() {
                    let t = ed.tag_input.trim().to_string();
                    if !ed.note.tags.contains(&t) {
                        ed.note.tags.push(t);
                    }
                    ed.tag_input.clear();
                }
            });
            let q = ed.tag_input.to_lowercase();
            let sugg: Vec<&String> = all_tags.iter().filter(|t| !ed.note.tags.contains(t) && (q.is_empty() || t.to_lowercase().contains(&q))).take(8).collect();
            if !sugg.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for t in sugg {
                        if ui.small_button(format!("+ {t}")).clicked() {
                            ed.note.tags.push(t.clone());
                            ed.tag_input.clear();
                        }
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if widgets::accent_button(ui, "Save").clicked() {
                    save = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                if ed.note.id > 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ed.confirm_delete {
                            if ui.button(RichText::new("Really delete").color(egui::Color32::from_rgb(0xc0, 0x30, 0x30))).clicked() {
                                delete = true;
                            }
                        } else if ui.button("🗑 Delete").clicked() {
                            ed.confirm_delete = true;
                        }
                    });
                }
            });
        });
    if save {
        let n = ed.note.clone();
        if !n.title.trim().is_empty() || !n.content.trim().is_empty() {
            app.user.save_note(&n);
            app.toast("Note saved");
        }
        app.note_editor = None;
    } else if delete {
        app.user.delete_note(ed.note.id);
        app.note_editor = None;
        app.toast("Note deleted");
    } else if cancel || !open {
        app.note_editor = None;
    }
    if app.note_editor.is_none() {
        // refresh any open reader
        for stack in app.stacks.values_mut() {
            for p in stack.iter_mut() {
                if let crate::app::Page::Reader(rd) = p {
                    rd.refresh(&app.user);
                }
            }
        }
    }
}
