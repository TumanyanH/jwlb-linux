//! The reading view: Bible chapters, publication pages and online-library articles, with
//! text-range highlighting in six colours, notes, ten bookmark slots and find-in-page.

use crate::app::App;
use crate::catalog::PubInfo;
use crate::content::{tokens, Block, Doc, Kind};
use crate::wol::{BibleRef, BookIdx};
use crate::userdata::{Bookmark, Loc, Mark, Note, Range, UserData};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, Rect, RichText, Ui};
use std::sync::Arc;

pub enum Source {
    Epub { p: PubInfo, href: String },
    Bible { b: BibleRef, idx: Vec<BookIdx>, book: u8, chapter: u16 },
    Online { url: String, lang: String },
    Static,
}

type Pt = (usize, usize);

pub struct Reader {
    pub source: Source,
    pub title: String,
    pub doc: Option<Doc>,
    pub error: Option<String>,
    pub request: u64,
    pub loc: Loc,
    pub pub_key: String,
    pub marks: Vec<Mark>,
    pub notes: Vec<Note>,
    pub bookmarks: Vec<Option<Bookmark>>,
    anchor: Option<Pt>,
    dragging: bool,
    pub sel: Option<(Pt, Pt)>,
    layout: Vec<(Rect, Option<Arc<egui::Galley>>)>,
    pub scroll_to_ident: Option<(i64, i64)>,
    pub scroll_to_anchor: Option<String>,
    show_bookmarks: bool,
    bookmark_ident: Option<(i64, i64, String)>,
    find: String,
    show_find: bool,
    pending_link: Option<String>,
    /// translation popup: where it rises from, and the result when it arrives
    translation: Option<(egui::Pos2, String, std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>)>,
    translation_wide: bool,
}

impl Reader {
    fn base(source: Source, title: String) -> Reader {
        Reader {
            source,
            title,
            doc: None,
            error: None,
            request: 0,
            loc: Loc::default(),
            pub_key: String::new(),
            marks: vec![],
            notes: vec![],
            bookmarks: (0..10).map(|_| None).collect(),
            anchor: None,
            dragging: false,
            sel: None,
            layout: vec![],
            scroll_to_ident: None,
            scroll_to_anchor: None,
            show_bookmarks: false,
            bookmark_ident: None,
            find: String::new(),
            show_find: false,
            pending_link: None,
            translation: None,
            translation_wide: false,
        }
    }

    pub fn epub(p: PubInfo, href: &str, user: &UserData) -> Result<Reader, String> {
        let mut e = crate::epub::Epub::open(&p.file_path())?;
        let (file, frag) = match href.split_once('#') {
            Some((f, a)) => (f.to_string(), Some(a.to_string())),
            None => (href.to_string(), None),
        };
        let text = e.read_text(&file).ok_or("This page is missing from the download")?;
        let doc = crate::content::parse_epub_page(&file, &text, true);
        let title = e
            .toc_title_for(&file)
            .or_else(|| if doc.title.is_empty() { None } else { Some(doc.title.clone()) })
            .unwrap_or_else(|| p.display_title());
        let pubref = serde_json::to_string(&p).unwrap_or_default();
        let loc = match doc.doc_id {
            Some(d) => Loc { key: format!("doc:{}:{}", p.meps, d), title: title.clone(), pubref, href: file.clone(), book: 0, chapter: 0, doc_id: d },
            None => Loc { key: format!("page:{}:{}", p.key(), file), title: title.clone(), pubref, href: file.clone(), book: 0, chapter: 0, doc_id: 0 },
        };
        let mut r = Reader::base(Source::Epub { p: p.clone(), href: file }, title);
        r.pub_key = p.key();
        r.loc = loc;
        r.doc = Some(doc);
        r.scroll_to_anchor = frag;
        r.refresh(user);
        user.visit(&r.loc);
        Ok(r)
    }

    /// A Bible chapter; its text arrives through the online-library loader (`request`).
    pub fn bible(b: BibleRef, idx: Vec<BookIdx>, book: u8, chapter: u16, verse: Option<u16>, request: u64) -> Reader {
        let mut r = Reader::base(Source::Bible { b, idx, book, chapter }, String::new());
        r.request = request;
        r.set_bible_title();
        if let Some(v) = verse {
            r.scroll_to_ident = Some((2, v as i64));
        }
        r
    }

    fn set_bible_title(&mut self) {
        if let Source::Bible { idx, book, chapter, .. } = &self.source {
            let name = idx.iter().find(|x| x.num == *book).map(|x| x.name.clone()).unwrap_or_default();
            self.title = format!("{name} {chapter}");
        }
    }

    /// A document that is already in memory (the daily text).
    pub fn from_doc(title: &str, doc: Doc, loc: Loc, user: &UserData) -> Reader {
        let mut r = Reader::base(Source::Static, title.to_string());
        r.pub_key = loc.key.clone();
        r.loc = loc;
        r.doc = Some(doc);
        r.refresh(user);
        r
    }

    pub fn online(url: &str, title: &str, request: u64, lang: &str) -> Reader {
        let mut r = Reader::base(Source::Online { url: url.to_string(), lang: lang.to_string() }, title.to_string());
        r.request = request;
        r
    }

