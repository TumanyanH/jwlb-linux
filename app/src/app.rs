use crate::catalog::{Catalog, PubInfo};
use crate::content::Doc;
use crate::reader::Reader;
use crate::settings::{Settings, ThemeChoice};
use crate::userdata::{Note, UserData};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Tab {
    Home,
    Bible,
    Publications,
    Media,
    Meetings,
    Study,
    Search,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 8] = [Tab::Home, Tab::Bible, Tab::Publications, Tab::Media, Tab::Meetings, Tab::Study, Tab::Search, Tab::Settings];
    pub fn label(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Bible => "Bible",
            Tab::Publications => "Publications",
            Tab::Media => "Media",
            Tab::Meetings => "Meetings",
            Tab::Study => "Personal Study",
            Tab::Search => "Search",
            Tab::Settings => "Settings",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Tab::Home => "🏠",
            Tab::Bible => "📖",
            Tab::Publications => "📚",
            Tab::Media => "🎬",
            Tab::Meetings => "👥",
            Tab::Study => "📝",
            Tab::Search => "🔍",
            Tab::Settings => "⚙",
        }
    }
    fn root(self) -> Page {
        match self {
            Tab::Home => Page::Home,
            Tab::Bible => Page::BibleBooks,
            Tab::Publications => Page::PubCategories,
            Tab::Media => Page::MediaRoot,
            Tab::Meetings => Page::Meetings,
            Tab::Study => Page::Study,
            Tab::Search => Page::Search,
            Tab::Settings => Page::Settings,
        }
    }
}

/// Optional start page from the command line (--open).
pub static START_PAGE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub enum Page {
    Home,
    BibleBooks,
    BibleChapters(u8),
    BibleSelect,
    Reader(Box<Reader>),
    PubCategories,
    PubList(i64),
    Toc(PubInfo),
    WolList(String, String),
    MediaRoot,
    MediaCategory(String, String),
    Meetings,
    Study,
    Search,
    Settings,
}

#[derive(Clone, Debug)]
pub struct MediaItem {
    pub key: String,
    pub title: String,
    pub duration: String,
    pub image: String,
    pub kind: String,
    pub files: Vec<(String, String, u64)>,
}

#[derive(Clone, Debug)]
pub struct MediaCategory {
    pub key: String,
    pub name: String,
    pub description: String,
    pub image: String,
    pub banner: String,
    pub subcats: Vec<MediaCategory>,
    pub media: Vec<MediaItem>,
}

#[derive(Clone, Debug)]
pub struct MeetingItem {
    pub line1: String,
    pub line2: String,
    pub href: String,
    pub symbol: String,
    /// cover picture of the publication (online-library thumbnail)
    pub thumb: String,
}

#[derive(Clone, Debug)]
pub struct MeetingSection {
    pub title: String,
    pub items: Vec<MeetingItem>,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub pubinfo: PubInfo,
    pub href: String,
    pub title: String,
    pub snippet: String,
    pub book_chapter: Option<(u8, u16)>,
}

pub enum Loadable<T> {
    Loading,
    Ready(T),
    Failed(String),
}

pub enum Msg {
    Catalog(Result<usize, String>),
    Daily(String, Result<Vec<crate::content::Block>, String>),
    Downloaded(String, Result<String, String>),
    Image(String, Option<egui::ColorImage>),
    Media(String, Result<MediaCategory, String>),
    Meetings(String, Result<Vec<MeetingSection>, String>),
    Online(u64, Result<Doc, String>),
    Search(u64, Vec<SearchHit>, bool),
    Toast(String),
    Bibles(String, Result<Vec<(String, String)>, String>),
    Books(String, Result<Vec<crate::wol::BookIdx>, String>),
    WolList(String, Result<Vec<crate::wol::Card>, String>),
    BibleAudio(String, Result<HashMap<(u8, u16), (String, String)>, String>),
    PubAudio(String, Vec<(String, String)>),
}

pub enum ImgState {
    Loading,
    Ready(egui::TextureHandle),
    Failed,
}

pub struct Progress {
    pub done: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
}

impl Progress {
    pub fn new() -> Progress {
        Progress { done: Arc::new(AtomicU64::new(0)), total: Arc::new(AtomicU64::new(0)) }
    }
    pub fn fraction(&self) -> f32 {
        let t = self.total.load(Ordering::Relaxed);
        if t == 0 {
            0.0
        } else {
            (self.done.load(Ordering::Relaxed) as f32 / t as f32).min(1.0)
        }
    }
}

pub struct NoteEditor {
    pub note: Note,
    pub tag_input: String,
    pub confirm_delete: bool,
}

pub struct SearchState {
    pub query: String,
    pub running: bool,
    pub req: u64,
    pub hits: Vec<SearchHit>,
    pub scope: usize,
    pub done_query: String,
    pub focus: bool,
}

pub struct App {
    pub settings: Settings,
    pub catalog: Catalog,
    pub user: UserData,
    pub tx: Sender<Msg>,
    rx: Receiver<Msg>,
    pub ctx: egui::Context,
    pub dark: bool,
    pub tab: Tab,
    pub stacks: HashMap<Tab, Vec<Page>>,
    pub catalog_job: Option<Progress>,
    pub catalog_error: Option<String>,
    pub downloads: HashMap<String, Progress>,
    pub images: HashMap<String, ImgState>,
    img_queue: VecDeque<String>,
    img_inflight: usize,
    pub daily: HashMap<String, Loadable<Vec<crate::content::Block>>>,
    pub daily_date: chrono::NaiveDate,
    pub media: HashMap<String, Loadable<MediaCategory>>,
    pub media_popup: Option<MediaItem>,
    pub meetings: HashMap<String, Loadable<Vec<MeetingSection>>>,
    pub meeting_week: chrono::NaiveDate,
    pub note_editor: Option<NoteEditor>,
    pub toast: Option<(String, f64)>,
    pub search: SearchState,
    pub study_tab: usize,
    pub study_filter: String,
    pub study_tag: Option<String>,
    pub study_playlist: Option<i64>,
    pub backup_path: String,
    pub lang_filter: String,
    pub show_lang_picker: bool,
    pub playlist_picker: Option<crate::userdata::PlaylistItem>,
    pub new_playlist: String,
    pub rename_tag: Option<(String, String)>,
    pub online_req: u64,
    screenshot: Option<std::path::PathBuf>,
    frame: u64,
    shot_requested: bool,
    pub bible_cache: Option<(String, Vec<crate::epub::BibleBookIdx>)>,
    pub bible_books: HashMap<String, Loadable<Vec<crate::wol::BookIdx>>>,
    pub bible_lists: HashMap<String, Loadable<Vec<(String, String)>>>,
    pub bible_add: Option<String>,
    pub bible_add_lang: Option<String>,
    pub wol_lists: HashMap<String, Loadable<Vec<crate::wol::Card>>>,
    pub player: Option<crate::ui_player::Player>,
    pub pending_bible: Option<(u8, u16)>,
    pub nav_compact: bool,
    pub page_title: String,
    pub page_subtitle: String,
    /// Library tabs: 0 publications, 1 video, 2 audio, 3 downloaded
    pub library_view: u8,
    pub bible_filter: String,
    pub bible_audio: HashMap<String, Loadable<HashMap<(u8, u16), (String, String)>>>,
    pub pub_audio: HashMap<String, Loadable<Vec<(String, String)>>>,
    pub bibles_refresh_asked: bool,
    pub media_fallback: Option<(Vec<(String, String, String)>, String)>,
    pub library_tab: Tab,
    pub pending_pop: bool,
    pub stacks_reset: bool,
    pub cur_depth: usize,
}

