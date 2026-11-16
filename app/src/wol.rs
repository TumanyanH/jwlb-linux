//! The online library (wol.jw.org): Bibles in every language, publications that have no EPUB,
//! meeting material. Every page fetched is kept on disk, so what has been downloaded or read
//! opens again without internet.

use crate::net::{WolLocale, WOL};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

fn cache_path(url: &str) -> std::path::PathBuf {
    crate::paths::sub("library-pages").join(format!("{:016x}.html", crate::app::fxhash(url)))
}

pub fn is_cached(url: &str) -> bool {
    cache_path(url).exists()
}

/// A page from the online library: from disk when it has been saved before.
pub fn fetch(url: &str) -> Result<String, String> {
    let p = cache_path(url);
    if let Ok(s) = std::fs::read_to_string(&p) {
        return Ok(s);
    }
    let s = crate::net::get_string(url)?;
    let _ = std::fs::write(&p, &s);
    Ok(s)
}

pub fn full_url(href: &str) -> String {
    if href.starts_with("http") {
        href.to_string()
    } else {
        format!("{WOL}{href}")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BibleRef {
    pub lang: String,
    pub symbol: String,
    pub title: String,
}

impl BibleRef {
    pub fn key(&self) -> String {
        format!("{}|{}", self.lang, self.symbol)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BookIdx {
    pub num: u8,
    pub name: String,
    pub abbr: String,
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Bible translations the online library has in a language: (symbol, title).
pub fn bibles(lang: &str) -> Result<Vec<(String, String)>, String> {
    let w = crate::net::wol_locale(lang)?;
    let base = format!("/{}/wol/binav/{}/{}/", w.locale, w.r, w.lp);
    let html = fetch(&format!("{WOL}/{}/wol/binav/{}/{}", w.locale, w.r, w.lp))?;
    let doc = scraper::Html::parse_document(&html);
    let sel = scraper::Selector::parse("a").unwrap();
    let mut out: Vec<(String, String)> = vec![];
    for a in doc.select(&sel) {
        let href = a.value().attr("href").unwrap_or("");
        let Some(rest) = href.strip_prefix(&base) else { continue };
        if rest.is_empty() || rest.contains('/') {
            continue;
        }
        let title = clean(&a.text().collect::<String>());
        if !out.iter().any(|(s, _)| s == rest) && !title.is_empty() {
            out.push((rest.to_string(), title));
        }
    }
    // The library's Bible list leaves out some editions (for example the 2024 Armenian New World
    // Translation, "nwt"), so the known editions are also checked directly, all at once.
    let known = ["nwt", "nwtsty", "bi12", "bi7", "bi10", "Rbi8", "int", "by", "bi22", "rh"];
    let missing: Vec<&str> = known.iter().copied().filter(|k| !out.iter().any(|(s, _)| s == k)).collect();
    let hs: Vec<_> = missing
        .into_iter()
        .map(|sym| {
            let w = w.clone();
            std::thread::spawn(move || -> Option<(String, String)> {
                let test = format!("{WOL}/{}/wol/b/{}/{}/{}/43/1", w.locale, w.r, w.lp, sym);
                let page = crate::net::get_string(&test).ok()?;
                if !page.contains(&format!("pub-{sym}")) {
                    return None;
                }
                let pubpage = crate::net::get_string(&format!("{WOL}/{}/wol/publication/{}/{}/{}", w.locale, w.r, w.lp, sym)).unwrap_or_default();
                let title = pubpage
                    .split("<title>")
                    .nth(1)
                    .and_then(|t| t.split("&mdash;").next())
                    .map(|t| clean(&t.replace(&format!("({sym})"), "")))
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| match sym {
                        "nwt" => "New World Translation".into(),
                        "nwtsty" => "New World Translation (Study Edition)".into(),
                        _ => sym.to_string(),
                    });
                Some((sym.to_string(), title))
            })
        })
        .collect();
    let mut extra: Vec<(String, String)> = hs.into_iter().filter_map(|h| h.join().ok().flatten()).collect();
    // newest New World Translation first
    extra.sort_by_key(|(s, _)| known.iter().position(|k| k == s).unwrap_or(99));
    for (i, e) in extra.into_iter().enumerate() {
        out.insert(i.min(out.len()), e);
    }
    Ok(out)
}

/// The books of a Bible with their names in its language.
pub fn books(b: &BibleRef) -> Result<Vec<BookIdx>, String> {
    let cache = crate::paths::cache_dir().join(format!("books_{}_{}.json", crate::paths::safe_name(&b.lang), crate::paths::safe_name(&b.symbol)));
    if let Ok(s) = std::fs::read_to_string(&cache) {
        if let Ok(v) = serde_json::from_str::<Vec<BookIdx>>(&s) {
            if !v.is_empty() {
                return Ok(v);
            }
        }
    }
    let w = crate::net::wol_locale(&b.lang)?;
    let base = format!("/{}/wol/binav/{}/{}/{}/", w.locale, w.r, w.lp, b.symbol);
    let html = fetch(&format!("{WOL}/{}/wol/binav/{}/{}/{}", w.locale, w.r, w.lp, b.symbol))?;
    let doc = scraper::Html::parse_document(&html);
    let sel = scraper::Selector::parse("a").unwrap();
    let span = scraper::Selector::parse("span").unwrap();
    let mut out: Vec<BookIdx> = vec![];
    for a in doc.select(&sel) {
        let href = a.value().attr("href").unwrap_or("");
        let Some(rest) = href.strip_prefix(&base) else { continue };
        let Ok(num) = rest.parse::<u8>() else { continue };
        if out.iter().any(|x| x.num == num) {
            continue;
        }
        let parts: Vec<String> = a.select(&span).map(|s| clean(&s.text().collect::<String>())).filter(|s| !s.is_empty()).collect();
        let (name, abbr) = if parts.len() >= 2 {
            (parts[0].clone(), parts[1].clone())
        } else {
            let t = clean(&a.text().collect::<String>());
            let mut it = t.split("  ");
            let n = it.next().unwrap_or("").to_string();
            (n.clone(), it.next().map(|x| x.to_string()).unwrap_or(n))
        };
        let fallback = crate::bible::BOOKS.get(num as usize - 1);
        out.push(BookIdx {
            num,
            name: if name.is_empty() { fallback.map(|f| f.name.to_string()).unwrap_or_default() } else { name },
            abbr: if abbr.is_empty() { fallback.map(|f| f.abbr.to_string()).unwrap_or_default() } else { abbr },
        });
    }
    if out.is_empty() {
        return Err("This Bible has no books in the online library".into());
    }
    out.sort_by_key(|b| b.num);
    let _ = std::fs::write(&cache, serde_json::to_string(&out).unwrap_or_default());
    Ok(out)
}

pub fn chapter_url(w: &WolLocale, b: &BibleRef, book: u8, chapter: u16) -> String {
    format!("{WOL}/{}/wol/b/{}/{}/{}/{}/{}", w.locale, w.r, w.lp, b.symbol, book, chapter)
}

pub fn chapter_count(book: u8) -> u16 {
    crate::bible::BOOKS.get(book as usize - 1).map(|b| b.chapters).unwrap_or(1)
}

/// Save every chapter of a Bible for offline reading.
pub fn download_bible(b: &BibleRef, done: Arc<AtomicU64>, total: Arc<AtomicU64>) -> Result<(), String> {
    let w = crate::net::wol_locale(&b.lang)?;
    let bks = books(b)?;
    let urls: Vec<String> = bks.iter().flat_map(|bk| (1..=chapter_count(bk.num)).map(move |c| (bk.num, c))).map(|(n, c)| chapter_url(&w, b, n, c)).collect();
    total.store(urls.len() as u64, Ordering::Relaxed);
    fetch_all(urls, done)
}

fn fetch_all(urls: Vec<String>, done: Arc<AtomicU64>) -> Result<(), String> {
    let queue = Arc::new(std::sync::Mutex::new(urls));
    let errors = Arc::new(AtomicU64::new(0));
    let mut hs = vec![];
    for _ in 0..4 {
        let q = queue.clone();
        let d = done.clone();
        let e = errors.clone();
        hs.push(std::thread::spawn(move || loop {
            let next = q.lock().unwrap().pop();
            let Some(u) = next else { break };
            let mut ok = fetch(&u).is_ok();
            if !ok {
                std::thread::sleep(std::time::Duration::from_millis(500));
                ok = fetch(&u).is_ok();
            }
            if !ok {
                e.fetch_add(1, Ordering::Relaxed);
            }
            d.fetch_add(1, Ordering::Relaxed);
        }));
    }
    for h in hs {
        let _ = h.join();
    }
    let n = errors.load(Ordering::Relaxed);
    if n > 20 {
        return Err(format!("{n} pages could not be downloaded; check the internet connection and try again"));
    }
    Ok(())
}

/// One entry of an online-library contents page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    pub title: String,
    pub sub: String,
    pub href: String,
    pub is_doc: bool,
}

pub fn cards(html: &str) -> Vec<Card> {
    let doc = scraper::Html::parse_document(html);
    let a_sel = scraper::Selector::parse("a.cardContainer").unwrap();
    let l1 = scraper::Selector::parse(".cardLine1").unwrap();
    let l2 = scraper::Selector::parse(".cardLine2").unwrap();
    let mut out: Vec<Card> = vec![];
    for a in doc.select(&a_sel) {
        let href = a.value().attr("href").unwrap_or("").to_string();
        if href.is_empty() || href.contains("/wol/h/") || out.iter().any(|c| c.href == href) {
            continue;
        }
        let is_doc = href.contains("/wol/d/");
        let is_list = href.contains("/wol/library/") || href.contains("/wol/publication/") || href.contains("/wol/tc/") || href.contains("/wol/lv/");
        if !is_doc && !is_list {
            continue;
        }
        let title = a.select(&l1).next().map(|e| clean(&e.text().collect::<String>())).unwrap_or_default();
        let sub = a.select(&l2).next().map(|e| clean(&e.text().collect::<String>())).unwrap_or_default();
        let title = if title.is_empty() { clean(&a.text().collect::<String>()) } else { title };
        out.push(Card { title, sub, href, is_doc });
    }
    out
}

/// Address of a publication's contents in the online library (via /wol/finder like the original).
pub fn publication_root(symbol: &str, issue: i64, lang: &str) -> Result<String, String> {
    let mut url = format!("{WOL}/wol/finder?wtlocale={}&pub={}", crate::net::urlencode(lang), crate::net::urlencode(symbol));
    if let Some(i) = crate::net::issue_param(issue) {
        url.push_str(&format!("&issue={i}"));
    }
    let (fin, body) = crate::net::get_with_final_url(&url)?;
    if fin.contains("/wol/h/") || cards(&body).is_empty() {
        return Err("This publication is not in the online library".into());
    }
    let _ = std::fs::write(cache_path(&fin), &body);
    Ok(fin)
}

/// Save a publication's pages (contents and every article) for offline reading.
pub fn download_publication(root: &str, done: Arc<AtomicU64>, total: Arc<AtomicU64>) -> Result<(), String> {
    let mut lists = vec![root.to_string()];
    let mut docs: Vec<String> = vec![];
    let mut seen = std::collections::HashSet::new();
    let mut depth = 0;
    while !lists.is_empty() && depth < 4 {
        let mut next = vec![];
        for l in lists {
            if !seen.insert(l.clone()) {
                continue;
            }
            let html = fetch(&l)?;
            for c in cards(&html) {
                let u = full_url(&c.href);
                if c.is_doc {
                    let u = u.split('#').next().unwrap_or(&u).to_string();
                    if !docs.contains(&u) {
                        docs.push(u);
                    }
                } else if !seen.contains(&u) && u.starts_with(root.split("/wol/").next().unwrap_or(WOL)) {
                    next.push(u);
                }
            }
            total.store((docs.len() + next.len()) as u64, Ordering::Relaxed);
        }
        lists = next;
        depth += 1;
        if docs.len() > 4000 {
            break;
        }
    }
    total.store(docs.len() as u64, Ordering::Relaxed);
    done.store(0, Ordering::Relaxed);
    fetch_all(docs, done)
}

/// Marker saved next to EPUB downloads for publications kept as online-library pages.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Installed {
    pub root: String,
    pub title: String,
}

/// Chapter recordings of a Bible: (book, chapter) -> (title, address). From pub-media (MP3),
/// as the original's audio downloads. The study edition's recordings are those of "nwt".
pub fn bible_audio(b: &BibleRef) -> Result<std::collections::HashMap<(u8, u16), (String, String)>, String> {
    let cache = crate::paths::cache_dir().join(format!("audio_{}_{}.json", crate::paths::safe_name(&b.lang), crate::paths::safe_name(&b.symbol)));
    if let Ok(s) = std::fs::read_to_string(&cache) {
        if let Ok(v) = serde_json::from_str::<Vec<(u8, u16, String, String)>>(&s) {
            return Ok(v.into_iter().map(|(bk, c, t, u)| ((bk, c), (t, u))).collect());
        }
    }
    let mut list: Vec<(u8, u16, String, String)> = vec![];
    for sym in [b.symbol.as_str(), "nwt"] {
        let url = format!(
            "{}?output=json&pub={}&fileformat=MP3&alllangs=0&langwritten={}",
            crate::net::PUB_MEDIA,
            crate::net::urlencode(sym),
            crate::net::urlencode(&b.lang)
        );
        if let Ok(v) = crate::net::get_json(&url) {
            if let Some(arr) = v.pointer(&format!("/files/{}/MP3", b.lang)).and_then(|a| a.as_array()) {
                for f in arr {
                    let bk = f.get("booknum").and_then(|x| x.as_u64()).unwrap_or(0) as u8;
                    let ch = f.pointer("/markers/bibleBookChapter").and_then(|x| x.as_u64()).unwrap_or(f.get("track").and_then(|x| x.as_u64()).unwrap_or(0)) as u16;
                    let u = f.pointer("/file/url").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let t = f.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    if bk > 0 && ch > 0 && !u.is_empty() {
                        list.push((bk, ch, t, u));
                    }
                }
            }
        }
        if !list.is_empty() || b.symbol != "nwtsty" {
            break;
        }
    }
    let _ = std::fs::write(&cache, serde_json::to_string(&list).unwrap_or_default());
    Ok(list.into_iter().map(|(bk, c, t, u)| ((bk, c), (t, u))).collect())
}

/// Audio tracks of a publication: (title, address).
pub fn pub_audio(symbol: &str, issue: i64, lang: &str) -> Vec<(String, String)> {
    let mut url = format!(
        "{}?output=json&pub={}&fileformat=MP3&alllangs=0&langwritten={}",
        crate::net::PUB_MEDIA,
        crate::net::urlencode(symbol),
        crate::net::urlencode(lang)
    );
    if let Some(i) = crate::net::issue_param(issue) {
        url.push_str(&format!("&issue={i}"));
    }
    let Ok(v) = crate::net::get_json(&url) else { return vec![] };
    v.pointer(&format!("/files/{lang}/MP3"))
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    let u = f.pointer("/file/url")?.as_str()?.to_string();
                    let t = f.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    (!u.ends_with(".zip")).then_some((t, u))
                })
                .collect()
        })
        .unwrap_or_default()
}
