//! Turns publication HTML (EPUB pages or online-library articles) into readable blocks.
//! Paragraph identifiers come from data-pid (BlockType 1) and verse numbers from
//! chapterN_verseM markers (BlockType 2), the same identifiers the original's notes and
//! highlights use (BlockRange.Identifier).

use scraper::{ElementRef, Html, Node};

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    H1,
    H2,
    H3,
    Para,
    Theme,
    Question,
    Item,
    Caption,
    Footnote,
    Image(String),
}

#[derive(Clone, Debug)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub bold: bool,
    pub italic: bool,
    pub sup: bool,
    pub link: Option<String>,
}

/// A part of a block that notes and highlights attach to.
#[derive(Clone, Debug)]
pub struct Seg {
    pub start: usize,
    pub end: usize,
    /// 1 = paragraph, 2 = Bible verse
    pub btype: i64,
    pub ident: i64,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub kind: Kind,
    pub text: String,
    pub spans: Vec<Span>,
    pub segs: Vec<Seg>,
    pub anchor: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub title: String,
    pub blocks: Vec<Block>,
    pub doc_id: Option<i64>,
    pub pub_symbol: Option<String>,
}

struct Builder {
    text: String,
    spans: Vec<Span>,
    segs: Vec<Seg>,
    bold: u32,
    italic: u32,
    sup: u32,
    link: Vec<String>,
    anchor: Option<String>,
}

impl Builder {
    fn new() -> Self {
        Builder { text: String::new(), spans: vec![], segs: vec![], bold: 0, italic: 0, sup: 0, link: vec![], anchor: None }
    }
    fn push(&mut self, s: &str) {
        let mut cleaned = String::with_capacity(s.len());
        let mut last_space = self.text.ends_with(' ') || self.text.is_empty();
        for ch in s.chars() {
            if ch.is_whitespace() && ch != '\u{a0}' {
                if !last_space {
                    cleaned.push(' ');
                    last_space = true;
                }
            } else {
                cleaned.push(ch);
                last_space = false;
            }
        }
        if cleaned.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(&cleaned);
        let end = self.text.len();
        self.spans.push(Span {
            start,
            end,
            bold: self.bold > 0,
            italic: self.italic > 0,
            sup: self.sup > 0,
            link: self.link.last().cloned(),
        });
    }
    fn start_seg(&mut self, btype: i64, ident: i64) {
        let pos = self.text.len();
        if let Some(last) = self.segs.last_mut() {
            last.end = pos;
            if last.start == pos {
                self.segs.pop();
            }
        }
        self.segs.push(Seg { start: pos, end: pos, btype, ident });
    }
}

fn verse_marker(id: &str) -> Option<(i64, i64)> {
    // online library: v{book}-{chapter}-{verse}-{part}
    if let Some(rest) = id.strip_prefix('v') {
        let p: Vec<&str> = rest.split('-').collect();
        if p.len() == 4 {
            return Some((p[1].parse().ok()?, p[2].parse().ok()?));
        }
    }
    let rest = id.strip_prefix("chapter")?;
    let (c, v) = rest.split_once("_verse")?;
    Some((c.parse().ok()?, v.parse().ok()?))
}

fn has_class(e: &ElementRef, c: &str) -> bool {
    e.value().classes().any(|x| x == c)
}

fn walk_inline(el: ElementRef, b: &mut Builder) {
    for child in el.children() {
        match child.value() {
            Node::Text(t) => b.push(t),
            Node::Element(e) => {
                let Some(ce) = ElementRef::wrap(child) else { continue };
                let name = e.name();
                if let Some(id) = e.attr("id") {
                    if let Some((_c, v)) = verse_marker(id) {
                        b.start_seg(2, v);
                    }
                }
                if has_class(&ce, "pageNum") || name == "script" || name == "style" {
                    continue;
                }
                let is_noteref = e.attrs().any(|(k, v)| k.ends_with("type") && v == "noteref");
                if is_noteref {
                    continue;
                }
                match name {
                    "br" => b.push(" "),
                    "img" => {}
                    "strong" | "b" => {
                        b.bold += 1;
                        walk_inline(ce, b);
                        b.bold -= 1;
                    }
                    "em" | "i" => {
                        b.italic += 1;
                        walk_inline(ce, b);
                        b.italic -= 1;
                    }
                    "sup" => {
                        b.sup += 1;
                        walk_inline(ce, b);
                        b.sup -= 1;
                    }
                    "a" => {
                        // online-library cross-reference (+) and footnote (*) markers are not text
                        let txt: String = ce.text().collect();
                        let t = txt.trim();
                        if (has_class(&ce, "b") || has_class(&ce, "fn") || has_class(&ce, "footnoteLink")) && (t == "+" || t == "*" || t.is_empty()) {
                            continue;
                        }
                        // online-library verse numbers
                        if has_class(&ce, "vl") {
                            b.sup += 1;
                            b.bold += 1;
                            walk_inline(ce, b);
                            b.bold -= 1;
                            b.sup -= 1;
                            continue;
                        }
                        if has_class(&ce, "cl") {
                            b.bold += 1;
                            walk_inline(ce, b);
                            b.bold -= 1;
                            continue;
                        }
                        let href = e.attr("href").unwrap_or("").to_string();
                        b.link.push(href);
                        walk_inline(ce, b);
                        b.link.pop();
                    }
                    _ => walk_inline(ce, b),
                }
            }
            _ => {}
        }
    }
}

