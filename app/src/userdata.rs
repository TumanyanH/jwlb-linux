//! Personal study data: highlights, notes, tags, bookmarks, favorites, history and playlists.
//! Stored in a local SQLite database. Can import the original's backup files (.jwlibrary:
//! a ZIP with userData.db, whose tables Location/UserMark/BlockRange/Note/Tag/TagMap/Bookmark
//! are documented in MECHANICS.md) and export/import its own JSON backups.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Loc {
    pub key: String,
    pub title: String,
    /// JSON of catalog::PubInfo (may be empty)
    pub pubref: String,
    /// EPUB page path or online address
    pub href: String,
    pub book: i64,
    pub chapter: i64,
    pub doc_id: i64,
}

impl Loc {
    pub fn bible(meps: i64, book: i64, chapter: i64, title: &str, pubref: &str) -> Loc {
        Loc { key: format!("bible:{meps}:{book}:{chapter}"), title: title.into(), pubref: pubref.into(), href: String::new(), book, chapter, doc_id: 0 }
    }
    pub fn pub_info(&self) -> Option<crate::catalog::PubInfo> {
        serde_json::from_str(&self.pubref).ok()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Range {
    pub btype: i64,
    pub ident: i64,
    pub start: i64,
    pub end: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mark {
    pub id: i64,
    pub color: i64,
    pub ranges: Vec<Range>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub guid: String,
    pub title: String,
    pub content: String,
    pub loc: Option<Loc>,
    pub btype: i64,
    pub ident: i64,
    pub color: i64,
    pub created: String,
    pub modified: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bookmark {
    pub pub_key: String,
    pub slot: i64,
    pub loc: Loc,
    pub btype: i64,
    pub ident: i64,
    pub title: String,
    pub snippet: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub image: String,
    pub duration: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub items: Vec<PlaylistItem>,
}

pub struct UserData {
    c: Connection,
}

fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn guid() -> String {
    let a = fastrand::u64(..);
    let b = fastrand::u64(..);
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        (a >> 16) as u16,
        (a & 0xfff) as u16,
        (b >> 48) as u16 & 0x3fff | 0x8000,
        b & 0xffff_ffff_ffff
    )
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS location(id INTEGER PRIMARY KEY, key TEXT UNIQUE NOT NULL, title TEXT, pubref TEXT, href TEXT,
    book INTEGER DEFAULT 0, chapter INTEGER DEFAULT 0, doc_id INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS usermark(id INTEGER PRIMARY KEY, guid TEXT UNIQUE, color INTEGER NOT NULL, location_id INTEGER NOT NULL, created TEXT);
CREATE TABLE IF NOT EXISTS blockrange(id INTEGER PRIMARY KEY, usermark_id INTEGER NOT NULL, btype INTEGER, ident INTEGER, start_tok INTEGER, end_tok INTEGER);
CREATE TABLE IF NOT EXISTS note(id INTEGER PRIMARY KEY, guid TEXT UNIQUE, title TEXT, content TEXT, location_id INTEGER,
    btype INTEGER DEFAULT 0, ident INTEGER DEFAULT 0, color INTEGER DEFAULT 0, created TEXT, modified TEXT);
CREATE TABLE IF NOT EXISTS tag(id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL);
CREATE TABLE IF NOT EXISTS tagmap(tag_id INTEGER NOT NULL, note_id INTEGER, position INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS bookmark(pub_key TEXT NOT NULL, slot INTEGER NOT NULL, location_id INTEGER, btype INTEGER, ident INTEGER,
    title TEXT, snippet TEXT, PRIMARY KEY(pub_key, slot));
CREATE TABLE IF NOT EXISTS favorite(pub_key TEXT PRIMARY KEY, pubref TEXT, position INTEGER);
CREATE TABLE IF NOT EXISTS history(location_id INTEGER PRIMARY KEY, opened TEXT);
CREATE TABLE IF NOT EXISTS playlist(id INTEGER PRIMARY KEY, name TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS playlist_item(id INTEGER PRIMARY KEY, playlist_id INTEGER, title TEXT, url TEXT, image TEXT, duration TEXT, position INTEGER);
CREATE INDEX IF NOT EXISTS ix_um_loc ON usermark(location_id);
CREATE INDEX IF NOT EXISTS ix_br_um ON blockrange(usermark_id);
CREATE INDEX IF NOT EXISTS ix_note_loc ON note(location_id);
";

impl UserData {
    pub fn open() -> UserData {
        let path = crate::paths::data_dir().join("userdata.db");
        let c = Connection::open(&path).unwrap_or_else(|_| Connection::open_in_memory().unwrap());
        let _ = c.execute_batch(SCHEMA);
        UserData { c }
    }

    pub fn loc_id(&self, l: &Loc) -> i64 {
        let _ = self.c.execute(
            "INSERT INTO location(key,title,pubref,href,book,chapter,doc_id) VALUES(?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(key) DO UPDATE SET title=CASE WHEN excluded.title<>'' THEN excluded.title ELSE title END,
               pubref=CASE WHEN excluded.pubref<>'' THEN excluded.pubref ELSE pubref END,
               href=CASE WHEN excluded.href<>'' THEN excluded.href ELSE href END",
            params![l.key, l.title, l.pubref, l.href, l.book, l.chapter, l.doc_id],
        );
        self.c.query_row("SELECT id FROM location WHERE key=?1", params![l.key], |r| r.get(0)).unwrap_or(0)
    }

    fn loc_by_id(&self, id: i64) -> Option<Loc> {
        self.c
            .query_row("SELECT key,title,pubref,href,book,chapter,doc_id FROM location WHERE id=?1", params![id], |r| {
                Ok(Loc {
                    key: r.get(0)?,
                    title: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    pubref: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    href: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    book: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    chapter: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    doc_id: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
                })
            })
            .ok()
    }

    // ---------- highlights ----------
    pub fn marks_for(&self, key: &str) -> Vec<Mark> {
        let mut out = vec![];
        let Ok(mut st) = self.c.prepare(
            "SELECT u.id,u.color FROM usermark u JOIN location l ON l.id=u.location_id WHERE l.key=?1 ORDER BY u.id",
        ) else {
            return out;
        };
        let marks: Vec<(i64, i64)> = st
            .query_map(params![key], |r| Ok((r.get(0)?, r.get(1)?)))
            .map(|it| it.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        for (id, color) in marks {
            let ranges = self.ranges(id);
            out.push(Mark { id, color, ranges });
        }
        out
    }

    fn ranges(&self, um: i64) -> Vec<Range> {
        let Ok(mut st) = self.c.prepare("SELECT btype,ident,start_tok,end_tok FROM blockrange WHERE usermark_id=?1") else {
            return vec![];
        };
        st.query_map(params![um], |r| {
            Ok(Range {
                btype: r.get(0)?,
                ident: r.get(1)?,
                start: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                end: r.get::<_, Option<i64>>(3)?.unwrap_or(i64::MAX / 2),
            })
        })
        .map(|it| it.filter_map(|x| x.ok()).collect())
        .unwrap_or_default()
    }

    pub fn add_mark(&self, loc: &Loc, color: i64, ranges: &[Range]) -> i64 {
        let lid = self.loc_id(loc);
        // a new highlight replaces the parts of older ones it covers
        self.erase(loc, ranges);
        let _ = self.c.execute(
            "INSERT INTO usermark(guid,color,location_id,created) VALUES(?1,?2,?3,?4)",
            params![guid(), color, lid, now()],
        );
        let id = self.c.last_insert_rowid();
        for r in ranges {
            let _ = self.c.execute(
                "INSERT INTO blockrange(usermark_id,btype,ident,start_tok,end_tok) VALUES(?1,?2,?3,?4,?5)",
                params![id, r.btype, r.ident, r.start, r.end],
            );
        }
        id
    }

    /// Remove highlighting from the given token ranges (splitting marks that only partly overlap).
    pub fn erase(&self, loc: &Loc, ranges: &[Range]) {
        let marks = self.marks_for(&loc.key);
        for m in marks {
            let mut changed = false;
            let mut kept: Vec<Range> = vec![];
            for r in &m.ranges {
                let mut pieces = vec![r.clone()];
                for e in ranges {
                    if e.btype != r.btype || e.ident != r.ident {
                        continue;
                    }
                    let mut next = vec![];
                    for p in pieces {
                        if e.end < p.start || e.start > p.end {
                            next.push(p);
                            continue;
                        }
                        changed = true;
                        if p.start < e.start {
                            next.push(Range { end: e.start - 1, ..p.clone() });
                        }
                        if p.end > e.end {
                            next.push(Range { start: e.end + 1, ..p.clone() });
                        }
                    }
                    pieces = next;
                }
                kept.extend(pieces);
            }
            if changed {
                let _ = self.c.execute("DELETE FROM blockrange WHERE usermark_id=?1", params![m.id]);
                if kept.is_empty() {
                    let _ = self.c.execute("DELETE FROM usermark WHERE id=?1", params![m.id]);
                } else {
                    for r in kept {
                        let _ = self.c.execute(
                            "INSERT INTO blockrange(usermark_id,btype,ident,start_tok,end_tok) VALUES(?1,?2,?3,?4,?5)",
                            params![m.id, r.btype, r.ident, r.start, r.end],
                        );
                    }
                }
            }
        }
    }

    pub fn highlight_count(&self) -> i64 {
        self.c.query_row("SELECT count(*) FROM usermark", [], |r| r.get(0)).unwrap_or(0)
    }

    /// Every highlighted place, newest first: (location, colour, mark id).
    pub fn all_marks(&self) -> Vec<(Loc, Mark)> {
        let Ok(mut st) = self.c.prepare("SELECT id,color,location_id FROM usermark ORDER BY id DESC LIMIT 500") else { return vec![] };
        let rows: Vec<(i64, i64, i64)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|it| it.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        rows.into_iter()
            .filter_map(|(id, color, lid)| Some((self.loc_by_id(lid)?, Mark { id, color, ranges: self.ranges(id) })))
            .collect()
    }

    // ---------- notes & tags ----------
    fn note_from_row(&self, r: &rusqlite::Row) -> rusqlite::Result<Note> {
        let lid: Option<i64> = r.get(4)?;
        Ok(Note {
            id: r.get(0)?,
            guid: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            title: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            content: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            loc: lid.and_then(|l| self.loc_by_id(l)),
            btype: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
            ident: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
            color: r.get::<_, Option<i64>>(7)?.unwrap_or(0),
            created: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
            modified: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
            tags: vec![],
        })
    }

    fn notes_query(&self, sql: &str, p: impl rusqlite::Params) -> Vec<Note> {
        let Ok(mut st) = self.c.prepare(sql) else { return vec![] };
        let notes: Vec<Note> = st
            .query_map(p, |r| self.note_from_row(r))
            .map(|it| it.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        notes
            .into_iter()
            .map(|mut n| {
                n.tags = self.note_tags(n.id);
                n
            })
            .collect()
    }

    const NOTE_COLS: &'static str = "n.id,n.guid,n.title,n.content,n.location_id,n.btype,n.ident,n.color,n.created,n.modified";

    pub fn notes(&self, search: &str, tag: Option<&str>) -> Vec<Note> {
        let like = format!("%{}%", search);
        match tag {
            Some(t) => self.notes_query(
                &format!("SELECT {} FROM note n JOIN tagmap m ON m.note_id=n.id JOIN tag t ON t.id=m.tag_id
                  WHERE t.name=?1 AND (n.title LIKE ?2 OR n.content LIKE ?2) ORDER BY m.position, n.modified DESC", Self::NOTE_COLS),
                params![t, like],
            ),
            None => self.notes_query(
                &format!("SELECT {} FROM note n WHERE n.title LIKE ?1 OR n.content LIKE ?1 ORDER BY n.modified DESC", Self::NOTE_COLS),
                params![like],
            ),
        }
    }

    pub fn notes_for(&self, key: &str) -> Vec<Note> {
        self.notes_query(
            &format!("SELECT {} FROM note n JOIN location l ON l.id=n.location_id WHERE l.key=?1 ORDER BY n.ident, n.id", Self::NOTE_COLS),
            params![key],
        )
    }

    pub fn save_note(&self, n: &Note) -> i64 {
        let lid = n.loc.as_ref().map(|l| self.loc_id(l));
        let id = if n.id > 0 {
            let _ = self.c.execute(
                "UPDATE note SET title=?1, content=?2, modified=?3, color=?4 WHERE id=?5",
                params![n.title, n.content, now(), n.color, n.id],
            );
            n.id
        } else {
            let g = if n.guid.is_empty() { guid() } else { n.guid.clone() };
            let created = if n.created.is_empty() { now() } else { n.created.clone() };
            let modified = if n.modified.is_empty() { now() } else { n.modified.clone() };
            let _ = self.c.execute(
                "INSERT OR IGNORE INTO note(guid,title,content,location_id,btype,ident,color,created,modified) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![g, n.title, n.content, lid, n.btype, n.ident, n.color, created, modified],
            );
            self.c.query_row("SELECT id FROM note WHERE guid=?1", params![g], |r| r.get(0)).unwrap_or(0)
        };
        self.set_note_tags(id, &n.tags);
        id
    }

    pub fn delete_note(&self, id: i64) {
        let _ = self.c.execute("DELETE FROM tagmap WHERE note_id=?1", params![id]);
        let _ = self.c.execute("DELETE FROM note WHERE id=?1", params![id]);
    }

    pub fn note_count(&self) -> i64 {
        self.c.query_row("SELECT count(*) FROM note", [], |r| r.get(0)).unwrap_or(0)
    }

    pub fn note_tags(&self, note: i64) -> Vec<String> {
        let Ok(mut st) = self.c.prepare("SELECT t.name FROM tagmap m JOIN tag t ON t.id=m.tag_id WHERE m.note_id=?1 ORDER BY t.name") else {
            return vec![];
        };
        st.query_map(params![note], |r| r.get(0)).map(|it| it.filter_map(|x| x.ok()).collect()).unwrap_or_default()
    }

    pub fn tag_id(&self, name: &str) -> i64 {
        let _ = self.c.execute("INSERT OR IGNORE INTO tag(name) VALUES(?1)", params![name]);
        self.c.query_row("SELECT id FROM tag WHERE name=?1", params![name], |r| r.get(0)).unwrap_or(0)
    }

    pub fn set_note_tags(&self, note: i64, tags: &[String]) {
        let _ = self.c.execute("DELETE FROM tagmap WHERE note_id=?1", params![note]);
        for t in tags {
            let t = t.trim();
            if t.is_empty() {
                continue;
            }
            let tid = self.tag_id(t);
            let pos: i64 = self
                .c
                .query_row("SELECT coalesce(max(position),-1)+1 FROM tagmap WHERE tag_id=?1", params![tid], |r| r.get(0))
                .unwrap_or(0);
            let _ = self.c.execute("INSERT INTO tagmap(tag_id,note_id,position) VALUES(?1,?2,?3)", params![tid, note, pos]);
        }
    }

    pub fn tags(&self) -> Vec<(String, i64)> {
        let Ok(mut st) = self
            .c
            .prepare("SELECT t.name, count(m.note_id) FROM tag t LEFT JOIN tagmap m ON m.tag_id=t.id GROUP BY t.id ORDER BY t.name COLLATE NOCASE")
        else {
            return vec![];
        };
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map(|it| it.filter_map(|x| x.ok()).collect()).unwrap_or_default()
    }

    pub fn rename_tag(&self, old: &str, new: &str) {
        let _ = self.c.execute("UPDATE tag SET name=?2 WHERE name=?1", params![old, new]);
    }

    pub fn delete_tag(&self, name: &str) {
        let _ = self.c.execute("DELETE FROM tagmap WHERE tag_id=(SELECT id FROM tag WHERE name=?1)", params![name]);
        let _ = self.c.execute("DELETE FROM tag WHERE name=?1", params![name]);
    }

    // ---------- bookmarks ----------
    pub fn bookmarks(&self, pub_key: &str) -> Vec<Option<Bookmark>> {
        let mut out: Vec<Option<Bookmark>> = (0..10).map(|_| None).collect();
        let Ok(mut st) = self.c.prepare("SELECT slot,location_id,btype,ident,title,snippet FROM bookmark WHERE pub_key=?1") else {
            return out;
        };
        let rows: Vec<(i64, i64, i64, i64, String, String)> = st
            .query_map(params![pub_key], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                ))
            })
            .map(|it| it.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        for (slot, lid, bt, id, title, snip) in rows {
            if (0..10).contains(&slot) {
                if let Some(loc) = self.loc_by_id(lid) {
                    out[slot as usize] = Some(Bookmark { pub_key: pub_key.into(), slot, loc, btype: bt, ident: id, title, snippet: snip });
                }
            }
        }
        out
    }

    pub fn set_bookmark(&self, b: &Bookmark) {
        let lid = self.loc_id(&b.loc);
        let _ = self.c.execute(
            "INSERT OR REPLACE INTO bookmark(pub_key,slot,location_id,btype,ident,title,snippet) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![b.pub_key, b.slot, lid, b.btype, b.ident, b.title, b.snippet],
        );
    }

    pub fn remove_bookmark(&self, pub_key: &str, slot: i64) {
        let _ = self.c.execute("DELETE FROM bookmark WHERE pub_key=?1 AND slot=?2", params![pub_key, slot]);
    }

    pub fn all_bookmarks(&self) -> Vec<Bookmark> {
        let keys: Vec<String> = self
            .c
            .prepare("SELECT DISTINCT pub_key FROM bookmark")
            .and_then(|mut st| st.query_map([], |r| r.get(0)).map(|it| it.filter_map(|x| x.ok()).collect()))
            .unwrap_or_default();
        keys.iter().flat_map(|k| self.bookmarks(k).into_iter().flatten()).collect()
    }

    // ---------- favorites ----------
    pub fn favorites(&self) -> Vec<crate::catalog::PubInfo> {
        self.c
            .prepare("SELECT pubref FROM favorite ORDER BY position")
            .and_then(|mut st| {
                st.query_map([], |r| r.get::<_, String>(0))
                    .map(|it| it.filter_map(|x| x.ok()).filter_map(|s| serde_json::from_str(&s).ok()).collect())
            })
            .unwrap_or_default()
    }

    pub fn is_favorite(&self, key: &str) -> bool {
        self.c.query_row("SELECT 1 FROM favorite WHERE pub_key=?1", params![key], |_| Ok(())).optional().ok().flatten().is_some()
    }

    pub fn toggle_favorite(&self, p: &crate::catalog::PubInfo) {
        if self.is_favorite(&p.key()) {
            let _ = self.c.execute("DELETE FROM favorite WHERE pub_key=?1", params![p.key()]);
        } else {
            let pos: i64 = self.c.query_row("SELECT coalesce(max(position),0)+1 FROM favorite", [], |r| r.get(0)).unwrap_or(0);
            let _ = self.c.execute(
                "INSERT OR REPLACE INTO favorite(pub_key,pubref,position) VALUES(?1,?2,?3)",
                params![p.key(), serde_json::to_string(p).unwrap_or_default(), pos],
            );
        }
    }

    // ---------- history ----------
    pub fn visit(&self, l: &Loc) {
        let id = self.loc_id(l);
        let _ = self.c.execute("INSERT OR REPLACE INTO history(location_id,opened) VALUES(?1,?2)", params![id, now()]);
    }

    pub fn recent(&self, n: i64) -> Vec<Loc> {
        let ids: Vec<i64> = self
            .c
            .prepare("SELECT location_id FROM history ORDER BY opened DESC LIMIT ?1")
            .and_then(|mut st| st.query_map(params![n], |r| r.get(0)).map(|it| it.filter_map(|x| x.ok()).collect()))
            .unwrap_or_default();
        ids.into_iter().filter_map(|i| self.loc_by_id(i)).collect()
    }

    // ---------- playlists ----------
    pub fn playlists(&self) -> Vec<Playlist> {
        let lists: Vec<(i64, String)> = self
            .c
            .prepare("SELECT id,name FROM playlist ORDER BY name COLLATE NOCASE")
            .and_then(|mut st| st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map(|it| it.filter_map(|x| x.ok()).collect()))
            .unwrap_or_default();
        lists
            .into_iter()
            .map(|(id, name)| {
                let items = self
                    .c
                    .prepare("SELECT id,title,url,image,duration FROM playlist_item WHERE playlist_id=?1 ORDER BY position")
                    .and_then(|mut st| {
                        st.query_map(params![id], |r| {
                            Ok(PlaylistItem {
                                id: r.get(0)?,
                                title: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                                url: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                                image: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                                duration: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                            })
                        })
                        .map(|it| it.filter_map(|x| x.ok()).collect())
                    })
                    .unwrap_or_default();
                Playlist { id, name, items }
            })
            .collect()
    }

    pub fn create_playlist(&self, name: &str) -> i64 {
        let _ = self.c.execute("INSERT INTO playlist(name) VALUES(?1)", params![name]);
        self.c.last_insert_rowid()
    }

    pub fn delete_playlist(&self, id: i64) {
        let _ = self.c.execute("DELETE FROM playlist_item WHERE playlist_id=?1", params![id]);
        let _ = self.c.execute("DELETE FROM playlist WHERE id=?1", params![id]);
    }

    pub fn rename_playlist(&self, id: i64, name: &str) {
        let _ = self.c.execute("UPDATE playlist SET name=?2 WHERE id=?1", params![id, name]);
    }

    pub fn add_to_playlist(&self, pl: i64, item: &PlaylistItem) {
        let pos: i64 = self
            .c
            .query_row("SELECT coalesce(max(position),0)+1 FROM playlist_item WHERE playlist_id=?1", params![pl], |r| r.get(0))
            .unwrap_or(0);
        let _ = self.c.execute(
            "INSERT INTO playlist_item(playlist_id,title,url,image,duration,position) VALUES(?1,?2,?3,?4,?5,?6)",
            params![pl, item.title, item.url, item.image, item.duration, pos],
        );
    }

    pub fn remove_playlist_item(&self, id: i64) {
        let _ = self.c.execute("DELETE FROM playlist_item WHERE id=?1", params![id]);
    }

    pub fn move_playlist_item(&self, pl: i64, id: i64, up: bool) {
        let items: Vec<i64> = self
            .c
            .prepare("SELECT id FROM playlist_item WHERE playlist_id=?1 ORDER BY position")
            .and_then(|mut st| st.query_map(params![pl], |r| r.get(0)).map(|it| it.filter_map(|x| x.ok()).collect()))
            .unwrap_or_default();
        let Some(i) = items.iter().position(|x| *x == id) else { return };
        let j = if up { i.checked_sub(1) } else { (i + 1 < items.len()).then_some(i + 1) };
        let Some(j) = j else { return };
        let mut v = items.clone();
        v.swap(i, j);
        for (p, iid) in v.iter().enumerate() {
            let _ = self.c.execute("UPDATE playlist_item SET position=?1 WHERE id=?2", params![p as i64, iid]);
        }
    }

    // ---------- backup ----------
    pub fn export_json(&self, path: &std::path::Path) -> Result<String, String> {
        #[derive(Serialize)]
        struct Backup {
            format: &'static str,
            version: u32,
            created: String,
            notes: Vec<Note>,
            highlights: Vec<(Loc, Mark)>,
            bookmarks: Vec<Bookmark>,
            favorites: Vec<crate::catalog::PubInfo>,
            playlists: Vec<Playlist>,
            tags: Vec<String>,
        }
        let b = Backup {
            format: "study-library-backup",
            version: 1,
            created: now(),
            notes: self.notes("", None),
            highlights: self.all_marks_unbounded(),
            bookmarks: self.all_bookmarks(),
            favorites: self.favorites(),
            playlists: self.playlists(),
            tags: self.tags().into_iter().map(|t| t.0).collect(),
        };
        let s = serde_json::to_string_pretty(&b).map_err(|e| e.to_string())?;
        std::fs::write(path, s).map_err(|e| e.to_string())?;
        Ok(format!(
            "Saved {} notes, {} highlights, {} bookmarks.",
            b.notes.len(),
            b.highlights.len(),
            b.bookmarks.len()
        ))
    }

    fn all_marks_unbounded(&self) -> Vec<(Loc, Mark)> {
        let Ok(mut st) = self.c.prepare("SELECT id,color,location_id FROM usermark ORDER BY id") else { return vec![] };
        let rows: Vec<(i64, i64, i64)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|it| it.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        rows.into_iter()
            .filter_map(|(id, color, lid)| Some((self.loc_by_id(lid)?, Mark { id, color, ranges: self.ranges(id) })))
            .collect()
    }

    pub fn import_json(&self, path: &std::path::Path) -> Result<String, String> {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct Backup {
            notes: Vec<Note>,
            highlights: Vec<(Loc, Mark)>,
            bookmarks: Vec<Bookmark>,
            favorites: Vec<crate::catalog::PubInfo>,
            playlists: Vec<Playlist>,
            tags: Vec<String>,
        }
        let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let b: Backup = serde_json::from_str(&s).map_err(|e| format!("Not a Study Library backup: {e}"))?;
        let _ = self.c.execute_batch("BEGIN");
        for t in &b.tags {
            self.tag_id(t);
        }
        for n in &b.notes {
            let mut n = n.clone();
            n.id = 0;
            self.save_note(&n);
        }
        for (l, m) in &b.highlights {
            self.add_mark(l, m.color, &m.ranges);
        }
        for bm in &b.bookmarks {
            self.set_bookmark(bm);
        }
        for f in &b.favorites {
            if !self.is_favorite(&f.key()) {
                self.toggle_favorite(f);
            }
        }
        for p in &b.playlists {
            let id = self.create_playlist(&p.name);
            for it in &p.items {
                self.add_to_playlist(id, it);
            }
        }
        let _ = self.c.execute_batch("COMMIT");
        Ok(format!("Restored {} notes, {} highlights, {} bookmarks.", b.notes.len(), b.highlights.len(), b.bookmarks.len()))
    }

    /// Import a backup made by the original Windows/Android/iOS app (.jwlibrary).
    pub fn import_jwlibrary(&self, path: &std::path::Path) -> Result<String, String> {
        let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut z = zip::ZipArchive::new(f).map_err(|_| "This file is not a backup (not a ZIP file).".to_string())?;
        let tmp = crate::paths::cache_dir().join("import_userData.db");
        {
            let mut entry = z.by_name("userData.db").map_err(|_| "The backup has no userData.db inside.".to_string())?;
            let mut out = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
        let src = Connection::open(&tmp).map_err(|e| e.to_string())?;
        let cols = |t: &str| -> Vec<String> {
            src.prepare(&format!("PRAGMA table_info({t})"))
                .and_then(|mut st| st.query_map([], |r| r.get::<_, String>(1)).map(|it| it.filter_map(|x| x.ok()).collect()))
                .unwrap_or_default()
        };
        // Locations
        let mut locs: std::collections::HashMap<i64, Loc> = std::collections::HashMap::new();
        {
            let mut st = src
                .prepare("SELECT LocationId,BookNumber,ChapterNumber,DocumentId,IssueTagNumber,KeySymbol,MepsLanguage,Title FROM Location")
                .map_err(|e| e.to_string())?;
            let rows = st
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, Option<i64>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                        r.get::<_, Option<i64>>(3)?,
                        r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                        r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                        r.get::<_, Option<i64>>(6)?.unwrap_or(0),
                        r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    ))
                })
                .map_err(|e| e.to_string())?;
            for row in rows.flatten() {
                let (id, book, chap, doc, issue, sym, meps, title) = row;
                let lang = crate::lang::by_id(meps).map(|l| l.symbol.clone()).unwrap_or_else(|| "E".into());
                let p = crate::catalog::PubInfo::simple(&sym, issue, &lang, &title, 0);
                let pubref = serde_json::to_string(&p).unwrap_or_default();
                let loc = if let (Some(b), Some(c)) = (book, chap) {
                    let name = crate::bible::BOOKS.get((b - 1).max(0) as usize).map(|x| x.name).unwrap_or("");
                    Loc::bible(meps, b, c, &format!("{name} {c}"), &pubref)
                } else if let Some(d) = doc {
                    Loc { key: format!("doc:{meps}:{d}"), title: title.clone(), pubref, href: String::new(), book: 0, chapter: 0, doc_id: d }
                } else {
                    Loc { key: format!("pub:{}", p.key()), title: title.clone(), pubref, href: String::new(), book: 0, chapter: 0, doc_id: 0 }
                };
                locs.insert(id, loc);
            }
        }
        let _ = self.c.execute_batch("BEGIN");
        // Highlights
        let mut n_marks = 0;
        {
            let mut st = src.prepare("SELECT UserMarkId,ColorIndex,LocationId FROM UserMark").map_err(|e| e.to_string())?;
            let marks: Vec<(i64, i64, i64)> = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(|e| e.to_string())?
                .flatten()
                .collect();
            let mut rs = src
                .prepare("SELECT BlockType,Identifier,StartToken,EndToken FROM BlockRange WHERE UserMarkId=?1")
                .map_err(|e| e.to_string())?;
            for (um, color, lid) in marks {
                let Some(loc) = locs.get(&lid) else { continue };
                let ranges: Vec<Range> = rs
                    .query_map(params![um], |r| {
                        Ok(Range {
                            btype: r.get(0)?,
                            ident: r.get(1)?,
                            start: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                            end: r.get::<_, Option<i64>>(3)?.unwrap_or(i64::MAX / 2),
                        })
                    })
                    .map(|it| it.flatten().collect())
                    .unwrap_or_default();
                if ranges.is_empty() || color <= 0 {
                    continue;
                }
                self.add_mark(loc, color, &ranges);
                n_marks += 1;
            }
        }
        // Tags
        let mut tag_names: std::collections::HashMap<i64, (i64, String)> = std::collections::HashMap::new();
        if let Ok(mut st) = src.prepare("SELECT TagId,Type,Name FROM Tag") {
            for (id, ty, name) in st
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))
                .map_err(|e| e.to_string())?
                .flatten()
            {
                tag_names.insert(id, (ty, name));
            }
        }
        // Notes
        let mut n_notes = 0;
        {
            let nc = cols("Note");
            let created = if nc.iter().any(|c| c == "Created") { "Created" } else { "LastModified" };
            let mut st = src
                .prepare(&format!(
                    "SELECT NoteId,Guid,Title,Content,LocationId,BlockType,BlockIdentifier,LastModified,{created},
                       (SELECT ColorIndex FROM UserMark u WHERE u.UserMarkId=Note.UserMarkId) FROM Note"
                ))
                .map_err(|e| e.to_string())?;
            let notes: Vec<(i64, Note, Option<i64>)> = st
                .query_map([], |r| {
                    let lid: Option<i64> = r.get(4)?;
                    Ok((
                        r.get::<_, i64>(0)?,
                        Note {
                            id: 0,
                            guid: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                            title: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                            content: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                            loc: None,
                            btype: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                            ident: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
                            color: r.get::<_, Option<i64>>(9)?.unwrap_or(0),
                            created: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
                            modified: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                            tags: vec![],
                        },
                        lid,
                    ))
                })
                .map_err(|e| e.to_string())?
                .flatten()
                .collect();
            let tagmap_cols = cols("TagMap");
            let has_note_col = tagmap_cols.iter().any(|c| c == "NoteId");
            for (old_id, mut n, lid) in notes {
                n.loc = lid.and_then(|l| locs.get(&l).cloned());
                if has_note_col {
                    if let Ok(mut ts) = src.prepare("SELECT TagId FROM TagMap WHERE NoteId=?1") {
                        let tids: Vec<i64> = ts.query_map(params![old_id], |r| r.get(0)).map(|it| it.flatten().collect()).unwrap_or_default();
                        n.tags = tids.iter().filter_map(|t| tag_names.get(t)).filter(|(ty, _)| *ty == 1).map(|(_, nm)| nm.clone()).collect();
                    }
                }
                self.save_note(&n);
                n_notes += 1;
            }
        }
        // Favorites (Tag type 0 mapped to locations)
        if cols("TagMap").iter().any(|c| c == "LocationId") {
            if let Ok(mut st) = src.prepare("SELECT m.LocationId FROM TagMap m JOIN Tag t ON t.TagId=m.TagId WHERE t.Type=0 AND m.LocationId IS NOT NULL ORDER BY m.Position") {
                let lids: Vec<i64> = st.query_map([], |r| r.get(0)).map(|it| it.flatten().collect()).unwrap_or_default();
                for l in lids {
                    if let Some(p) = locs.get(&l).and_then(|l| l.pub_info()) {
                        if !self.is_favorite(&p.key()) {
                            self.toggle_favorite(&p);
                        }
                    }
                }
            }
        }
        // Bookmarks
        let mut n_bm = 0;
        if let Ok(mut st) = src.prepare("SELECT LocationId,PublicationLocationId,Slot,Title,Snippet,BlockType,BlockIdentifier FROM Bookmark") {
            let rows: Vec<(i64, i64, i64, String, Option<String>, i64, Option<i64>)> = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))
                .map(|it| it.flatten().collect())
                .unwrap_or_default();
            for (lid, plid, slot, title, snip, bt, bid) in rows {
                let (Some(loc), Some(ploc)) = (locs.get(&lid), locs.get(&plid)) else { continue };
                let Some(p) = ploc.pub_info() else { continue };
                // Bible bookmarks are shared by all Bibles in this app
                let key = if loc.key.starts_with("bible:") { format!("bible|{}", p.lang) } else { p.key() };
                self.set_bookmark(&Bookmark { pub_key: key, slot, loc: loc.clone(), btype: bt, ident: bid.unwrap_or(0), title, snippet: snip.unwrap_or_default() });
                n_bm += 1;
            }
        }
        let _ = self.c.execute_batch("COMMIT");
        drop(src);
        let _ = std::fs::remove_file(&tmp);
        Ok(format!("Imported {n_notes} notes, {n_marks} highlights and {n_bm} bookmarks."))
    }
}