pub fn app_icon() -> egui::IconData {
    let n = 128usize;
    let mut rgba = vec![0u8; n * n * 4];
    let blue = [0x4a, 0x6d, 0xa7];
    for y in 0..n {
        for x in 0..n {
            let i = (y * n + x) * 4;
            // rounded square
            let r = 24.0;
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = fx.clamp(r, n as f32 - r);
            let cy = fy.clamp(r, n as f32 - r);
            let inside = (fx - cx).powi(2) + (fy - cy).powi(2) <= r * r;
            if !inside {
                continue;
            }
            let mut c = [blue[0], blue[1], blue[2], 255];
            // open book: two white pages
            let page_l = x >= 22 && x <= 62 && y >= 34 && y <= 94 && (y as i32) >= 34 + (62 - x as i32) / 6;
            let page_r = x >= 66 && x <= 106 && y >= 34 && y <= 94 && (y as i32) >= 34 + (x as i32 - 66) / 6;
            if page_l || page_r {
                c = [255, 255, 255, 255];
                // text lines
                let rel = (y as i32 - 46) % 9;
                let lx = if page_l { x >= 28 && x <= 56 } else { x >= 72 && x <= 100 };
                if y >= 46 && y <= 86 && rel == 0 && lx {
                    c = [0x9f, 0xb9, 0xe3, 255];
                }
            }
            rgba[i..i + 4].copy_from_slice(&c);
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<std::path::PathBuf>) -> App {
        crate::fonts::install(&cc.egui_ctx);
        let settings = Settings::load();
        let (tx, rx) = channel();
        let catalog = Catalog::open(&settings.lang);
        let today = chrono::Local::now().date_naive();
        let mut app = App {
            catalog,
            user: UserData::open(),
            tx,
            rx,
            ctx: cc.egui_ctx.clone(),
            dark: false,
            tab: Tab::Home,
            stacks: HashMap::new(),
            catalog_job: None,
            catalog_error: None,
            downloads: HashMap::new(),
            images: HashMap::new(),
            img_queue: VecDeque::new(),
            img_inflight: 0,
            daily: HashMap::new(),
            daily_date: today,
            media: HashMap::new(),
            media_popup: None,
            meetings: HashMap::new(),
            meeting_week: monday_of(today),
            note_editor: None,
            toast: None,
            search: SearchState { query: String::new(), running: false, req: 0, hits: vec![], scope: 0, done_query: String::new(), focus: false },
            study_tab: 0,
            study_filter: String::new(),
            study_tag: None,
            study_playlist: None,
            backup_path: dirs::download_dir()
                .or_else(dirs::home_dir)
                .map(|d| d.join(format!("StudyLibrary-backup-{}.studylib", today.format("%Y-%m-%d"))).to_string_lossy().into_owned())
                .unwrap_or_default(),
            lang_filter: String::new(),
            show_lang_picker: false,
            playlist_picker: None,
            new_playlist: String::new(),
            rename_tag: None,
            online_req: 0,
            screenshot,
            frame: 0,
            shot_requested: false,
            bible_cache: None,
            bible_books: HashMap::new(),
            bible_lists: HashMap::new(),
            bible_add: None,
            bible_add_lang: None,
            wol_lists: HashMap::new(),
            player: None,
            pending_bible: None,
            nav_compact: true,
            page_title: String::new(),
            page_subtitle: String::new(),
            library_view: 0,
            bible_filter: String::new(),
            bible_audio: HashMap::new(),
            pub_audio: HashMap::new(),
            bibles_refresh_asked: false,
            media_fallback: None,
            library_tab: Tab::Publications,
            pending_pop: false,
            stacks_reset: false,
            cur_depth: 1,
            settings,
        };
        app.update_theme();
        for t in Tab::ALL {
            app.stacks.insert(t, vec![t.root()]);
        }
        // Like the original, fetch the publication catalog on first start (not during a screenshot check).
        if !app.catalog.ready() && app.screenshot.is_none() {
            app.start_catalog_update();
        }
        if let Some(start) = START_PAGE.get() {
            let parts: Vec<&str> = start.split(':').collect();
            match parts.as_slice() {
                ["bible", b, c] => {
                    if let (Ok(b), Ok(c)) = (b.parse(), c.parse()) {
                        app.tab = Tab::Bible;
                        app.pending_bible = Some((b, c));
                    }
                }
                ["chapters", b] => {
                    if let Ok(b) = b.parse() {
                        app.tab = Tab::Bible;
                        app.stacks.entry(Tab::Bible).or_default().push(Page::BibleChapters(b));
                    }
                }
                ["library", v] => {
                    app.tab = Tab::Publications;
                    app.library_view = v.parse().unwrap_or(0);
                }
                ["bible-select"] => {
                    app.tab = Tab::Bible;
                    app.stacks.entry(Tab::Bible).or_default().push(Page::BibleSelect);
                }
                _ if start.starts_with("play:") => {
                    let u = start["play:".len()..].to_string();
                    app.play(vec![("Test".into(), u, String::new())], false);
                }
                [t] => {
                    if let Some(tab) = Tab::ALL.iter().find(|x| x.label().to_lowercase().replace(' ', "") == t.to_lowercase()) {
                        app.tab = *tab;
                    }
                }
                _ => {}
            }
        }
        app
    }

    pub fn update_theme(&mut self) {
        let dark = match self.settings.theme {
            ThemeChoice::Dark => true,
            ThemeChoice::Light => false,
            ThemeChoice::System => self.ctx.system_theme().map(|t| t == egui::Theme::Dark).unwrap_or(false),
        };
        self.dark = dark;
        crate::theme::apply(&self.ctx, dark);
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        let t = self.ctx.input(|i| i.time);
        self.toast = Some((s.into(), t + 4.0));
    }

    // ---------- navigation ----------
    pub fn push(&mut self, p: Page) {
        self.stacks.entry(self.tab).or_default().push(p);
    }
    pub fn push_on(&mut self, tab: Tab, p: Page) {
        self.tab = tab;
        self.stacks.entry(tab).or_default().push(p);
    }
    pub fn back(&mut self) {
        let s = self.stacks.entry(self.tab).or_default();
        if s.len() > 1 {
            s.pop();
        }
    }
    pub fn can_go_back(&self) -> bool {
        self.stacks.get(&self.tab).map(|s| s.len() > 1).unwrap_or(false)
    }

    // ---------- background work ----------
    pub fn start_catalog_update(&mut self) {
        if self.catalog_job.is_some() {
            return;
        }
        let p = Progress::new();
        let (d, t) = (p.done.clone(), p.total.clone());
        let lang = self.settings.lang.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = crate::catalog::update(&lang, d, t);
            let _ = tx.send(Msg::Catalog(r));
            ctx.request_repaint();
        });
        self.catalog_error = None;
        self.catalog_job = Some(p);
    }

    pub fn download_pub(&mut self, p: &PubInfo) {
        let key = p.key();
        if self.downloads.contains_key(&key) {
            return;
        }
        let prog = Progress::new();
        let (d, t) = (prog.done.clone(), prog.total.clone());
        let p = p.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let k = key.clone();
        std::thread::spawn(move || {
            let r = (|| {
                let epub = crate::net::find_pub_file(&p.key_symbol, p.issue, &p.lang, &["EPUB"]).or_else(|e| {
                    if p.symbol != p.key_symbol {
                        crate::net::find_pub_file(&p.symbol, p.issue, &p.lang, &["EPUB"])
                    } else {
                        Err(e)
                    }
                });
                if let Ok((_, url, _)) = epub {
                    let dest = p.file_path();
                    crate::net::download(&url, &dest, d, t)?;
                    crate::epub::Epub::open(&dest).map_err(|e| {
                        let _ = std::fs::remove_file(&dest);
                        format!("The download could not be opened: {e}")
                    })?;
                    return Ok("EPUB".to_string());
                }
                // No EPUB (e.g. study Bible, Insight): keep the publication's online-library pages.
                let root = crate::wol::publication_root(&p.key_symbol, p.issue, &p.lang)?;
                crate::wol::download_publication(&root, d, t)?;
                let m = crate::wol::Installed { root, title: p.display_title() };
                std::fs::write(p.wol_marker(), serde_json::to_string(&m).unwrap_or_default()).map_err(|e| e.to_string())?;
                Ok("WOL".to_string())
            })();
            let _ = tx.send(Msg::Downloaded(k, r));
            ctx.request_repaint();
        });
        self.downloads.insert(key, prog);
    }

    pub fn delete_pub(&mut self, p: &PubInfo) {
        let _ = std::fs::remove_file(p.file_path());
        let _ = std::fs::remove_file(p.pdf_path());
        let _ = std::fs::remove_file(p.wol_marker());
        if let Some((k, _)) = &self.bible_cache {
            if *k == p.key() {
                self.bible_cache = None;
            }
        }
        self.toast(format!("Deleted “{}”", p.display_title()));
    }

    /// Request a picture (web address or epub:path::inner); returns its texture when ready.
    pub fn image(&mut self, key: &str) -> Option<egui::TextureHandle> {
        match self.images.get(key) {
            Some(ImgState::Ready(t)) => return Some(t.clone()),
            Some(_) => return None,
            None => {}
        }
        self.images.insert(key.to_string(), ImgState::Loading);
        self.img_queue.push_back(key.to_string());
        None
    }

    fn pump_images(&mut self) {
        while self.img_inflight < 6 {
            let Some(key) = self.img_queue.pop_front() else { break };
            self.img_inflight += 1;
            let tx = self.tx.clone();
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                let bytes = if let Some(rest) = key.strip_prefix("epub:") {
                    let mut parts = rest.splitn(2, "::");
                    let file = parts.next().unwrap_or("");
                    let inner = parts.next().unwrap_or("");
                    (|| {
                        let f = std::fs::File::open(file).ok()?;
                        let mut z = zip::ZipArchive::new(f).ok()?;
                        let mut e = z.by_name(inner).ok()?;
                        let mut v = Vec::new();
                        std::io::Read::read_to_end(&mut e, &mut v).ok()?;
                        Some(v)
                    })()
                } else {
                    let cache = crate::paths::cache_dir().join("img").join(format!("{:x}", fxhash(&key)));
                    if let Ok(b) = std::fs::read(&cache) {
                        Some(b)
                    } else {
                        let b = crate::net::get_bytes(&key, 8 * 1024 * 1024).ok();
                        if let Some(b) = &b {
                            let _ = std::fs::create_dir_all(cache.parent().unwrap());
                            let _ = std::fs::write(&cache, b);
                        }
                        b
                    }
                };
                let img = bytes.and_then(|b| image::load_from_memory(&b).ok()).map(|im| {
                    let im = if im.width() > 1400 { im.resize(1400, 1400 * im.height() / im.width().max(1), image::imageops::FilterType::Triangle) } else { im };
                    let rgba = im.to_rgba8();
                    egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw())
                });
                let _ = tx.send(Msg::Image(key, img));
                ctx.request_repaint();
            });
        }
    }

    pub fn load_daily(&mut self, date: chrono::NaiveDate) {
        let k = format!("{}|{}", self.settings.lang, date);
        if self.daily.contains_key(&k) {
            return;
        }
        self.daily.insert(k.clone(), Loadable::Loading);
        let lang = self.settings.lang.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let cache = crate::paths::cache_dir().join(format!("dt_{}_{}.html", crate::paths::safe_name(&lang), date));
            let r = (|| {
                let html = if let Ok(s) = std::fs::read_to_string(&cache) {
                    s
                } else {
                    let w = crate::net::wol_locale(&lang)?;
                    let url = format!(
                        "{}/wol/dt/{}/{}/{}",
                        crate::net::WOL,
                        w.r,
                        w.lp,
                        date.format("%Y/%-m/%-d")
                    );
                    let v = crate::net::get_json(&url)?;
                    let html = v
                        .pointer("/items/0/content")
                        .and_then(|c| c.as_str())
                        .ok_or("No daily text for this day")?
                        .to_string();
                    let _ = std::fs::write(&cache, &html);
                    html
                };
                Ok(crate::content::parse_fragment(&html))
            })();
            let _ = tx.send(Msg::Daily(k, r));
            ctx.request_repaint();
        });
    }

    pub fn load_media(&mut self, key: &str) {
        let k = format!("{}|{}", self.settings.lang, key);
        if self.media.contains_key(&k) {
            return;
        }
        self.media.insert(k.clone(), Loadable::Loading);
        let lang = self.settings.lang.clone();
        let key = key.to_string();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let url = format!("{}/categories/{}/{}?detailed=1&clientType=www", crate::net::MEDIATOR, crate::net::urlencode(&lang), crate::net::urlencode(&key));
            let r = crate::net::get_json(&url).and_then(|v| parse_category(v.get("category").ok_or("No such category")?).ok_or("Bad answer".into()));
            let _ = tx.send(Msg::Media(k, r));
            ctx.request_repaint();
        });
    }

    pub fn load_meetings(&mut self, monday: chrono::NaiveDate) {
        let k = format!("{}|{}", self.settings.lang, monday);
        if self.meetings.contains_key(&k) {
            return;
        }
        self.meetings.insert(k.clone(), Loadable::Loading);
        let lang = self.settings.lang.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = (|| {
                let w = crate::net::wol_locale(&lang)?;
                use chrono::Datelike;
                let iw = monday.iso_week();
                let url = format!("{}/{}/wol/meetings/{}/{}/{}/{}", crate::net::WOL, w.locale, w.r, w.lp, iw.year(), iw.week());
                let html = crate::net::get_string(&url)?;
                Ok(parse_meetings(&html))
            })();
            let _ = tx.send(Msg::Meetings(k, r));
            ctx.request_repaint();
        });
    }

    pub fn load_online(&mut self, url: String) -> u64 {
        self.online_req += 1;
        let id = self.online_req;
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = crate::wol::fetch(&url).map(|h| crate::content::parse_wol_page(&h));
            let _ = tx.send(Msg::Online(id, r));
            ctx.request_repaint();
        });
        id
    }

    fn handle_msgs(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Catalog(r) => {
                    self.catalog_job = None;
                    match r {
                        Ok(n) => {
                            self.catalog = Catalog::open(&self.settings.lang);
                            self.toast(format!("Library updated: {n} publications"));
                        }
                        Err(e) => self.catalog_error = Some(e),
                    }
                }
                Msg::Daily(k, r) => {
                    self.daily.insert(k, match r {
                        Ok(b) => Loadable::Ready(b),
                        Err(e) => Loadable::Failed(e),
                    });
                }
                Msg::Downloaded(k, r) => {
                    self.downloads.remove(&k);
                    match r {
                        Ok(_) => self.toast("Download complete"),
                        Err(e) => self.toast(format!("Download failed: {e}")),
                    }
                }
                Msg::Image(k, img) => {
                    self.img_inflight = self.img_inflight.saturating_sub(1);
                    let st = match img {
                        Some(ci) => ImgState::Ready(self.ctx.load_texture(&k, ci, egui::TextureOptions::LINEAR)),
                        None => ImgState::Failed,
                    };
                    self.images.insert(k, st);
                }
                Msg::Media(k, r) => {
                    self.media.insert(k, match r {
                        Ok(c) => Loadable::Ready(c),
                        Err(e) => Loadable::Failed(e),
                    });
                }
                Msg::Meetings(k, r) => {
                    self.meetings.insert(k, match r {
                        Ok(c) => Loadable::Ready(c),
                        Err(e) => Loadable::Failed(e),
                    });
                }
                Msg::Online(id, r) => {
                    for stack in self.stacks.values_mut() {
                        for p in stack.iter_mut() {
                            if let Page::Reader(rd) = p {
                                if rd.request == id {
                                    rd.receive_online(r.clone(), &self.user);
                                }
                            }
                        }
                    }
                }
                Msg::Search(id, hits, done) => {
                    if id == self.search.req {
                        self.search.hits.extend(hits);
                        if done {
                            self.search.running = false;
                        }
                    }
                }
                Msg::Toast(s) => self.toast(s),
                Msg::Bibles(l, r) => {
                    // first start: use this language's first Bible automatically
                    if let Ok(list) = &r {
                        if self.settings.bibles.is_empty() && l == self.settings.lang {
                            if let Some((sym, title)) = list.iter().find(|(s, _)| s == "nwt").or(list.first()) {
                                let b = crate::wol::BibleRef { lang: l.clone(), symbol: sym.clone(), title: title.clone() };
                                self.add_bible(b);
                            }
                        }
                    }
                    self.bible_lists.insert(l, match r {
                        Ok(v) => Loadable::Ready(v),
                        Err(e) => Loadable::Failed(e),
                    });
                }
                Msg::Books(k, r) => {
                    self.bible_books.insert(k, match r {
                        Ok(v) => Loadable::Ready(v),
                        Err(e) => Loadable::Failed(e),
                    });
                }
                Msg::BibleAudio(k, r) => {
                    self.bible_audio.insert(k, match r {
                        Ok(v) => Loadable::Ready(v),
                        Err(e) => Loadable::Failed(e),
                    });
                }
                Msg::PubAudio(k, v) => {
                    self.pub_audio.insert(k, Loadable::Ready(v));
                }
                Msg::WolList(k, r) => {
                    self.wol_lists.insert(k, match r {
                        Ok(v) => Loadable::Ready(v),
                        Err(e) => Loadable::Failed(e),
                    });
                }
            }
        }
    }

    pub fn downloaded_pubs(&self) -> Vec<PubInfo> {
        let dir = crate::paths::pubs_dir(&self.settings.lang);
        let mut out = vec![];
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let Some(stem) = name.strip_suffix(".epub").or_else(|| name.strip_suffix(".wol.json")) else { continue };
                let Some((sym, issue)) = stem.rsplit_once('_') else { continue };
                let issue: i64 = issue.parse().unwrap_or(0);
                let p = self
                    .catalog
                    .find(sym, issue)
                    .unwrap_or_else(|| PubInfo::simple(sym, issue, &self.settings.lang, sym, 0));
                out.push(p);
            }
        }
        out.dedup_by(|a, b| a.key() == b.key());
        out.sort_by(|a, b| a.type_id.cmp(&b.type_id).then(b.year.cmp(&a.year)).then(a.title.cmp(&b.title)));
        out
    }

    pub fn open_pub(&mut self, p: &PubInfo) {
        // Bibles open on the Bible tab (every translation is read through the online library and saved)
        if p.type_id == 1 {
            let b = crate::wol::BibleRef { lang: p.lang.clone(), symbol: p.key_symbol.clone(), title: p.display_title() };
            self.add_bible(b.clone());
            if !App::bible_downloaded(&b) {
                self.download_bible(&b);
            }
            self.tab = Tab::Bible;
            return;
        }
        if p.file_path().exists() {
            self.push(Page::Toc(p.clone()));
        } else if let Some(m) = p.wol_installed() {
            self.push(Page::WolList(m.root, m.title));
        } else {
            self.download_pub(p);
        }
    }

    /// Chapter recordings of a Bible (loaded once, in the background).
    pub fn audio_for(&mut self, b: &crate::wol::BibleRef) -> Option<HashMap<(u8, u16), (String, String)>> {
        let k = b.key();
        match self.bible_audio.get(&k) {
            Some(Loadable::Ready(v)) => return Some(v.clone()),
            Some(_) => return None,
            None => {}
        }
        self.bible_audio.insert(k.clone(), Loadable::Loading);
        let b = b.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = crate::wol::bible_audio(&b);
            let _ = tx.send(Msg::BibleAudio(k, r));
            ctx.request_repaint();
        });
        None
    }

    /// Audio tracks of a publication (loaded once, in the background).
    pub fn audio_for_pub(&mut self, p: &PubInfo) -> Option<Vec<(String, String)>> {
        let k = p.key();
        match self.pub_audio.get(&k) {
            Some(Loadable::Ready(v)) => return Some(v.clone()),
            Some(_) => return None,
            None => {}
        }
        self.pub_audio.insert(k.clone(), Loadable::Loading);
        let p = p.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let v = crate::wol::pub_audio(&p.key_symbol, p.issue, &p.lang);
            let _ = tx.send(Msg::PubAudio(k, v));
            ctx.request_repaint();
        });
        None
    }

    /// Play a Bible book's recordings from a chapter onwards.
    pub fn play_bible_audio(&mut self, b: &crate::wol::BibleRef, book: u8, chapter: u16) {
        let Some(map) = self.audio_for(b) else {
            self.toast("Loading the recordings…");
            return;
        };
        let mut chs: Vec<(u16, (String, String))> = map.iter().filter(|((bk, c), _)| *bk == book && *c >= chapter).map(|((_, c), v)| (*c, v.clone())).collect();
        chs.sort_by_key(|x| x.0);
        if chs.is_empty() {
            self.toast("No recording for this chapter");
            return;
        }
        let items = chs.into_iter().map(|(_, (t, u))| (t, u, String::new())).collect();
        self.play(items, true);
    }

    pub fn load_wol_list(&mut self, url: &str) {
        if self.wol_lists.contains_key(url) {
            return;
        }
        self.wol_lists.insert(url.to_string(), Loadable::Loading);
        let u = url.to_string();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r = crate::wol::fetch(&u).map(|h| crate::wol::cards(&h));
            let _ = tx.send(Msg::WolList(u, r));
            ctx.request_repaint();
        });
    }

    pub fn open_doc(&mut self, p: &PubInfo, href: &str) {
        match Reader::epub(p.clone(), href, &self.user) {
            Ok(rd) => self.push(Page::Reader(Box::new(rd))),
            Err(e) => self.toast(e),
        }
    }

    pub fn open_online(&mut self, url: &str, title: &str) {
        let id = self.load_online(url.to_string());
        let rd = Reader::online(url, title, id, &self.settings.lang);
        self.push(Page::Reader(Box::new(rd)));
    }

    /// Open a saved place (from notes, bookmarks, history, highlights).
    pub fn open_loc(&mut self, l: &crate::userdata::Loc, ident: Option<(i64, i64)>) {
        if l.key.starts_with("bible:") {
            let verse = ident.filter(|(bt, _)| *bt == 2).map(|(_, v)| v as u16);
            self.open_bible(l.book as u8, l.chapter as u16, verse);
            return;
        }
        let p = l.pub_info();
        // downloaded EPUB?
        if let Some(p) = &p {
            let p = self.catalog.find(&p.key_symbol, p.issue).unwrap_or_else(|| p.clone());
            if p.is_downloaded() {
                let href = if !l.href.is_empty() && !l.href.starts_with("http") {
                    Some(l.href.clone())
                } else if l.doc_id > 0 {
                    crate::epub::Epub::open(&p.file_path()).ok().and_then(|mut e| e.find_doc(l.doc_id))
                } else {
                    None
                };
                if let Some(h) = href {
                    if let Ok(mut rd) = Reader::epub(p.clone(), &h, &self.user) {
                        rd.scroll_to_ident = ident;
                        self.push(Page::Reader(Box::new(rd)));
                        return;
                    }
                }
            }
        }
        if l.href.starts_with("http") {
            self.open_online(&l.href.clone(), &l.title.clone());
            return;
        }
        if l.doc_id > 0 {
            let lang = p.map(|p| p.lang).unwrap_or_else(|| self.settings.lang.clone());
            if let Ok(w) = crate::net::wol_locale(&lang) {
                let url = format!("{}/{}/wol/d/{}/{}/{}", crate::net::WOL, w.locale, w.r, w.lp, l.doc_id);
                self.open_online(&url, &l.title.clone());
                return;
            }
        }
        self.toast("Download this publication to open it");
    }

    /// Search from the top bar: a scripture reference opens the Bible, anything else searches.
    pub fn run_search_or_reference(&mut self) {
        let q = self.search.query.clone();
        if q.chars().any(|c| c.is_ascii_digit()) {
            let mut names: Vec<(u8, String, String)> = crate::bible::BOOKS.iter().enumerate().map(|(i, b)| (i as u8 + 1, b.name.to_string(), b.abbr.to_string())).collect();
            if let Some((_, idx)) = self.bible_index() {
                names.splice(0..0, idx.iter().map(|b| (b.num, b.name.clone(), b.abbr.clone())));
            }
            if let Some((b, c, v)) = crate::bible::parse_reference(&q, &names) {
                self.open_bible(b, c, v);
                return;
            }
        }
        self.start_search();
    }

    pub fn edit_note(&mut self, n: Note) {
        self.note_editor = Some(NoteEditor { note: n, tag_input: String::new(), confirm_delete: false });
    }

    pub fn start_search(&mut self) {
        let q = self.search.query.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.search.req += 1;
        self.search.hits.clear();
        self.search.running = true;
        self.search.done_query = q.clone();
        let id = self.search.req;
        let pubs = self.downloaded_pubs();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let ql = q.to_lowercase();
            let mut total = 0;
            for p in pubs {
                let Ok(mut e) = crate::epub::Epub::open(&p.file_path()) else { continue };
                let mut batch = vec![];
                let spine = e.spine.clone();
                let bible_map: HashMap<String, (u8, u16)> = e
                    .bible
                    .as_ref()
                    .map(|b| {
                        b.iter()
                            .flat_map(|bk| bk.chapters.iter().enumerate().map(move |(i, h)| (h.clone(), (bk.num, i as u16 + 1))))
                            .collect()
                    })
                    .unwrap_or_default();
                for href in spine {
                    if href.contains("nav") {
                        continue;
                    }
                    let Some(html) = e.read_text(&href) else { continue };
                    let text = crate::content::plain_text(&html);
                    let lower = text.to_lowercase();
                    let same = lower.len() == text.len();
                    let mut from = 0;
                    let mut per_doc = 0;
                    while let Some(pos) = lower[from..].find(&ql) {
                        let at = from + pos;
                        from = at + ql.len().max(1);
                        let src = if same { &text } else { &lower };
                        let mut s = at.saturating_sub(70);
                        while !src.is_char_boundary(s) {
                            s -= 1;
                        }
                        let mut en = (at + ql.len() + 90).min(src.len());
                        while !src.is_char_boundary(en) {
                            en += 1;
                        }
                        let snippet = src[s..en].split_whitespace().collect::<Vec<_>>().join(" ");
                        let title = e
                            .toc_title_for(&href)
                            .or_else(|| text.lines().map(|l| l.trim()).find(|l| !l.is_empty()).map(|l| l.chars().take(80).collect()))
                            .unwrap_or_default();
                        batch.push(SearchHit {
                            pubinfo: p.clone(),
                            href: href.clone(),
                            title,
                            snippet,
                            book_chapter: bible_map.get(&href).copied(),
                        });
                        per_doc += 1;
                        total += 1;
                        if per_doc >= 5 || total >= 400 {
                            break;
                        }
                    }
                    if total >= 400 {
                        break;
                    }
                }
                if !batch.is_empty() {
                    let _ = tx.send(Msg::Search(id, batch, false));
                    ctx.request_repaint();
                }
                if total >= 400 {
                    break;
                }
            }
            let _ = tx.send(Msg::Search(id, vec![], true));
            ctx.request_repaint();
        });
    }

    pub fn change_language(&mut self, sym: &str) {
        if self.settings.lang == sym {
            return;
        }
        self.settings.lang = sym.to_string();
        self.settings.save();
        self.catalog = Catalog::open(sym);
        self.bible_cache = None;
        self.daily.clear();
        self.media.clear();
        self.meetings.clear();
        if !self.catalog.ready() {
            self.start_catalog_update();
        }
        for t in Tab::ALL {
            self.stacks.insert(t, vec![t.root()]);
        }
        self.stacks_reset = true;
        self.tab = Tab::Settings;
    }

    // ---------- frame ----------
    /// The original's purple window title bar (jwlibrary_purple), with our own window buttons.
    #[allow(dead_code)]
    fn title_bar(&mut self, ctx: &egui::Context) {
        let dark = self.dark;
        let bar = if dark { crate::theme::hex(0x2b1d42) } else { crate::theme::PRIMARY };
        egui::TopBottomPanel::top("winbar")
            .exact_height(32.0)
            .show_separator_line(false)
            .frame(egui::Frame::default().fill(bar).inner_margin(egui::Margin { left: 10, right: 0, top: 0, bottom: 0 }))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                let drag = ui.interact(full, egui::Id::new("winbar_drag"), egui::Sense::click_and_drag());
                if drag.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    let m = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!m));
                }
                ui.painter().text(full.left_center() + egui::vec2(10.0, 0.0), egui::Align2::LEFT_CENTER, "Study Library", egui::FontId::proportional(12.0), egui::Color32::WHITE);
                // window buttons
                let w = 46.0;
                let mut x = full.right() - w;
                for (i, glyph) in ["🗙", "🗖", "🗕"].iter().enumerate() {
                    let r = egui::Rect::from_min_size(egui::pos2(x, full.top()), egui::vec2(w, full.height()));
                    let resp = ui.interact(r, egui::Id::new(("winbtn", i)), egui::Sense::click());
                    if resp.hovered() {
                        let c = if i == 0 { crate::theme::hex(0xc42b1c) } else { egui::Color32::from_white_alpha(30) };
                        ui.painter().rect_filled(r, 0.0, c);
                    }
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, *glyph, egui::FontId::proportional(13.0), egui::Color32::WHITE);
                    if resp.clicked() {
                        match i {
                            0 => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                            1 => {
                                let m = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!m));
                            }
                            _ => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                        }
                    }
                    x -= w;
                }
            });
    }

    /// Resize from the window edges (the window draws its own frame).
    #[allow(dead_code)]
    fn edge_resize(&mut self, ctx: &egui::Context) {
        use egui::viewport::ResizeDirection as D;
        let rect = ctx.screen_rect();
        let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { return };
        let m = 5.0;
        let (l, r, t, b) = (p.x < rect.left() + m, p.x > rect.right() - m, p.y < rect.top() + m, p.y > rect.bottom() - m);
        let dir = match (l, r, t, b) {
            (true, _, true, _) => Some(D::NorthWest),
            (_, true, true, _) => Some(D::NorthEast),
            (true, _, _, true) => Some(D::SouthWest),
            (_, true, _, true) => Some(D::SouthEast),
            (true, ..) => Some(D::West),
            (_, true, ..) => Some(D::East),
            (_, _, true, _) => Some(D::North),
            (.., true) => Some(D::South),
            _ => None,
        };
        if let Some(d) = dir {
            let icon = match d {
                D::North | D::South => egui::CursorIcon::ResizeVertical,
                D::East | D::West => egui::CursorIcon::ResizeHorizontal,
                D::NorthWest | D::SouthEast => egui::CursorIcon::ResizeNwSe,
                _ => egui::CursorIcon::ResizeNeSw,
            };
            ctx.set_cursor_icon(icon);
            if ctx.input(|i| i.pointer.primary_pressed()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(d));
            }
        }
    }

    /// Page header bar (background #f1f1f1): page title on the left, app-bar icons on the right.
    fn header_bar(&mut self, ctx: &egui::Context) {
        let dark = self.dark;
        egui::TopBottomPanel::top("pagebar")
            .exact_height(50.0)
            .show_separator_line(false)
            .frame(egui::Frame::default().fill(crate::theme::page_bg(dark)).inner_margin(egui::Margin { left: 16, right: 10, top: 0, bottom: 0 }))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let more = crate::icons::button(ui, crate::icons::Icon::More, 18.0, "More");
                        crate::widgets::popup(ui, &more, "more_menu", |ui| {
                            if ui.button("Settings").clicked() {
                                self.tab = Tab::Settings;
                            }
                            if ui.button(if self.dark { "Light mode" } else { "Dark mode" }).clicked() {
                                self.settings.theme = if self.dark { crate::settings::ThemeChoice::Light } else { crate::settings::ThemeChoice::Dark };
                                self.settings.save();
                                self.update_theme();
                            }
                            if ui.button("Check for updates").clicked() {
                                self.start_catalog_update();
                            }
                        });
                        let hist = crate::icons::button(ui, crate::icons::Icon::History, 18.0, "History");
                        let recent = self.user.recent(15);
                        let mut go = None;
                        crate::widgets::popup(ui, &hist, "hist_menu", |ui| {
                            ui.set_min_width(280.0);
                            if recent.is_empty() {
                                ui.label("Nothing read yet");
                            }
                            for l in &recent {
                                let t = if l.title.is_empty() { l.key.clone() } else { l.title.clone() };
                                if ui.button(t).clicked() {
                                    go = Some(l.clone());
                                }
                            }
                        });
                        if let Some(l) = go {
                            self.tab = if l.key.starts_with("bible:") { Tab::Bible } else { Tab::Publications };
                            self.open_loc(&l, None);
                        }
                        if crate::icons::button(ui, crate::icons::Icon::Search, 18.0, "Search (Ctrl+F)").clicked() {
                            self.tab = Tab::Search;
                            self.search.focus = true;
                        }
                        // reading screen: its actions live here, in the header bar
                        let tab = self.tab;
                        if matches!(self.stacks.get(&tab).and_then(|s| s.last()), Some(Page::Reader(_))) {
                            let idx = self.stacks.get(&tab).map(|s| s.len() - 1).unwrap_or(0);
                            let mut page = self.stacks.get_mut(&tab).unwrap().pop().unwrap();
                            if let Page::Reader(rd) = &mut page {
                                ui.add_space(6.0);
                                crate::reader::header_actions(self, ui, rd);
                            }
                            let s = self.stacks.entry(tab).or_default();
                            if s.len() >= idx {
                                s.insert(idx, page);
                            }
                        }
                        let on_books = matches!(self.stacks.get(&Tab::Bible).and_then(|s| s.last()), Some(Page::BibleBooks));
                        if self.tab == Tab::Bible && on_books && !self.settings.bibles.is_empty() {
                            ui.add_space(4.0);
                            self.bible_menu_button(ui);
                        }
                        // the title takes what is left, shortened with … when needed
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if self.can_go_back() && crate::icons::button(ui, crate::icons::Icon::Back, 16.0, "Back (Alt+Left)").clicked() {
                            self.back();
                        }
                        let t = self.page_title.clone();
                        if self.page_subtitle.is_empty() {
                            ui.add(egui::Label::new(egui::RichText::new(t).font(crate::fonts::bold(16.0)).color(crate::theme::text(dark))).truncate());
                        } else {
                            let st = self.page_subtitle.clone();
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.add_space(6.0);
                                ui.add(egui::Label::new(egui::RichText::new(t).font(crate::fonts::bold(15.0)).color(crate::theme::text(dark))).truncate());
                                ui.label(egui::RichText::new(st).size(12.0).color(crate::theme::text(dark)));
                            });
                        }
                        });
                    });
                });
            });
    }

    /// Left navigation rail (background_navigation #d8d8d8): icons only, the hamburger opens labels.
    fn sidebar(&mut self, ctx: &egui::Context) {
        let dark = self.dark;
        let w = if self.nav_compact { 48.0 } else { 260.0 };
        egui::SidePanel::left("nav")
            .exact_width(w)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::default().fill(if dark { crate::theme::hex(0x292929) } else { crate::theme::hex(0xd9d9d9) }))
            .show(ctx, |ui| {
                use crate::icons::Icon;
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add_space(4.0);
                if self.nav_item(ui, Icon::Menu, "", false) {
                    self.nav_compact = !self.nav_compact;
                }
                ui.add_space(4.0);
                for (t, icon) in [(Tab::Home, Icon::Home), (Tab::Bible, Icon::Bible), (Tab::Publications, Icon::Library), (Tab::Meetings, Icon::Meetings), (Tab::Study, Icon::Study)] {
                    let selected = self.tab == t || (t == Tab::Publications && self.tab == Tab::Media);
                    let label = if t == Tab::Publications { "Library" } else { t.label() };
                    if self.nav_item(ui, icon, label, selected) {
                        let target = if t == Tab::Publications && self.tab != Tab::Media && self.tab != Tab::Publications { self.library_tab } else { t };
                        if self.tab == target {
                            self.stacks.insert(target, vec![target.root()]);
                            self.stacks_reset = true;
                        }
                        self.tab = target;
                    }
                }
                if !self.nav_compact {
                    ui.add_space(6.0);
                    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
                    ui.painter().rect_filled(r.shrink2(egui::vec2(12.0, 0.0)), 0.0, crate::theme::separator(dark));
                    ui.add_space(6.0);
                    let lang = self.settings.lang.clone();
                    for (icon, label, url) in [
                        (Icon::Globe, "JW.ORG", format!("https://www.jw.org/finder?wtlocale={lang}")),
                        (Icon::OnlineLibrary, "Online Library", format!("https://wol.jw.org/wol/finder?wtlocale={lang}")),
                        (Icon::Broadcast, "JW Broadcasting", format!("https://www.jw.org/finder?wtlocale={lang}&docid=1011214")),
                        (Icon::Donate, "Donations", "https://donate.jw.org?referer=jwlibrary".to_string()),
                    ] {
                        if self.nav_item(ui, icon, label, false) {
                            crate::paths::open_external(&url);
                        }
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_space(4.0);
                    if self.nav_item(ui, Icon::Settings, "Settings", self.tab == Tab::Settings) {
                        self.tab = Tab::Settings;
                    }
                    if !self.nav_compact {
                        if let Some(p) = &self.catalog_job {
                            let f = p.fraction();
                            ui.add(egui::ProgressBar::new(f).desired_width(230.0).text(egui::RichText::new(format!("Updating library {:.0}%", f * 100.0)).size(11.0)));
                        }
                    } else if self.catalog_job.is_some() || !self.downloads.is_empty() {
                        ui.spinner();
                    }
                });
            });
    }

    /// One navigation entry: outline icon (+ label when open); selected = lavender-grey background
    /// with a purple bar on the left, as in the original.
    fn nav_item(&mut self, ui: &mut egui::Ui, icon: crate::icons::Icon, label: &str, selected: bool) -> bool {
        let dark = self.dark;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click());
        if selected {
            ui.painter().rect_filled(rect, 0.0, if dark { crate::theme::hex(0x3d3550) } else { crate::theme::hex(0xc2bcd1) });
            ui.painter().rect_filled(egui::Rect::from_min_size(rect.left_top(), egui::vec2(3.0, rect.height())), 0.0, crate::theme::accent(dark));
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 0.0, if dark { crate::theme::hex(0x3c3c3c) } else { crate::theme::hex(0xcacaca) });
        }
        let col = if selected { crate::theme::accent(dark) } else { crate::theme::icon(dark) };
        let fill = if selected { Some(if dark { crate::theme::hex(0x2a1048) } else { crate::theme::PURPLE_50 }) } else { None };
        crate::icons::paint(ui.painter(), crate::icons::rect_at(egui::pos2(rect.left() + 24.0, rect.center().y), 20.0), icon, col, fill);
        if !self.nav_compact && !label.is_empty() {
            ui.painter().text(egui::pos2(rect.left() + 52.0, rect.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(14.5), crate::theme::text(dark));
        }
        let resp = if self.nav_compact && !label.is_empty() { resp.on_hover_text(label) } else { resp };
        resp.clicked()
    }

    fn central(&mut self, ctx: &egui::Context) {
        self.stacks_reset = false;
        let tab = self.tab;
        let mut stack = std::mem::take(self.stacks.entry(tab).or_default());
        if stack.is_empty() {
            stack.push(tab.root());
        }
        let depth = stack.len();
        let mut page = stack.pop().unwrap();
        self.cur_depth = depth;
        self.stacks.insert(tab, stack);
        let reading = matches!(page, Page::Reader(_));
        let dark = self.dark;
        // content area: rounded top-left corner over the navigation colour, as in the WinUI NavigationView
        let edge = matches!(page, Page::Home | Page::BibleBooks | Page::BibleSelect | Page::PubCategories | Page::MediaRoot);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(if reading { crate::theme::content_bg(dark) } else { crate::theme::page_bg(dark) })
                    .inner_margin(if edge { egui::Margin::ZERO } else { egui::Margin { left: 24, right: 24, top: 6, bottom: 12 } }),
            )
            .show(ctx, |ui| {
                self.page_subtitle.clear();
                if matches!(page, Page::Home) {
                    self.page_title = "Home".into();
                }
                match &mut page {
                    Page::Home => self.ui_home(ui),
                    Page::BibleBooks => self.ui_bible_books(ui),
                    Page::BibleSelect => self.ui_bible_select(ui, true),
                    Page::BibleChapters(b) => {
                        let b = *b;
                        self.ui_bible_chapters(ui, b)
                    }
                    Page::Reader(rd) => crate::reader::ui(self, ui, rd, depth > 1),
                    Page::PubCategories => self.ui_library(ui),
                    Page::PubList(t) => {
                        let t = *t;
                        self.ui_pub_list(ui, t)
                    }
                    Page::Toc(p) => {
                        let p = p.clone();
                        self.ui_toc(ui, &p)
                    }
                    Page::WolList(u, t) => {
                        let (u, t) = (u.clone(), t.clone());
                        self.ui_wol_list(ui, &u, &t)
                    }
                    Page::MediaRoot => {
                        if self.library_view == 0 {
                            self.library_view = 1;
                        }
                        self.ui_library(ui)
                    }
                    Page::MediaCategory(k, n) => {
                        let (k, n) = (k.clone(), n.clone());
                        self.ui_media_category(ui, &k, &n)
                    }
                    Page::Meetings => self.ui_meetings(ui),
                    Page::Study => self.ui_study(ui),
                    Page::Search => self.ui_search(ui),
                    Page::Settings => self.ui_settings(ui),
                }
            });
        // put the page back under anything it pushed
        if !self.stacks_reset {
            let s = self.stacks.entry(tab).or_default();
            if s.len() >= depth - 1 {
                let pushed: Vec<Page> = s.drain(depth - 1..).collect();
                if !self.pending_pop {
                    s.push(page);
                }
                s.extend(pushed);
            }
        }
        self.stacks_reset = false;
        self.pending_pop = false;
    }
}