fn block_kind(name: &str, e: &ElementRef) -> Option<Kind> {
    let cls = |c: &str| has_class(e, c);
    Some(match name {
        "h1" => Kind::H1,
        "h2" => Kind::H2,
        "h3" | "h4" | "h5" | "h6" => Kind::H3,
        "p" => {
            if cls("themeScrp") {
                Kind::Theme
            } else if cls("qu") {
                Kind::Question
            } else {
                Kind::Para
            }
        }
        "li" => Kind::Item,
        "figcaption" => Kind::Caption,
        _ => return None,
    })
}

struct Ctx<'a> {
    base: &'a str,
    blocks: Vec<Block>,
    in_footnote: bool,
    with_images: bool,
}

fn finish(ctx: &mut Ctx, kind: Kind, e: &ElementRef, mut b: Builder) {
    let pid: Option<i64> = e.value().attr("data-pid").and_then(|p| p.parse().ok());
    if let Some(last) = b.segs.last_mut() {
        last.end = b.text.len();
    }
    // Text before the first verse marker in a Bible paragraph belongs to that paragraph.
    if b.segs.is_empty() {
        if let Some(p) = pid {
            b.segs.push(Seg { start: 0, end: b.text.len(), btype: 1, ident: p });
        }
    } else if b.segs[0].start > 0 {
        if let Some(p) = pid {
            b.segs.insert(0, Seg { start: 0, end: b.segs[0].start, btype: 1, ident: p });
        }
    }
    let trimmed = b.text.trim_end().len();
    b.text.truncate(trimmed);
    for s in &mut b.spans {
        s.end = s.end.min(trimmed);
        s.start = s.start.min(s.end);
    }
    for s in &mut b.segs {
        s.end = s.end.min(trimmed);
        s.start = s.start.min(s.end);
    }
    if b.text.trim().is_empty() {
        return;
    }
    let kind = if ctx.in_footnote { Kind::Footnote } else { kind };
    ctx.blocks.push(Block { kind, text: b.text, spans: b.spans, segs: b.segs, anchor: b.anchor.or_else(|| e.value().attr("id").map(|s| s.to_string())) });
}

fn walk_blocks(el: ElementRef, ctx: &mut Ctx) {
    for child in el.children() {
        let Some(ce) = ElementRef::wrap(child) else {
            if let Node::Text(t) = child.value() {
                if !t.trim().is_empty() && el.value().name() != "body" {
                    let mut b = Builder::new();
                    b.push(t);
                    ctx.blocks.push(Block { kind: Kind::Para, text: b.text.trim().to_string(), spans: vec![], segs: vec![], anchor: None });
                }
            }
            continue;
        };
        let name = ce.value().name();
        if matches!(name, "script" | "style" | "nav" | "head" | "noscript" | "button" | "input" | "form") {
            continue;
        }
        if has_class(&ce, "w_navigation") || has_class(&ce, "pageNum") || has_class(&ce, "jsPinnedAudioPlayer") {
            continue;
        }
        if name == "img" {
            if ctx.with_images {
                if let Some(src) = ce.value().attr("src") {
                    let full = if src.starts_with("http") || src.starts_with('/') {
                        src.to_string()
                    } else {
                        crate::epub::resolve(ctx.base, src)
                    };
                    ctx.blocks.push(Block { kind: Kind::Image(full), text: String::new(), spans: vec![], segs: vec![], anchor: None });
                }
            }
            continue;
        }
        let footnote = ce.value().attrs().any(|(k, v)| k.ends_with("type") && v == "footnote") || has_class(&ce, "groupFootnote");
        if footnote {
            let was = ctx.in_footnote;
            ctx.in_footnote = true;
            walk_blocks(ce, ctx);
            ctx.in_footnote = was;
            continue;
        }
        if let Some(kind) = block_kind(name, &ce) {
            // A list item that wraps paragraphs is a container.
            if name == "li" && ce.children().any(|c| ElementRef::wrap(c).map(|x| x.value().name() == "p").unwrap_or(false)) {
                walk_blocks(ce, ctx);
                continue;
            }
            let mut b = Builder::new();
            walk_inline(ce, &mut b);
            finish(ctx, kind, &ce, b);
            // images nested inside a paragraph
            if ctx.with_images {
                let sel = scraper::Selector::parse("img").unwrap();
                for img in ce.select(&sel) {
                    if let Some(src) = img.value().attr("src") {
                        let full = if src.starts_with("http") || src.starts_with('/') { src.to_string() } else { crate::epub::resolve(ctx.base, src) };
                        ctx.blocks.push(Block { kind: Kind::Image(full), text: String::new(), spans: vec![], segs: vec![], anchor: None });
                    }
                }
            }
            continue;
        }
        walk_blocks(ce, ctx);
    }
}