    pub fn receive_online(&mut self, r: Result<Doc, String>, user: &UserData) {
        if let Source::Bible { b, book, chapter, .. } = &self.source {
            match r {
                Ok(doc) => {
                    let meps = crate::lang::meps_id(&b.lang);
                    let p = PubInfo::simple(&b.symbol, 0, &b.lang, &b.title, 1);
                    let pubref = serde_json::to_string(&p).unwrap_or_default();
                    self.loc = Loc::bible(meps, *book as i64, *chapter as i64, &self.title, &pubref);
                    self.pub_key = format!("bible|{}", b.lang);
                    self.doc = Some(doc);
                    self.error = None;
                    self.refresh(user);
                    user.visit(&self.loc);
                }
                Err(e) => self.error = Some(format!("This chapter isn't on this computer yet and could not be loaded. Connect to the internet, or download the Bible on the Bible tab. ({e})")),
            }
            return;
        }
        let Source::Online { url, lang } = &self.source else { return };
        match r {
            Ok(doc) => {
                if self.title.is_empty() || self.title.len() < 3 {
                    self.title = doc.title.clone();
                }
                let meps = crate::lang::meps_id(lang);
                let sym = doc.pub_symbol.clone().unwrap_or_default();
                let p = PubInfo::simple(&sym, 0, lang, &self.title, 0);
                let pubref = serde_json::to_string(&p).unwrap_or_default();
                self.loc = match doc.doc_id {
                    Some(d) => Loc { key: format!("doc:{meps}:{d}"), title: self.title.clone(), pubref, href: url.clone(), book: 0, chapter: 0, doc_id: d },
                    None => Loc { key: format!("url:{url}"), title: self.title.clone(), pubref, href: url.clone(), book: 0, chapter: 0, doc_id: 0 },
                };
                self.pub_key = format!("{}|0|{}", sym, lang);
                self.doc = Some(doc);
                self.refresh(user);
                user.visit(&self.loc);
            }
            Err(e) => self.error = Some(format!("Could not load this page. Check your internet connection. ({e})")),
        }
    }

    pub fn refresh(&mut self, user: &UserData) {
        if self.loc.key.is_empty() {
            return;
        }
        self.marks = user.marks_for(&self.loc.key);
        self.notes = user.notes_for(&self.loc.key);
        self.bookmarks = user.bookmarks(&self.pub_key);
    }

    fn selection_ranges(&self) -> (Vec<Range>, String) {
        let Some(doc) = &self.doc else { return (vec![], String::new()) };
        let Some((a, b)) = self.sel else { return (vec![], String::new()) };
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let mut out = vec![];
        let mut text = String::new();
        for bi in a.0..=b.0.min(doc.blocks.len().saturating_sub(1)) {
            let blk = &doc.blocks[bi];
            let s = if bi == a.0 { a.1 } else { 0 };
            let e = if bi == b.0 { b.1 } else { blk.text.len() };
            if s >= e {
                continue;
            }
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&blk.text[s..e]);
            for seg in &blk.segs {
                if seg.end <= s || seg.start >= e {
                    continue;
                }
                let toks = tokens(&blk.text, seg.start, seg.end);
                let first = toks.iter().position(|t| t.1 > s);
                let last = toks.iter().rposition(|t| t.0 < e);
                if let (Some(i), Some(j)) = (first, last) {
                    if i <= j {
                        out.push(Range { btype: seg.btype, ident: seg.ident, start: i as i64, end: j as i64 });
                    }
                }
            }
        }
        (out, text)
    }

    fn nav(&mut self, app: &mut App, dir: i32) {
        match &mut self.source {
            Source::Bible { b, idx, book, chapter } => {
                let bi = idx.iter().position(|x| x.num == *book).unwrap_or(0);
                let n = crate::wol::chapter_count(idx[bi].num) as i32;
                let c = *chapter as i32 + dir;
                if c >= 1 && c <= n {
                    *chapter = c as u16;
                } else if dir > 0 && bi + 1 < idx.len() {
                    *book = idx[bi + 1].num;
                    *chapter = 1;
                } else if dir < 0 && bi > 0 {
                    *book = idx[bi - 1].num;
                    *chapter = crate::wol::chapter_count(idx[bi - 1].num);
                } else {
                    return;
                }
                let Ok(w) = crate::net::wol_locale(&b.lang) else { return };
                let url = crate::wol::chapter_url(&w, b, *book, *chapter);
                self.request = app.load_online(url);
                self.doc = None;
                self.error = None;
                self.sel = None;
                self.set_bible_title();
                self.scroll_to_anchor = Some("__top".into());
            }
            Source::Epub { p, href } => {
                let Ok(e) = crate::epub::Epub::open(&p.file_path()) else { return };
                let spine: Vec<String> = e.spine.iter().filter(|s| !s.contains("nav") && !s.contains("cover")).cloned().collect();
                let Some(i) = spine.iter().position(|s| s == href) else { return };
                let j = i as i32 + dir;
                if j < 0 || j as usize >= spine.len() {
                    return;
                }
                let p = p.clone();
                if let Ok(r) = Reader::epub(p, &spine[j as usize], &app.user) {
                    *self = r;
                    self.scroll_to_anchor = Some("__top".into());
                }
            }
            Source::Online { .. } | Source::Static => {}
        }
    }

    fn follow_link(&mut self, app: &mut App, link: &str) {
        if link.starts_with("http") {
            crate::paths::open_external(link);
            return;
        }
        if link.starts_with("/wol/") || link.starts_with("/en/wol/") || link.contains("/wol/") {
            let url = format!("{}{}", crate::net::WOL, link);
            app.open_online(&url, "");
            return;
        }
        if let Source::Epub { p, href } = &self.source {
            if let Some(frag) = link.strip_prefix('#') {
                self.scroll_to_anchor = Some(frag.to_string());
                return;
            }
            let target = crate::epub::resolve(href, link);
            let frag = link.split('#').nth(1);
            let full = match frag {
                Some(f) => format!("{target}#{f}"),
                None => target,
            };
            let p = p.clone();
            app.open_doc(&p, &full);
        }
        if let Source::Bible { .. } = &self.source {
            if let Some(frag) = link.strip_prefix('#') {
                self.scroll_to_anchor = Some(frag.to_string());
            }
        }
    }
}

