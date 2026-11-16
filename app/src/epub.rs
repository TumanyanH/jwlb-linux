//! Reading EPUB publications (the official free downloads).

use std::collections::HashMap;
use std::io::Read;

#[derive(Clone, Debug)]
pub struct TocEntry {
    pub title: String,
    pub href: String,
    pub level: usize,
}

#[derive(Clone, Debug)]
pub struct BibleBookIdx {
    pub num: u8,
    pub name: String,
    pub abbr: String,
    pub chapters: Vec<String>,
}

pub struct Epub {
    pub path: std::path::PathBuf,
    zip: zip::ZipArchive<std::io::BufReader<std::fs::File>>,
    pub title: String,
    pub spine: Vec<String>,
    pub toc: Vec<TocEntry>,
    pub bible: Option<Vec<BibleBookIdx>>,
    pub cover: Option<String>,
}

pub fn resolve(base_file: &str, href: &str) -> String {
    let href = href.split('#').next().unwrap_or("");
    if href.is_empty() {
        return base_file.to_string();
    }
    let mut parts: Vec<&str> = base_file.split('/').collect();
    parts.pop();
    for seg in href.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", "\u{a0}").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
}

impl Epub {
    pub fn open(path: &std::path::Path) -> Result<Epub, String> {
        let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let zip = zip::ZipArchive::new(std::io::BufReader::new(f)).map_err(|e| e.to_string())?;
        let mut e = Epub {
            path: path.to_path_buf(),
            zip,
            title: String::new(),
            spine: vec![],
            toc: vec![],
            bible: None,
            cover: None,
        };
        e.load_index()?;
        Ok(e)
    }

    pub fn read_bytes(&mut self, name: &str) -> Option<Vec<u8>> {
        let mut f = self.zip.by_name(name).ok()?;
        let mut v = Vec::new();
        f.read_to_end(&mut v).ok()?;
        Some(v)
    }