fn doc_meta(html: &Html) -> (Option<i64>, Option<String>) {
    let mut doc_id = None;
    let mut sym = None;
    for sel in ["body", "article", "#article", "div.docClass"] {
        let Ok(s) = scraper::Selector::parse(sel) else { continue };
        for e in html.select(&s) {
            for c in e.value().classes() {
                if doc_id.is_none() {
                    if let Some(n) = c.strip_prefix("docId-") {
                        doc_id = n.parse().ok();
                    }
                }
                if sym.is_none() {
                    if let Some(p) = c.strip_prefix("pub-") {
                        sym = Some(p.to_string());
                    }
                }
            }
        }
    }
    (doc_id, sym)
}

/// Parse an EPUB page. `base` is the page's path inside the EPUB (for pictures).
pub fn parse_epub_page(base: &str, text: &str, with_images: bool) -> Doc {
    let html = Html::parse_document(text);
    let title = html
        .select(&scraper::Selector::parse("title").unwrap())
        .next()
        .map(|t| t.text().collect::<String>().trim().to_string())
        .unwrap_or_default();
    let (doc_id, pub_symbol) = doc_meta(&html);
    let mut ctx = Ctx { base, blocks: vec![], in_footnote: false, with_images };
    if let Some(body) = html.select(&scraper::Selector::parse("body").unwrap()).next() {
        walk_blocks(body, &mut ctx);
    }
    Doc { title, blocks: ctx.blocks, doc_id, pub_symbol }
}

/// Parse an online-library page (wol.jw.org): only the article is kept.
pub fn parse_wol_page(text: &str) -> Doc {
    let html = Html::parse_document(text);
    let (doc_id, pub_symbol) = doc_meta(&html);
    let mut ctx = Ctx { base: "", blocks: vec![], in_footnote: false, with_images: true };
    let art = ["article#article", "#article", "article", "#content", "body"]
        .iter()
        .filter_map(|s| scraper::Selector::parse(s).ok())
        .find_map(|s| html.select(&s).next());
    if let Some(a) = art {
        walk_blocks(a, &mut ctx);
    }
    let title = ctx
        .blocks
        .iter()
        .find(|b| matches!(b.kind, Kind::H1 | Kind::H2))
        .map(|b| b.text.clone())
        .or_else(|| {
            html.select(&scraper::Selector::parse("title").unwrap())
                .next()
                .map(|t| t.text().collect::<String>().trim().to_string())
        })
        .unwrap_or_default();
    // turn site-relative pictures into full addresses
    for b in &mut ctx.blocks {
        if let Kind::Image(src) = &mut b.kind {
            if src.starts_with('/') {
                *src = format!("{}{}", crate::net::WOL, src);
            }
        }
    }
    Doc { title, blocks: ctx.blocks, doc_id, pub_symbol }
}

/// Parse an HTML fragment (e.g. the daily text) without pictures.
pub fn parse_fragment(text: &str) -> Vec<Block> {
    let html = Html::parse_fragment(text);
    let mut ctx = Ctx { base: "", blocks: vec![], in_footnote: false, with_images: false };
    walk_blocks(html.root_element(), &mut ctx);
    ctx.blocks
}

/// Plain text of an HTML page, for searching.
pub fn plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut tag = String::new();
    let mut skip = false;
    for ch in html.chars() {
        if in_tag {
            if ch == '>' {
                in_tag = false;
                let t = tag.to_lowercase();
                if t.starts_with("script") || t.starts_with("style") || t.starts_with("head") {
                    skip = true;
                } else if t.starts_with("/script") || t.starts_with("/style") || t.starts_with("/head") {
                    skip = false;
                }
                if t.starts_with('p') || t.starts_with("/p") || t.starts_with("br") || t.starts_with("h") || t.starts_with("/h") {
                    out.push('\n');
                }
                tag.clear();
            } else {
                tag.push(ch);
            }
        } else if ch == '<' {
            in_tag = true;
        } else if !skip {
            out.push(ch);
        }
    }
    out.replace("&nbsp;", " ").replace("&amp;", "&").replace("&#8212;", "—").replace("&quot;", "\"")
}

/// Word tokens of a text range: (start, end) byte offsets. Highlights are stored as token
/// ranges (BlockRange.StartToken/EndToken in the original).
pub fn tokens(text: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let slice = &text[start..end];
    let mut cur: Option<usize> = None;
    for (i, ch) in slice.char_indices() {
        if ch.is_whitespace() {
            if let Some(s) = cur.take() {
                out.push((start + s, start + i));
            }
        } else if cur.is_none() {
            cur = Some(i);
        }
    }
    if let Some(s) = cur {
        out.push((start + s, end));
    }
    out
}
