//! The publication catalog. Like the original, it is downloaded from
//! app.jw-cdn.org/catalogs/publications/v4/{current}/catalog.db.gz; this app then keeps only
//! the rows for the chosen language in a small local database.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PubInfo {
    pub key_symbol: String,
    pub symbol: String,
    pub issue: i64,
    pub lang: String,
    pub meps: i64,
    pub title: String,
    pub short_title: String,
    pub issue_title: String,
    pub year: i64,
    pub type_id: i64,
    pub cataloged: String,
    pub image: String,
    pub wide_image: String,
}

impl PubInfo {
    pub fn key(&self) -> String {
        format!("{}|{}|{}", self.key_symbol, self.issue, self.lang)
    }
    pub fn display_title(&self) -> String {
        if !self.issue_title.is_empty() {
            self.issue_title.clone()
        } else {
            self.title.clone()
        }
    }
    pub fn file_path(&self) -> std::path::PathBuf {
        crate::paths::pubs_dir(&self.lang)
            .join(format!("{}_{}.epub", crate::paths::safe_name(&self.key_symbol), self.issue))
    }
    pub fn pdf_path(&self) -> std::path::PathBuf {
        crate::paths::pubs_dir(&self.lang)
            .join(format!("{}_{}.pdf", crate::paths::safe_name(&self.key_symbol), self.issue))
    }
    /// Marker for publications kept as online-library pages (no EPUB exists for them).
    pub fn wol_marker(&self) -> std::path::PathBuf {
        crate::paths::pubs_dir(&self.lang)
            .join(format!("{}_{}.wol.json", crate::paths::safe_name(&self.key_symbol), self.issue))
    }
    pub fn wol_installed(&self) -> Option<crate::wol::Installed> {
        std::fs::read_to_string(self.wol_marker()).ok().and_then(|s| serde_json::from_str(&s).ok())
    }
    pub fn is_downloaded(&self) -> bool {
        self.file_path().exists() || self.wol_marker().exists()
    }
    pub fn image_url(&self) -> Option<String> {
        if self.image.is_empty() {
            None
        } else {
            Some(format!("{}/{}", crate::net::CATALOG_BASE, self.image))
        }
    }
    pub fn simple(symbol: &str, issue: i64, lang: &str, title: &str, type_id: i64) -> PubInfo {
        PubInfo {
            key_symbol: symbol.into(),
            symbol: symbol.into(),
            issue,
            lang: lang.into(),
            meps: crate::lang::meps_id(lang),
            title: title.into(),
            short_title: title.into(),
            issue_title: String::new(),
            year: 0,
            type_id,
            cataloged: String::new(),
            image: String::new(),
            wide_image: String::new(),
        }
    }
}

/// Publication type ids seen in the catalog, with the label keys the original uses (pub_type_*).
pub fn type_name(id: i64) -> &'static str {
    match id {
        1 => "Bibles",
        2 => "Books",
        3 => "Programs",
        4 => "Brochures and Booklets",
        5 => "Indexes",
        6 => "Manuals and Guidelines",
        7 => "Kingdom Ministry",
        8 => "Letters",
        9 => "Information Packets",
        10 => "Tracts and Invitations",
        11 => "Forms",
        12 => "Videos",
        13 => "Awake!",
        14 => "The Watchtower",
        15 => "Curriculums",
        17 => "Other",
        22 => "Web Series",
        30 => "Meeting Workbook",
        31 => "Programs",
        33 => "Index",
        _ => "Other",
    }
}

/// Display order of categories on the Publications tab (estimated from the original's layout).
pub const TYPE_ORDER: [i64; 13] = [1, 2, 14, 13, 30, 4, 10, 22, 7, 31, 5, 6, 17];

pub fn db_path(lang: &str) -> std::path::PathBuf {
    crate::paths::sub("catalog").join(format!("catalog_{}.db", crate::paths::safe_name(lang)))
}

pub fn exists(lang: &str) -> bool {
    db_path(lang).exists()
}