    pub fn read_text(&mut self, name: &str) -> Option<String> {
        self.read_bytes(name).map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    fn load_index(&mut self) -> Result<(), String> {
        let container = self.read_text("META-INF/container.xml").ok_or("not an EPUB file")?;
        let doc = roxmltree::Document::parse(&container).map_err(|e| e.to_string())?;
        let opf_path = doc
            .descendants()
            .find(|n| n.has_tag_name("rootfile"))
            .and_then(|n| n.attribute("full-path"))
            .ok_or("EPUB has no package file")?
            .to_string();
        let opf = self.read_text(&opf_path).ok_or("EPUB package file missing")?;
        let opf_doc = roxmltree::Document::parse(&opf).map_err(|e| e.to_string())?;
        let mut manifest: HashMap<String, (String, String)> = HashMap::new();
        let mut nav_href = None;
        let mut ncx_href = None;
        for n in opf_doc.descendants() {
            if n.tag_name().name() == "title" && self.title.is_empty() {
                self.title = n.text().unwrap_or("").trim().to_string();
            }
            if n.tag_name().name() == "item" {
                let id = n.attribute("id").unwrap_or("").to_string();
                let href = resolve(&opf_path, n.attribute("href").unwrap_or(""));
                let mt = n.attribute("media-type").unwrap_or("").to_string();
                let props = n.attribute("properties").unwrap_or("");
                if props.split(' ').any(|p| p == "nav") {
                    nav_href = Some(href.clone());
                }
                if props.contains("cover-image") {
                    self.cover = Some(href.clone());
                }
                if mt == "application/x-dtbncx+xml" {
                    ncx_href = Some(href.clone());
                }
                manifest.insert(id, (href, mt));
            }
        }
        for n in opf_doc.descendants().filter(|n| n.tag_name().name() == "itemref") {
            if let Some((href, mt)) = n.attribute("idref").and_then(|i| manifest.get(i)) {
                if mt.contains("html") {
                    self.spine.push(href.clone());
                }
            }
        }
        if let Some(nav) = nav_href {
            if let Some(t) = self.read_text(&nav) {
                self.toc = parse_nav(&nav, &t);
            }
        }
        if self.toc.is_empty() {
            if let Some(ncx) = ncx_href {
                if let Some(t) = self.read_text(&ncx) {
                    self.toc = parse_ncx(&ncx, &t);
                }
            }
        }
        // Skip navigation helper pages (Bible verse/chapter grids) from the table of contents.
        self.toc.retain(|t| !t.href.contains("bibleversenav") && !t.href.contains("biblechapternav"));
        self.load_bible_index();
        Ok(())
    }

    fn load_bible_index(&mut self) {
        let Some(booknav) = self.spine.iter().find(|s| s.ends_with("biblebooknav.xhtml")).cloned().or_else(|| {
            let names: Vec<String> = self.zip.file_names().map(|s| s.to_string()).collect();
            names.into_iter().find(|n| n.ends_with("biblebooknav.xhtml"))
        }) else {
            return;
        };
        let Some(t) = self.read_text(&booknav) else { return };
        let html = scraper::Html::parse_document(&t);
        let sel = scraper::Selector::parse("a").unwrap();
        let mut books = vec![];
        for a in html.select(&sel) {
            let href = a.value().attr("href").unwrap_or("");
            let Some(num) = href
                .rsplit('/')
                .next()
                .and_then(|f| f.strip_prefix("biblechapternav"))
                .and_then(|f| f.strip_suffix(".xhtml"))
                .and_then(|n| n.parse::<u8>().ok())
            else {
                continue;
            };
            let abbr = a.text().collect::<String>().trim().to_string();
            let nav = resolve(&booknav, href);
            let mut name = abbr.clone();
            let mut chapters = vec![];
            if let Some(ct) = self.read_text(&nav) {
                let ch = scraper::Html::parse_document(&ct);
                if let Some(h) = ch.select(&scraper::Selector::parse("h2").unwrap()).next() {
                    let n = h.text().collect::<String>().trim().to_string();
                    if !n.is_empty() {
                        name = n;
                    }
                }
                for ca in ch.select(&sel) {
                    let h = ca.value().attr("href").unwrap_or("");
                    if h.contains("biblebooknav") {
                        continue;
                    }
                    if ca.text().collect::<String>().trim().parse::<u16>().is_ok() {
                        chapters.push(resolve(&nav, h));
                    }
                }
            }
            if chapters.is_empty() {
                // single-chapter books link straight to the text
                continue;
            }
            books.push(BibleBookIdx { num, name, abbr, chapters });
        }
        if !books.is_empty() {
            books.sort_by_key(|b| b.num);
            self.bible = Some(books);
        }
    }

    /// Find a spine document containing a MEPS document id (body class docId-N).
    pub fn find_doc(&mut self, doc_id: i64) -> Option<String> {
        let needle = format!("docId-{doc_id}");
        let spine = self.spine.clone();
        for s in spine {
            if let Some(t) = self.read_text(&s) {
                if t.contains(&needle) {
                    return Some(s);
                }
            }
        }
        None
    }

    pub fn toc_title_for(&self, href: &str) -> Option<String> {
        self.toc.iter().find(|t| t.href.split('#').next() == Some(href)).map(|t| t.title.clone())
    }
}

fn parse_nav(base: &str, text: &str) -> Vec<TocEntry> {
    let html = scraper::Html::parse_document(text);
    let nav_sel = scraper::Selector::parse("nav").unwrap();
    let a_sel = scraper::Selector::parse("a").unwrap();
    let mut out = vec![];
    let navs: Vec<_> = html.select(&nav_sel).collect();
    let nav = navs
        .iter()
        .find(|n| n.value().attrs().any(|(k, v)| k.ends_with("type") && v == "toc"))
        .or(navs.first());
    let Some(nav) = nav else { return out };
    for a in nav.select(&a_sel) {
        let mut level = 0;
        let mut p = a.parent();
        while let Some(n) = p {
            if let Some(el) = n.value().as_element() {
                if el.name() == "ol" || el.name() == "ul" {
                    level += 1;
                }
                if el.name() == "nav" {
                    break;
                }
            }
            p = n.parent();
        }
        let title = a.text().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
        let href = a.value().attr("href").unwrap_or("");
        let frag = href.split('#').nth(1);
        let mut full = resolve(base, href);
        if let Some(f) = frag {
            full = format!("{full}#{f}");
        }
        if !title.is_empty() {
            out.push(TocEntry { title: decode_entities(&title), href: full, level: level.max(1) - 1 });
        }
    }
    out
}

fn parse_ncx(base: &str, text: &str) -> Vec<TocEntry> {
    let Ok(doc) = roxmltree::Document::parse(text) else { return vec![] };
    let mut out = vec![];
    fn walk(n: roxmltree::Node, base: &str, level: usize, out: &mut Vec<TocEntry>) {
        for c in n.children().filter(|c| c.tag_name().name() == "navPoint") {
            let title = c
                .descendants()
                .find(|d| d.tag_name().name() == "text")
                .and_then(|d| d.text())
                .unwrap_or("")
                .trim()
                .to_string();
            let src = c
                .children()
                .find(|d| d.tag_name().name() == "content")
                .and_then(|d| d.attribute("src"))
                .unwrap_or("");
            let mut full = resolve(base, src);
            if let Some(f) = src.split('#').nth(1) {
                full = format!("{full}#{f}");
            }
            out.push(TocEntry { title, href: full, level });
            walk(c, base, level + 1, out);
        }
    }
    if let Some(map) = doc.descendants().find(|n| n.tag_name().name() == "navMap") {
        walk(map, base, 0, &mut out);
    }
    out
}