/// Heading sizes follow the original's du-fontSize--basePlusN steps (×1.205 per step,
/// PrimaryPane.bundle.js): +3 = 1.7497, +2 = 1.452025, +1 = 1.205.
/// body{line-height:1.5} in the original reader.
const LINE_HEIGHT: f32 = 1.5;

fn kind_scale(k: &Kind) -> f32 {
    match k {
        Kind::H1 => 1.749_690,
        Kind::H2 => 1.452_025,
        Kind::H3 => 1.205,
        Kind::Question | Kind::Caption => 0.88,
        Kind::Footnote => 0.8,
        _ => 1.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn block_job(b: &Block, size: f32, dark: bool, hl: &[(usize, usize, Color32)], sel: Option<(usize, usize)>, finds: &[(usize, usize)], wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let base = size * kind_scale(&b.kind);
    let heading = matches!(b.kind, Kind::H1 | Kind::H2 | Kind::H3);
    let muted = matches!(b.kind, Kind::Question | Kind::Caption | Kind::Footnote | Kind::Theme);
    let text_col = if muted { crate::theme::muted(dark) } else { crate::theme::text(dark) };
    let link_col = crate::theme::link(dark);
    let sel_col = if dark { Color32::from_rgb(0x3d, 0x5a, 0x8a) } else { Color32::from_rgb(0xc5, 0xd6, 0xf2) };
    let find_col = Color32::from_rgb(0xff, 0x9f, 0x43);
    let mut pts: Vec<usize> = vec![0, b.text.len()];
    for s in &b.spans {
        pts.push(s.start);
        pts.push(s.end);
    }
    for h in hl {
        pts.push(h.0);
        pts.push(h.1);
    }
    if let Some((s, e)) = sel {
        pts.push(s);
        pts.push(e);
    }
    for f in finds {
        pts.push(f.0);
        pts.push(f.1);
    }
    pts.retain(|p| *p <= b.text.len() && b.text.is_char_boundary(*p));
    pts.sort_unstable();
    pts.dedup();
    let mut si = 0;
    for w in pts.windows(2) {
        let (a, e) = (w[0], w[1]);
        if a >= e {
            continue;
        }
        while si + 1 < b.spans.len() && b.spans[si].end <= a {
            si += 1;
        }
        let span = b.spans.get(si).filter(|s| s.start <= a && s.end >= e);
        let bold = heading || span.map(|s| s.bold).unwrap_or(false);
        let italic = matches!(b.kind, Kind::Theme) ^ span.map(|s| s.italic).unwrap_or(false);
        let sup = span.map(|s| s.sup).unwrap_or(false);
        let link = span.and_then(|s| s.link.as_ref()).is_some();
        let fsize = if sup { base * 0.62 } else { base };
        let font = if bold { crate::fonts::bold(fsize) } else { egui::FontId::proportional(fsize) };
        let mut bg = Color32::TRANSPARENT;
        for h in hl {
            if h.0 <= a && h.1 >= e {
                bg = h.2;
            }
        }
        for f in finds {
            if f.0 <= a && f.1 >= e {
                bg = find_col;
            }
        }
        if let Some((s, en)) = sel {
            if s <= a && en >= e {
                bg = sel_col;
            }
        }
        let color = if link && !sup { link_col } else if sup && !heading { link_col } else { text_col };
        let fg = if bg != Color32::TRANSPARENT && dark && bg != sel_col { Color32::from_rgb(20, 20, 20) } else { color };
        job.append(
            &b.text[a..e],
            0.0,
            TextFormat {
                font_id: font,
                color: fg,
                background: bg,
                italics: italic,
                valign: if sup { egui::Align::TOP } else { egui::Align::BOTTOM },
                line_height: Some(if sup { base * LINE_HEIGHT } else { fsize * LINE_HEIGHT }),
                ..Default::default()
            },
        );
    }
    job
}

fn char_to_byte(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

pub fn ui(app: &mut App, ui: &mut Ui, rd: &mut Reader, can_back: bool) {
    let dark = app.dark;
    let size = app.settings.text_size();
    // ---------- title (the buttons are in the header bar, see header_actions) ----------
    let _ = can_back;
    app.page_title = match &rd.source {
        Source::Epub { p, .. } => p.display_title(),
        Source::Bible { b, .. } => b.title.clone(),
        Source::Online { .. } => "Online Library".into(),
        Source::Static => rd.title.clone(),
    };
    ui.label(RichText::new(&rd.title).font(crate::fonts::bold(20.0)).color(crate::theme::text(dark)));
    if rd.show_find {
        ui.horizontal(|ui| {
            ui.label("Find:");
            let r = ui.add(egui::TextEdit::singleline(&mut rd.find).desired_width(260.0).hint_text("Word or phrase"));
            r.request_focus();
            if ui.button("🗙").clicked() {
                rd.show_find = false;
                rd.find.clear();
            }
        });
    }
    ui.separator();

    if let Some(e) = &rd.error {
        crate::widgets::empty_state(ui, dark, "⚠", "Unable to open", e);
        return;
    }
    if rd.doc.is_none() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.spinner();
            ui.label("Loading…");
        });
        return;
    }

    // ---------- text ----------
    let find_l = rd.find.to_lowercase();
    let mut clicked_note: Option<Note> = None;
    let mut add_note_for: Option<(i64, i64)> = None;
    let layer = ui.layer_id();
    let epub_path = match &rd.source {
        Source::Epub { p, .. } => Some(p.file_path().to_string_lossy().into_owned()),
        _ => None,
    };
    let scroll = egui::ScrollArea::vertical()
        .id_salt(("reader", rd.loc.key.clone(), rd.title.clone()))
        .auto_shrink([false, false])
        .drag_to_scroll(false);
    let top_req = rd.scroll_to_anchor.as_deref() == Some("__top");
    let scroll = if top_req { scroll.vertical_scroll_offset(0.0) } else { scroll };
    if top_req {
        rd.scroll_to_anchor = None;
    }
    scroll.show(ui, |ui| {
        let avail = ui.available_width();
        let w = avail.min(840.0) - 40.0;
        let pad = ((avail - w) / 2.0).max(20.0);
        rd.layout.clear();
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.vertical(|ui| {
                ui.set_width(w);
                let doc = rd.doc.as_ref().unwrap();
                let nblocks = doc.blocks.len();
                for bi in 0..nblocks {
                    let b = &rd.doc.as_ref().unwrap().blocks[bi];
                    if matches!(b.kind, Kind::Footnote) && !app.settings.show_footnotes {
                        rd.layout.push((Rect::NOTHING, None));
                        continue;
                    }
                    if let Kind::Image(src) = &b.kind {
                        let key = if src.starts_with("http") {
                            src.clone()
                        } else if let Some(ep) = &epub_path {
                            format!("epub:{ep}::{src}")
                        } else {
                            String::new()
                        };
                        if key.is_empty() {
                            rd.layout.push((Rect::NOTHING, None));
                            continue;
                        }
                        let tex = app.image(&key);
                        match tex {
                            Some(t) => {
                                let [iw, ih] = t.size();
                                let dw = (iw as f32).min(w).max(1.0);
                                let dh = dw * ih as f32 / iw.max(1) as f32;
                                let (r, _) = ui.allocate_exact_size(egui::vec2(w, dh), egui::Sense::hover());
                                let ir = Rect::from_min_size(egui::pos2(r.center().x - dw / 2.0, r.top()), egui::vec2(dw, dh));
                                ui.painter().image(t.id(), ir, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                            }
                            None => {
                                if matches!(app.images.get(&key), Some(crate::app::ImgState::Failed)) {
                                    rd.layout.push((Rect::NOTHING, None));
                                    continue;
                                }
                                let (r, _) = ui.allocate_exact_size(egui::vec2(w, 120.0), egui::Sense::hover());
                                ui.painter().rect_filled(r, 6.0, crate::theme::card(dark));
                            }
                        }
                        ui.add_space(10.0);
                        rd.layout.push((Rect::NOTHING, None));
                        continue;
                    }
                    // highlights for this block
                    let mut hl = vec![];
                    for m in &rd.marks {
                        for r in &m.ranges {
                            for seg in b.segs.iter().filter(|s| s.btype == r.btype && s.ident == r.ident) {
                                let toks = tokens(&b.text, seg.start, seg.end);
                                if toks.is_empty() {
                                    continue;
                                }
                                let s = (r.start.max(0) as usize).min(toks.len() - 1);
                                let e = (r.end.max(0) as usize).min(toks.len() - 1);
                                if s <= e {
                                    hl.push((toks[s].0, toks[e].1, crate::theme::highlight(m.color, dark)));
                                }
                            }
                        }
                    }
                    let sel = rd.sel.and_then(|(a, c)| {
                        let (a, c) = if a <= c { (a, c) } else { (c, a) };
                        if bi < a.0 || bi > c.0 {
                            return None;
                        }
                        let s = if bi == a.0 { a.1 } else { 0 };
                        let e = if bi == c.0 { c.1 } else { b.text.len() };
                        (s < e).then_some((s, e))
                    });
                    let mut finds = vec![];
                    if find_l.len() >= 2 {
                        let lower = b.text.to_lowercase();
                        if lower.len() == b.text.len() {
                            let mut from = 0;
                            while let Some(p) = lower[from..].find(&find_l) {
                                finds.push((from + p, from + p + find_l.len()));
                                from += p + find_l.len();
                            }
                        }
                    }
                    let indent = if matches!(b.kind, Kind::Item) { 22.0 } else { 0.0 };
                    let job = block_job(b, size, dark, &hl, sel, &finds, w - indent);
                    let galley = ui.fonts(|f| f.layout_job(job));
                    if matches!(b.kind, Kind::H1 | Kind::H2) {
                        ui.add_space(size * 0.4);
                    }
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, galley.size().y), egui::Sense::hover());
                    let text_pos = rect.min + egui::vec2(indent, 0.0);
                    if indent > 0.0 {
                        ui.painter().text(rect.min + egui::vec2(6.0, size * 0.75), egui::Align2::LEFT_CENTER, "•", egui::FontId::proportional(size), crate::theme::text(dark));
                    }
                    ui.painter().galley(text_pos, galley.clone(), crate::theme::text(dark));
                    // margin markers: bookmarks and notes
                    for (slot, bm) in rd.bookmarks.iter().enumerate() {
                        if let Some(bm) = bm {
                            if bm.loc.key == rd.loc.key && b.segs.iter().any(|s| s.btype == bm.btype && s.ident == bm.ident) {
                                let r = Rect::from_min_size(egui::pos2(rect.left() - 18.0, rect.top() + 2.0), egui::vec2(9.0, 18.0));
                                ui.painter().rect_filled(r, 1.0, crate::theme::bookmark(slot, dark));
                            }
                        }
                    }
                    let block_notes: Vec<Note> = rd
                        .notes
                        .iter()
                        .filter(|n| n.btype != 0 && b.segs.iter().any(|s| s.btype == n.btype && s.ident == n.ident))
                        .cloned()
                        .collect();
                    rd.layout.push((Rect::from_min_size(text_pos, galley.size()), Some(galley)));
                    // scroll targets
                    if let Some((bt, id)) = rd.scroll_to_ident {
                        if b.segs.iter().any(|s| s.btype == bt && s.ident == id) {
                            ui.scroll_to_rect(rect, Some(egui::Align::Center));
                            rd.scroll_to_ident = None;
                        }
                    }
                    if let Some(a) = &rd.scroll_to_anchor {
                        if b.anchor.as_deref() == Some(a.as_str()) {
                            ui.scroll_to_rect(rect, Some(egui::Align::TOP));
                            rd.scroll_to_anchor = None;
                        }
                    }
                    for n in block_notes {
                        ui.add_space(4.0);
                        let col = if n.color > 0 { crate::theme::highlight(n.color, dark) } else { crate::theme::PRIMARY_LIGHT };
                        let resp = egui::Frame::default()
                            .fill(crate::theme::card(dark))
                            .corner_radius(6.0)
                            .inner_margin(egui::Margin { left: 12, right: 10, top: 6, bottom: 6 })
                            .show(ui, |ui| {
                                ui.set_width(w - 24.0);
                                let t = if n.title.is_empty() { "Note".to_string() } else { n.title.clone() };
                                ui.label(RichText::new(format!("📝 {t}")).font(crate::fonts::bold(13.5)).color(crate::theme::text(dark)));
                                if !n.content.is_empty() {
                                    let short: String = n.content.chars().take(220).collect();
                                    ui.label(RichText::new(short).size(13.0).color(crate::theme::muted(dark)));
                                }
                                if !n.tags.is_empty() {
                                    ui.label(RichText::new(n.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join("  ")).size(12.0).color(crate::theme::accent(dark)));
                                }
                            })
                            .response;
                        ui.painter().rect_filled(Rect::from_min_size(resp.rect.min, egui::vec2(4.0, resp.rect.height())), 2.0, col);
                        if resp.interact(egui::Sense::click()).clicked() {
                            clicked_note = Some(n.clone());
                        }
                    }
                    let gap = match b.kind {
                        Kind::H1 | Kind::H2 => size * 0.5,
                        Kind::Footnote => size * 0.25,
                        _ => size * 0.65,
                    };
                    ui.add_space(gap);
                }
                // page-level notes
                let page_notes: Vec<Note> = rd.notes.iter().filter(|n| n.btype == 0).cloned().collect();
                if !page_notes.is_empty() {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.label(RichText::new("Notes on this page").font(crate::fonts::bold(15.0)));
                    for n in page_notes {
                        let t = if n.title.is_empty() { "Note".to_string() } else { n.title.clone() };
                        if ui.link(format!("📝 {t}")).clicked() {
                            clicked_note = Some(n.clone());
                        }
                    }
                }
                ui.add_space(80.0);
            });
        });
    });

    // ---------- pointer: select text, click links ----------
    let (pressed, down, released, pos) = ui.input(|i| {
        (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released(), i.pointer.interact_pos())
    });
    let over_reader = pos.map(|p| ui.ctx().layer_id_at(p) == Some(layer) && ui.clip_rect().contains(p)).unwrap_or(false);
    let hit = |p: egui::Pos2, nearest: bool| -> Option<Pt> {
        let doc = rd.doc.as_ref()?;
        let mut best: Option<(f32, usize)> = None;
        for (i, (r, g)) in rd.layout.iter().enumerate() {
            if g.is_none() || !r.is_positive() {
                continue;
            }
            if r.expand2(egui::vec2(30.0, 3.0)).contains(p) {
                best = Some((0.0, i));
                break;
            }
            if nearest {
                let d = if p.y < r.top() { r.top() - p.y } else if p.y > r.bottom() { p.y - r.bottom() } else { 0.0 };
                if best.map(|b| d < b.0).unwrap_or(true) {
                    best = Some((d, i));
                }
            }
        }
        let (_, i) = best?;
        let (r, g) = &rd.layout[i];
        let g = g.as_ref()?;
        let rel = p - r.min;
        let rel = egui::vec2(rel.x.clamp(-1.0, r.width() + 1.0), rel.y.clamp(0.0, r.height() - 1.0));
        let c = g.cursor_from_pos(rel);
        Some((i, char_to_byte(&doc.blocks[i].text, c.ccursor.index)))
    };
    if pressed && over_reader {
        if let Some(p) = pos {
            rd.anchor = hit(p, false);
            rd.dragging = rd.anchor.is_some();
            if rd.anchor.is_some() {
                rd.sel = None;
            }
        }
    }
    if rd.dragging && down {
        if let (Some(a), Some(p)) = (rd.anchor, pos) {
            if let Some(c) = hit(p, true) {
                if c != a && (c.0 != a.0 || c.1.abs_diff(a.1) > 0) {
                    rd.sel = Some((a, c));
                }
            }
        }
    }
    if released && rd.dragging {
        rd.dragging = false;
        if rd.sel.is_none() {
            // a click: follow a link, or select the verse/paragraph under the pointer
            if let (Some((bi, byte)), Some(doc)) = (rd.anchor, rd.doc.as_ref()) {
                let b = &doc.blocks[bi];
                let link = b.spans.iter().find(|s| s.start <= byte && s.end > byte).and_then(|s| s.link.clone());
                if let Some(l) = link.filter(|l| !l.is_empty()) {
                    rd.pending_link = Some(l);
                } else if let Some(seg) = b.segs.iter().find(|s| s.start <= byte && s.end >= byte) {
                    rd.sel = Some(((bi, seg.start), (bi, seg.end)));
                }
            }
        } else if let Some((a, c)) = rd.sel {
            // snap to whole words
            let doc = rd.doc.as_ref().unwrap();
            let (a, c) = if a <= c { (a, c) } else { (c, a) };
            let snap_start = |bi: usize, byte: usize| {
                let t = &doc.blocks[bi].text;
                let mut s = byte.min(t.len());
                while s > 0 && !t[..s].ends_with(char::is_whitespace) {
                    s -= 1;
                    while !t.is_char_boundary(s) {
                        s -= 1;
                    }
                }
                s
            };
            let snap_end = |bi: usize, byte: usize| {
                let t = &doc.blocks[bi].text;
                let mut e = byte.min(t.len());
                while e < t.len() && !t[e..].starts_with(char::is_whitespace) {
                    e += 1;
                    while !t.is_char_boundary(e) {
                        e += 1;
                    }
                }
                e
            };
            rd.sel = Some(((a.0, snap_start(a.0, a.1)), (c.0, snap_end(c.0, c.1))));
        }
    }
    if let Some(l) = rd.pending_link.take() {
        rd.follow_link(app, &l);
    }
    if let Some(n) = clicked_note {
        app.edit_note(n);
    }

    // ---------- selection actions ----------
    if rd.sel.is_some() {
        let (ranges, text) = rd.selection_ranges();
        egui::Area::new(egui::Id::new("sel_bar"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(100.0, -26.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::default()
                    .fill(if dark { crate::theme::hex(0x333333) } else { Color32::WHITE })
                    .stroke(egui::Stroke::new(1.0_f32, Color32::from_black_alpha(40)))
                    .corner_radius(24.0)
                    .shadow(egui::Shadow { offset: [0, 3], blur: 12, spread: 0, color: Color32::from_black_alpha(50) })
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let can_mark = !ranges.is_empty();
                            for ci in crate::theme::HIGHLIGHT_PICKER_ORDER {
                                let (r, resp) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
                                let c = crate::theme::highlight(ci, dark);
                                ui.painter().circle_filled(r.center(), if resp.hovered() { 12.0 } else { 10.0 }, if can_mark { c } else { c.linear_multiply(0.3) });
                                if ci == app.settings.default_highlight + 1 {
                                    ui.painter().circle_stroke(r.center(), 12.5, egui::Stroke::new(2.0_f32, crate::theme::text(dark)));
                                }
                                let resp = resp.on_hover_text(format!("Highlight {}", crate::theme::HIGHLIGHT_NAMES[(ci - 1) as usize]));
                                if resp.clicked() && can_mark {
                                    app.settings.default_highlight = ci - 1;
                                    app.settings.save();
                                    app.user.add_mark(&rd.loc, ci, &ranges);
                                    rd.sel = None;
                                    rd.refresh(&app.user);
                                }
                            }
                            if ui.button("⌫").on_hover_text("Remove highlight").clicked() {
                                app.user.erase(&rd.loc, &ranges);
                                rd.sel = None;
                                rd.refresh(&app.user);
                            }
                            ui.separator();
                            if ui.button("📝 Note").clicked() {
                                if let Some(r) = ranges.first() {
                                    add_note_for = Some((r.btype, r.ident));
                                }
                            }
                            if ui.button("📋 Copy").clicked() {
                                let mut t = text.clone();
                                if let Source::Bible { .. } = rd.source {
                                    t = format!("{t}\n— {}", rd.title);
                                }
                                ui.ctx().copy_text(t);
                                app.toast("Copied");
                                rd.sel = None;
                            }
                            if ui.button("🔖 Bookmark").clicked() {
                                if let Some(r) = ranges.first() {
                                    let snippet: String = text.chars().take(120).collect();
                                    rd.bookmark_ident = Some((r.btype, r.ident, snippet));
                                    rd.show_bookmarks = true;
                                }
                            }
                            let tr = ui.button("🌐 Translate").on_hover_text(format!("Translate into {}", crate::translate::name(&app.settings.translate_to)));
                            if tr.clicked() {
                                let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
                                let (s2, t, to) = (slot.clone(), text.clone(), app.settings.translate_to.clone());
                                let from = crate::lang::by_symbol(&app.settings.lang).map(|l| l.ietf.split('-').next().unwrap_or("").to_string()).unwrap_or_default();
                                let ctx = ui.ctx().clone();
                                std::thread::spawn(move || {
                                    let r = crate::translate::translate(&t, &to, &from);
                                    *s2.lock().unwrap() = Some(r);
                                    ctx.request_repaint();
                                });
                                rd.translation = Some((tr.rect.center_top(), app.settings.translate_to.clone(), slot));
                            }
                            if ui.button("🔍 Search").clicked() {
                                app.search.query = text.chars().take(80).collect();
                                app.tab = crate::app::Tab::Search;
                                app.start_search();
                                rd.sel = None;
                            }
                            if ui.button("🗙").clicked() {
                                rd.sel = None;
                            }
                        });
                    });
            });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            rd.sel = None;
        } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !ranges.is_empty() && !ui.ctx().wants_keyboard_input() {
            app.user.add_mark(&rd.loc, app.settings.default_highlight + 1, &ranges);
            rd.sel = None;
            rd.refresh(&app.user);
        }
    }
    // ---------- translation popup (rises from the Translate button; × closes it) ----------
    let mut close_tr = false;
    let mut toggle_wide_flag = false;
    if let Some((anchor, to, slot)) = &rd.translation {
        let result = slot.lock().unwrap().clone();
        let wide = rd.translation_wide;
        let (w, max_h) = if wide { (760.0_f32, 560.0_f32) } else { (480.0_f32, 320.0_f32) };
        let mut toggle_wide = false;
        egui::Area::new(egui::Id::new("translation_popup"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_BOTTOM)
            .fixed_pos(*anchor - egui::vec2(0.0, 10.0))
            .constrain(true)
            .show(ui.ctx(), |ui| {
                egui::Frame::default()
                    .fill(if dark { crate::theme::hex(0x333333) } else { Color32::WHITE })
                    .stroke(egui::Stroke::new(1.0_f32, Color32::from_black_alpha(40)))
                    .corner_radius(10.0)
                    .shadow(egui::Shadow { offset: [0, 3], blur: 14, spread: 0, color: Color32::from_black_alpha(60) })
                    .inner_margin(egui::Margin { left: 14, right: 8, top: 8, bottom: 12 })
                    .show(ui, |ui| {
                        ui.set_width(w);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(crate::translate::name(to)).size(12.5).color(crate::theme::accent(dark)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new("🗙").size(12.0)).frame(false)).on_hover_text("Close").clicked() {
                                    close_tr = true;
                                }
                                let label = if wide { "⇲ Smaller" } else { "⇱ Wider" };
                                if ui.add(egui::Button::new(RichText::new(label).size(12.5)).corner_radius(6.0)).on_hover_text("Change the size of this window").clicked() {
                                    toggle_wide = true;
                                }
                            });
                        });
                        ui.add_space(2.0);
                        match &result {
                            None => {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(RichText::new("Translating…").color(crate::theme::muted(dark)));
                                });
                            }
                            Some(Ok(t)) => {
                                // the wide window is also taller: it keeps its full height even for short text
                                egui::ScrollArea::vertical()
                                    .max_height(max_h)
                                    .min_scrolled_height(if wide { max_h } else { 0.0 })
                                    .auto_shrink(if wide { [false, false] } else { [false, true] })
                                    .show(ui, |ui| {
                                    ui.add(egui::Label::new(RichText::new(t).size(if wide { 17.0 } else { 15.5 }).color(crate::theme::text(dark))).selectable(true));
                                });
                            }
                            Some(Err(e)) => {
                                ui.label(RichText::new(format!("Translation isn't available right now ({e}). Check the internet connection.")).color(crate::theme::muted(dark)));
                            }
                        }
                    });
            });
        toggle_wide_flag = toggle_wide;
    }
    if close_tr {
        rd.translation = None;
    }
    if toggle_wide_flag {
        rd.translation_wide = !rd.translation_wide;
    }
    if let Some((bt, id)) = add_note_for {
        let (ranges, text) = rd.selection_ranges();
        let has_mark = rd.marks.iter().any(|m| m.ranges.iter().any(|r| r.btype == bt && r.ident == id));
        if !has_mark && !ranges.is_empty() {
            app.user.add_mark(&rd.loc, app.settings.default_highlight + 1, &ranges);
        }
        let title: String = text.split_whitespace().take(8).collect::<Vec<_>>().join(" ");
        app.edit_note(Note { loc: Some(rd.loc.clone()), btype: bt, ident: id, title, color: if has_mark { 0 } else { app.settings.default_highlight + 1 }, ..Default::default() });
        rd.sel = None;
        rd.refresh(&app.user);
    }

    // ---------- bookmarks ----------
    if rd.show_bookmarks {
        let mut open = true;
        let mut goto: Option<Bookmark> = None;
        egui::Window::new("Bookmarks")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-30.0, 90.0))
            .show(ui.ctx(), |ui| {
                if rd.bookmark_ident.is_some() {
                    ui.label(RichText::new("Choose a slot for the selected text").color(crate::theme::muted(dark)));
                } else {
                    ui.label(RichText::new("Click an empty slot to bookmark this page").color(crate::theme::muted(dark)));
                }
                ui.add_space(4.0);
                for slot in 0..10usize {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 24.0), egui::Sense::hover());
                        ui.painter().rect_filled(r, 1.0, crate::theme::bookmark(slot, dark));
                        let bm = rd.bookmarks[slot].clone();
                        match bm {
                            Some(bm) => {
                                let label = format!("{}  {}", bm.title, bm.snippet.chars().take(48).collect::<String>());
                                if ui.add(egui::Button::new(label).frame(false)).on_hover_text("Go to bookmark").clicked() {
                                    goto = Some(bm.clone());
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("🗙").on_hover_text("Remove").clicked() {
                                        app.user.remove_bookmark(&rd.pub_key, slot as i64);
                                        rd.refresh(&app.user);
                                    }
                                    if ui.small_button("⟳").on_hover_text("Replace with this place").clicked() {
                                        set_bookmark(app, rd, slot);
                                    }
                                });
                            }
                            None => {
                                if ui.add(egui::Button::new(RichText::new("Empty").color(crate::theme::muted(dark))).frame(false)).clicked() {
                                    set_bookmark(app, rd, slot);
                                }
                            }
                        }
                    });
                }
            });
        if !open {
            rd.show_bookmarks = false;
        }
        if let Some(bm) = goto {
            rd.show_bookmarks = false;
            if bm.loc.key == rd.loc.key {
                rd.scroll_to_ident = Some((bm.btype, bm.ident));
            } else {
                app.open_loc(&bm.loc, Some((bm.btype, bm.ident)));
            }
        }
    }
}