/// Download the full catalog and keep this language's rows. Progress through the counters.
pub fn update(lang: &str, done: Arc<AtomicU64>, total: Arc<AtomicU64>) -> Result<usize, String> {
    let manifest = crate::net::get_json(&format!("{}/v4/manifest.json", crate::net::CATALOG_BASE))?;
    let current = manifest
        .get("current")
        .and_then(|c| c.as_str())
        .ok_or("catalog manifest has no current version")?
        .to_string();
    let tmpdir = crate::paths::cache_dir();
    let gz = tmpdir.join("catalog.db.gz");
    let full = tmpdir.join("catalog_full.db");
    crate::net::download(
        &format!("{}/v4/{}/catalog.db.gz", crate::net::CATALOG_BASE, current),
        &gz,
        done.clone(),
        total.clone(),
    )?;
    {
        let f = std::fs::File::open(&gz).map_err(|e| e.to_string())?;
        let mut dec = flate2::read::GzDecoder::new(std::io::BufReader::new(f));
        let mut out = std::fs::File::create(&full).map_err(|e| e.to_string())?;
        std::io::copy(&mut dec, &mut out).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_file(&gz);
    let n = slim(&full, lang)?;
    let _ = std::fs::remove_file(&full);
    done.store(total.load(Ordering::Relaxed), Ordering::Relaxed);
    Ok(n)
}

fn slim(full: &std::path::Path, lang: &str) -> Result<usize, String> {
    let meps = crate::lang::meps_id(lang);
    let out = db_path(lang);
    let tmp = out.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    let c = Connection::open(&tmp).map_err(|e| e.to_string())?;
    c.execute_batch(
        "CREATE TABLE pub(id INTEGER PRIMARY KEY, key_symbol TEXT, symbol TEXT, issue INTEGER, title TEXT,
            short_title TEXT, issue_title TEXT, year INTEGER, type_id INTEGER, cataloged TEXT, image TEXT, wide_image TEXT);
         CREATE TABLE dated(class INTEGER, start TEXT, end TEXT, pub_id INTEGER);
         CREATE TABLE curated(list_type INTEGER, sort INTEGER, pub_id INTEGER);
         CREATE TABLE doc(doc_id INTEGER, pub_id INTEGER);
         CREATE TABLE meta(k TEXT PRIMARY KEY, v TEXT);
         CREATE TABLE bibles(lang_id INTEGER, symbol TEXT, short_title TEXT, title TEXT, year INTEGER, size INTEGER);",
    )
    .map_err(|e| e.to_string())?;
    let path = full.to_string_lossy().replace('\'', "''");
    c.execute_batch(&format!("ATTACH DATABASE '{path}' AS src;")).map_err(|e| e.to_string())?;
    let n = c
        .execute(
            "INSERT INTO pub SELECT p.Id, coalesce(p.KeySymbol,p.Symbol), p.Symbol, p.IssueTagNumber, p.Title, p.ShortTitle,
               coalesce(p.IssueTitle,''), p.Year, p.PublicationTypeId, coalesce(a.CatalogedOn,''),
               coalesce((SELECT i.NameFragment FROM src.PublicationAssetImageMap m JOIN src.ImageAsset i ON i.Id=m.ImageAssetId
                   WHERE m.PublicationAssetId=a.Id AND i.NameFragment LIKE '%sqr%' ORDER BY abs(i.Width-270) LIMIT 1),''),
               coalesce((SELECT i.NameFragment FROM src.PublicationAssetImageMap m JOIN src.ImageAsset i ON i.Id=m.ImageAssetId
                   WHERE m.PublicationAssetId=a.Id AND i.NameFragment LIKE '%lsr%' ORDER BY abs(i.Width-560) LIMIT 1),'')
             FROM src.Publication p LEFT JOIN src.PublicationAsset a ON a.PublicationId=p.Id
             WHERE p.MepsLanguageId=?1",
            params![meps],
        )
        .map_err(|e| e.to_string())?;
    c.execute(
        "INSERT INTO dated SELECT d.Class, d.Start, d.End, d.PublicationId FROM src.DatedText d
           JOIN pub ON pub.id=d.PublicationId",
        [],
    )
    .map_err(|e| e.to_string())?;
    c.execute(
        "INSERT INTO curated SELECT c.ListType, c.SortOrder, a.PublicationId FROM src.CuratedAsset c
           JOIN src.PublicationAsset a ON a.Id=c.PublicationAssetId JOIN pub ON pub.id=a.PublicationId",
        [],
    )
    .map_err(|e| e.to_string())?;
    let _ = c.execute(
        "INSERT INTO doc SELECT d.DocumentId, d.PublicationId FROM src.PublicationDocument d JOIN pub ON pub.id=d.PublicationId",
        [],
    );
    // every language's Bibles, for the "Select a Bible" page
    c.execute(
        "INSERT INTO bibles SELECT p.MepsLanguageId, coalesce(p.KeySymbol,p.Symbol), p.ShortTitle, p.Title, p.Year, coalesce(a.Size,0)
           FROM src.Publication p LEFT JOIN src.PublicationAsset a ON a.PublicationId=p.Id WHERE p.PublicationTypeId=1",
        [],
    )
    .map_err(|e| e.to_string())?;
    c.execute_batch(
        "CREATE INDEX ix_pub_type ON pub(type_id); CREATE INDEX ix_doc ON doc(doc_id); CREATE INDEX ix_dated ON dated(class,start);",
    )
    .map_err(|e| e.to_string())?;
    c.execute(
        "INSERT OR REPLACE INTO meta VALUES('updated', ?1)",
        params![chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()],
    )
    .map_err(|e| e.to_string())?;
    c.execute_batch("DETACH DATABASE src;").map_err(|e| e.to_string())?;
    drop(c);
    std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
    Ok(n)
}