// pending_pop lives outside the struct literal above for readability
impl App {
    pub fn request_back(&mut self) {
        self.pending_pop = true;
    }
}

pub fn monday_of(d: chrono::NaiveDate) -> chrono::NaiveDate {
    use chrono::Datelike;
    d - chrono::Duration::days(d.weekday().num_days_from_monday() as i64)
}

pub fn fxhash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn pick_image(v: &serde_json::Value) -> String {
    for (kind, size) in [("wss", "sm"), ("lss", "sm"), ("pnr", "sm"), ("sqr", "sm"), ("wss", "lg"), ("lss", "lg"), ("pnr", "lg"), ("sqr", "md"), ("cvr", "sm")] {
        if let Some(u) = v.get(kind).and_then(|k| k.get(size)).and_then(|u| u.as_str()) {
            return u.to_string();
        }
    }
    String::new()
}

fn parse_media(m: &serde_json::Value) -> Option<MediaItem> {
    let files = m
        .get("files")
        .and_then(|f| f.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    Some((
                        f.get("label").and_then(|l| l.as_str()).unwrap_or("").to_string(),
                        f.get("progressiveDownloadURL")?.as_str()?.to_string(),
                        f.get("filesize").and_then(|s| s.as_u64()).unwrap_or(0),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(MediaItem {
        key: m.get("naturalKey").and_then(|k| k.as_str()).unwrap_or("").to_string(),
        title: m.get("title")?.as_str()?.to_string(),
        duration: m.get("durationFormattedMinSec").and_then(|d| d.as_str()).unwrap_or("").to_string(),
        image: m.get("images").map(pick_image).unwrap_or_default(),
        kind: m.get("type").and_then(|t| t.as_str()).unwrap_or("video").to_string(),
        files,
    })
}

pub fn parse_category(c: &serde_json::Value) -> Option<MediaCategory> {
    Some(MediaCategory {
        key: c.get("key")?.as_str()?.to_string(),
        name: c.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
        description: c.get("description").and_then(|n| n.as_str()).unwrap_or("").to_string(),
        image: c.get("images").map(pick_image).unwrap_or_default(),
        banner: c
            .get("images")
            .and_then(|v| {
                [("pnr", "lg"), ("lss", "lg"), ("wss", "lg"), ("pnr", "md"), ("wss", "sm")]
                    .iter()
                    .find_map(|(k, s)| v.get(*k).and_then(|x| x.get(*s)).and_then(|u| u.as_str()).map(|u| u.to_string()))
            })
            .unwrap_or_default(),
        subcats: c
            .get("subcategories")
            .and_then(|s| s.as_array())
            .map(|a| a.iter().filter_map(parse_category).collect())
            .unwrap_or_default(),
        media: c
            .get("media")
            .and_then(|s| s.as_array())
            .map(|a| a.iter().filter_map(parse_media).collect())
            .unwrap_or_default(),
    })
}

pub fn parse_meetings(html: &str) -> Vec<MeetingSection> {
    let doc = scraper::Html::parse_document(html);
    let nav = scraper::Selector::parse("#materialNav").unwrap();
    let mut out: Vec<MeetingSection> = vec![];
    let Some(root) = doc.select(&nav).next() else { return out };
    let h2 = scraper::Selector::parse("h2, li.todayItem a, li a.cardContainer").unwrap();
    let line1 = scraper::Selector::parse(".cardLine1").unwrap();
    let line2 = scraper::Selector::parse(".cardLine2").unwrap();
    let clean = |s: String| s.split_whitespace().collect::<Vec<_>>().join(" ");
    for el in root.select(&h2) {
        if el.value().name() == "h2" {
            out.push(MeetingSection { title: clean(el.text().collect()), items: vec![] });
            continue;
        }
        let href = el.value().attr("href").unwrap_or("").to_string();
        if href.is_empty() {
            continue;
        }
        let l1 = el.select(&line1).next().map(|e| clean(e.text().collect())).unwrap_or_default();
        let l2 = el.select(&line2).next().map(|e| clean(e.text().collect())).unwrap_or_default();
        let symbol = el.value().classes().find_map(|c| c.strip_prefix("pub-").map(|s| s.to_string())).unwrap_or_default();
        let img_sel = scraper::Selector::parse("img").unwrap();
        let thumb = el
            .select(&img_sel)
            .next()
            .and_then(|i| i.value().attr("src").or_else(|| i.value().attr("data-img-src")))
            .map(|u| if u.starts_with('/') { format!("{}{}", crate::net::WOL, u) } else { u.to_string() })
            .unwrap_or_default();
        if out.is_empty() {
            out.push(MeetingSection { title: String::new(), items: vec![] });
        }
        let sec = out.last_mut().unwrap();
        if !sec.items.iter().any(|i| i.href == href) {
            sec.items.push(MeetingItem { line1: l1, line2: l2, href, symbol, thumb });
        }
    }
    out.retain(|s| !s.items.is_empty());
    out
}

impl eframe::App for App {
    /// Faster scrolling: every wheel/touchpad step is multiplied by the Scrolling speed setting.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        let k = self.settings.scroll_speed.clamp(0.5, 8.0);
        for e in raw.events.iter_mut() {
            if let egui::Event::MouseWheel { delta, modifiers, .. } = e {
                if !modifiers.ctrl && !modifiers.command {
                    *delta *= k;
                }
            }
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        self.handle_msgs();
        self.pump_images();
        if self.settings.theme == ThemeChoice::System && self.frame % 120 == 1 {
            self.update_theme();
        }
        // Keyboard / mouse back
        let back = ctx.input(|i| {
            (i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft))
                || i.pointer.button_pressed(egui::PointerButton::Extra1)
                || (i.key_pressed(egui::Key::Escape) && !i.focused)
        });
        if back && self.note_editor.is_none() && self.media_popup.is_none() && !self.player.as_ref().map(|p| p.expanded).unwrap_or(false) {
            self.back();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
            self.tab = Tab::Search;
            self.search.focus = true;
        }
        if let Some((b, c)) = self.pending_bible {
            if self.bible_index().is_some() {
                self.pending_bible = None;
                self.open_bible(b, c, None);
            } else if self.settings.bibles.is_empty() {
                let l = self.settings.lang.clone();
                self.load_bible_list(&l);
            }
        }
        crate::ui_player::player_ui(self, ctx);
        self.sidebar(ctx);
        let before_title = self.page_title.clone();
        self.header_bar(ctx);
        if matches!(self.tab, Tab::Publications | Tab::Media) {
            self.library_tab = self.tab;
        }
        self.central(ctx);
        if self.page_title != before_title {
            ctx.request_repaint();
        }
        crate::ui_bible::add_bible_window(self, ctx);
        crate::ui_player::fallback_window(self, ctx);
        crate::ui_study::note_editor_window(self, ctx);
        crate::ui_media::media_popup(self, ctx);
        crate::ui_media::playlist_picker(self, ctx);
        // toast
        let now = ctx.input(|i| i.time);
        if let Some((msg, until)) = self.toast.clone() {
            if now > until {
                self.toast = None;
            } else {
                egui::Area::new(egui::Id::new("toast"))
                    .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(100.0, -24.0))
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::default()
                            .fill(egui::Color32::from_rgb(0x33, 0x33, 0x33))
                            .corner_radius(8.0)
                            .inner_margin(egui::Margin::symmetric(16, 10))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(msg).color(egui::Color32::WHITE));
                            });
                    });
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
        }
        if self.catalog_job.is_some() || !self.downloads.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        // --screenshot: draw about 120 frames, save the picture, quit.
        if let Some(path) = self.screenshot.clone() {
            ctx.request_repaint();
            let at = std::env::var("STUDYLIB_SHOT_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
            if self.frame >= at && !self.shot_requested {
                self.shot_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = shot {
                let [w, h] = img.size;
                let mut buf = Vec::with_capacity(w * h * 4);
                for p in &img.pixels {
                    buf.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
                }
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Some(im) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                    if let Err(e) = im.save(&path) {
                        eprintln!("could not save screenshot: {e}");
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if self.frame > 3000 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}