fn set_bookmark(app: &mut App, rd: &mut Reader, slot: usize) {
    let (bt, id, snippet) = match rd.bookmark_ident.take() {
        Some(x) => x,
        None => {
            // first paragraph/verse of the page
            let doc = rd.doc.as_ref();
            let first = doc.and_then(|d| d.blocks.iter().find_map(|b| b.segs.first().map(|s| (s.btype, s.ident, b.text[s.start..s.end].chars().take(120).collect::<String>()))));
            first.unwrap_or((0, 0, String::new()))
        }
    };
    if rd.loc.key.is_empty() {
        return;
    }
    let bm = Bookmark { pub_key: rd.pub_key.clone(), slot: slot as i64, loc: rd.loc.clone(), btype: bt, ident: id, title: rd.title.clone(), snippet };
    app.user.set_bookmark(&bm);
    rd.refresh(&app.user);
    app.toast(format!("Bookmark {} set", slot + 1));
    rd.show_bookmarks = false;
}

/// The reading screen's actions, shown as icons in the top header bar (right side, laid out
/// right to left): open on the web, favourite, bookmarks, note, find, text size, listen, next, previous.
pub fn header_actions(app: &mut App, ui: &mut Ui, rd: &mut Reader) {
    use crate::icons::{button, button_filled, Icon};
    let dark = app.dark;
    if let Source::Online { url, .. } = &rd.source {
        if button(ui, Icon::Globe, 18.0, "Open in web browser").clicked() {
            crate::paths::open_external(url);
        }
    }
    if let Source::Epub { p, .. } = &rd.source {
        let fav = app.user.is_favorite(&p.key());
        let fill = if fav { Some(crate::theme::accent(dark)) } else { None };
        if button_filled(ui, Icon::Star, 18.0, if fav { "Remove from Favorites" } else { "Add to Favorites" }, fill).clicked() {
            let p = p.clone();
            app.user.toggle_favorite(&p);
        }
    }
    if button(ui, Icon::Bookmark, 18.0, "Bookmarks").clicked() {
        rd.show_bookmarks = !rd.show_bookmarks;
        rd.bookmark_ident = None;
    }
    if button(ui, Icon::Note, 18.0, "Add a note to this page").clicked() && !rd.loc.key.is_empty() {
        app.edit_note(Note { loc: Some(rd.loc.clone()), ..Default::default() });
    }
    if button(ui, Icon::Search, 18.0, "Find on this page").clicked() {
        rd.show_find = !rd.show_find;
    }
    // text size: "Aa" with a small menu
    let (r, resp) = ui.allocate_exact_size(egui::vec2(32.0, 30.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xe4e4e4) });
    }
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "Aa", egui::FontId::proportional(16.0), crate::theme::icon(dark));
    let resp = resp.on_hover_text("Text size");
    crate::widgets::popup(ui, &resp, "textsize", |ui| {
        ui.horizontal(|ui| {
            if ui.button("A−").clicked() {
                app.settings.font_percent = (app.settings.font_percent - crate::settings::FONT_PERCENT_STEP).max(crate::settings::FONT_PERCENT_MIN);
                app.settings.save();
            }
            ui.label(format!("{:.0}%", app.settings.font_percent));
            if ui.button("A+").clicked() {
                app.settings.font_percent = (app.settings.font_percent + crate::settings::FONT_PERCENT_STEP).min(crate::settings::FONT_PERCENT_MAX);
                app.settings.save();
            }
        });
    });
    // listen: the chapter's recording, or the publication track with this article's title
    match &rd.source {
        Source::Bible { b, book, chapter, .. } => {
            let (b, book, chapter) = (b.clone(), *book, *chapter);
            if app.audio_for(&b).map(|m| m.contains_key(&(book, chapter))).unwrap_or(false) && button(ui, Icon::Audio, 18.0, "Listen to this chapter").clicked() {
                app.play_bible_audio(&b, book, chapter);
            }
        }
        Source::Epub { p, .. } => {
            let p = p.clone();
            let t = rd.title.to_lowercase();
            if let Some(tracks) = app.audio_for_pub(&p) {
                let hit = tracks.iter().position(|(tt, _)| {
                    let tt = tt.to_lowercase();
                    !tt.is_empty() && (tt == t || tt.contains(&t) || t.contains(&tt))
                });
                if let Some(i) = hit {
                    if button(ui, Icon::Audio, 18.0, "Listen to this article").clicked() {
                        let items: Vec<(String, String, String)> = tracks[i..].iter().map(|(a, u)| (a.clone(), u.clone(), String::new())).collect();
                        app.play(items, true);
                    }
                }
            }
        }
        _ => {}
    }
    if matches!(rd.source, Source::Bible { .. } | Source::Epub { .. }) {
        if button(ui, Icon::Chevron, 16.0, "Next").clicked() {
            rd.nav(app, 1);
        }
        if button(ui, Icon::ChevronLeft, 16.0, "Previous").clicked() {
            rd.nav(app, -1);
        }
    }
}