pub struct Catalog {
    conn: Option<Connection>,
    pub lang: String,
}

const COLS: &str = "key_symbol, symbol, issue, title, short_title, issue_title, year, type_id, cataloged, image, wide_image";

impl Catalog {
    pub fn open(lang: &str) -> Catalog {
        let conn = if exists(lang) {
            Connection::open_with_flags(db_path(lang), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()
        } else {
            None
        };
        Catalog { conn, lang: lang.into() }
    }
    pub fn ready(&self) -> bool {
        self.conn.is_some()
    }
    fn row(&self, r: &rusqlite::Row) -> rusqlite::Result<PubInfo> {
        Ok(PubInfo {
            key_symbol: r.get(0)?,
            symbol: r.get(1)?,
            issue: r.get(2)?,
            lang: self.lang.clone(),
            meps: crate::lang::meps_id(&self.lang),
            title: r.get(3)?,
            short_title: r.get(4)?,
            issue_title: r.get(5)?,
            year: r.get(6)?,
            type_id: r.get(7)?,
            cataloged: r.get(8)?,
            image: r.get(9)?,
            wide_image: r.get(10)?,
        })
    }
    fn query(&self, sql: &str, p: impl rusqlite::Params) -> Vec<PubInfo> {
        let Some(c) = &self.conn else { return vec![] };
        let Ok(mut st) = c.prepare(sql) else { return vec![] };
        let rows = st.query_map(p, |r| self.row(r));
        match rows {
            Ok(it) => it.filter_map(|x| x.ok()).collect(),
            Err(_) => vec![],
        }
    }
    pub fn type_counts(&self) -> Vec<(i64, i64)> {
        let Some(c) = &self.conn else { return vec![] };
        let Ok(mut st) = c.prepare("SELECT type_id, count(*) FROM pub GROUP BY type_id") else { return vec![] };
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map(|it| it.filter_map(|x| x.ok()).collect())
            .unwrap_or_default()
    }
    pub fn by_type(&self, t: i64) -> Vec<PubInfo> {
        self.query(
            &format!("SELECT {COLS} FROM pub WHERE type_id=?1 ORDER BY year DESC, issue DESC, title"),
            params![t],
        )
    }
    pub fn whats_new(&self, n: i64) -> Vec<PubInfo> {
        self.query(
            &format!("SELECT {COLS} FROM pub WHERE cataloged<>'' ORDER BY cataloged DESC LIMIT ?1"),
            params![n],
        )
    }
    pub fn curated(&self, list: i64) -> Vec<PubInfo> {
        self.query(
            &format!("SELECT {} FROM curated c JOIN pub p ON p.id=c.pub_id WHERE c.list_type=?1 ORDER BY c.sort",
                COLS.split(", ").map(|c| format!("p.{c}")).collect::<Vec<_>>().join(", ")),
            params![list],
        )
    }
    pub fn find(&self, key_symbol: &str, issue: i64) -> Option<PubInfo> {
        self.query(
            &format!("SELECT {COLS} FROM pub WHERE key_symbol=?1 AND issue=?2 LIMIT 1"),
            params![key_symbol, issue],
        )
        .into_iter()
        .next()
    }
    pub fn search(&self, q: &str) -> Vec<PubInfo> {
        let like = format!("%{}%", q.replace('%', ""));
        self.query(
            &format!("SELECT {COLS} FROM pub WHERE title LIKE ?1 OR issue_title LIKE ?1 OR key_symbol LIKE ?1 ORDER BY year DESC LIMIT 60"),
            params![like],
        )
    }
    /// Publication used for a dated text class on a date (4 = daily text, 68/106 = meeting weeks).
    pub fn dated(&self, class: i64, date: &str) -> Option<PubInfo> {
        self.query(
            &format!("SELECT {} FROM dated d JOIN pub p ON p.id=d.pub_id WHERE d.class=?1 AND d.start<=?2 AND d.end>=?2 LIMIT 1",
                COLS.split(", ").map(|c| format!("p.{c}")).collect::<Vec<_>>().join(", ")),
            params![class, date],
        )
        .into_iter()
        .next()
    }
    /// Bibles in every language: (language id, symbol, short title, title, year, size in bytes).
    /// None when this copy of the list was made before Bibles were kept.
    pub fn all_bibles(&self) -> Option<Vec<(i64, String, String, String, i64, i64)>> {
        let c = self.conn.as_ref()?;
        let mut st = c.prepare("SELECT lang_id, symbol, short_title, title, year, size FROM bibles").ok()?;
        let v = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
            .ok()?
            .filter_map(|x| x.ok())
            .collect();
        Some(v)
    }

    pub fn updated(&self) -> Option<String> {
        let c = self.conn.as_ref()?;
        c.query_row("SELECT v FROM meta WHERE k='updated'", [], |r| r.get(0)).ok()
    }
}
