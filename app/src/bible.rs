//! The 66 Bible books: English names, abbreviations, chapter counts and groups
//! (groups and chapter counts from the original's language database, BibleBookGroup/BibleRange).

pub struct Book {
    pub name: &'static str,
    pub abbr: &'static str,
    pub chapters: u16,
    pub group: u8,
}

macro_rules! b {
    ($n:expr, $a:expr, $c:expr, $g:expr) => {
        Book { name: $n, abbr: $a, chapters: $c, group: $g }
    };
}

pub static BOOKS: [Book; 66] = [
    b!("Genesis", "Gen.", 50, 0),
    b!("Exodus", "Ex.", 40, 0),
    b!("Leviticus", "Lev.", 27, 0),
    b!("Numbers", "Num.", 36, 0),
    b!("Deuteronomy", "Deut.", 34, 0),
    b!("Joshua", "Josh.", 24, 1),
    b!("Judges", "Judg.", 21, 1),
    b!("Ruth", "Ruth", 4, 1),
    b!("1 Samuel", "1 Sam.", 31, 1),
    b!("2 Samuel", "2 Sam.", 24, 1),
    b!("1 Kings", "1 Ki.", 22, 1),
    b!("2 Kings", "2 Ki.", 25, 1),
    b!("1 Chronicles", "1 Chron.", 29, 1),
    b!("2 Chronicles", "2 Chron.", 36, 1),
    b!("Ezra", "Ezra", 10, 1),
    b!("Nehemiah", "Neh.", 13, 1),
    b!("Esther", "Esther", 10, 1),
    b!("Job", "Job", 42, 2),
    b!("Psalms", "Ps.", 150, 2),
    b!("Proverbs", "Prov.", 31, 2),
    b!("Ecclesiastes", "Eccl.", 12, 2),
    b!("Song of Solomon", "Song of Sol.", 8, 2),
    b!("Isaiah", "Isa.", 66, 3),
    b!("Jeremiah", "Jer.", 52, 3),
    b!("Lamentations", "Lam.", 5, 3),
    b!("Ezekiel", "Ezek.", 48, 3),
    b!("Daniel", "Dan.", 12, 3),
    b!("Hosea", "Hos.", 14, 3),
    b!("Joel", "Joel", 3, 3),
    b!("Amos", "Amos", 9, 3),
    b!("Obadiah", "Obad.", 1, 3),
    b!("Jonah", "Jonah", 4, 3),
    b!("Micah", "Mic.", 7, 3),
    b!("Nahum", "Nah.", 3, 3),
    b!("Habakkuk", "Hab.", 3, 3),
    b!("Zephaniah", "Zeph.", 3, 3),
    b!("Haggai", "Hag.", 2, 3),
    b!("Zechariah", "Zech.", 14, 3),
    b!("Malachi", "Mal.", 4, 3),
    b!("Matthew", "Matt.", 28, 4),
    b!("Mark", "Mark", 16, 4),
    b!("Luke", "Luke", 24, 4),
    b!("John", "John", 21, 4),
    b!("Acts", "Acts", 28, 5),
    b!("Romans", "Rom.", 16, 6),
    b!("1 Corinthians", "1 Cor.", 16, 6),
    b!("2 Corinthians", "2 Cor.", 13, 6),
    b!("Galatians", "Gal.", 6, 6),
    b!("Ephesians", "Eph.", 6, 6),
    b!("Philippians", "Phil.", 4, 6),
    b!("Colossians", "Col.", 4, 6),
    b!("1 Thessalonians", "1 Thess.", 5, 6),
    b!("2 Thessalonians", "2 Thess.", 3, 6),
    b!("1 Timothy", "1 Tim.", 6, 6),
    b!("2 Timothy", "2 Tim.", 4, 6),
    b!("Titus", "Titus", 3, 6),
    b!("Philemon", "Philem.", 1, 6),
    b!("Hebrews", "Heb.", 13, 6),
    b!("James", "Jas.", 5, 6),
    b!("1 Peter", "1 Pet.", 5, 6),
    b!("2 Peter", "2 Pet.", 3, 6),
    b!("1 John", "1 John", 5, 6),
    b!("2 John", "2 John", 1, 6),
    b!("3 John", "3 John", 1, 6),
    b!("Jude", "Jude", 1, 6),
    b!("Revelation", "Rev.", 22, 7),
];

/// Book number of the first book of the Christian Greek Scriptures (BibleInfo.FirstBookOfGreekScripturesNumber).
pub const FIRST_GREEK_BOOK: u8 = 40;

/// Parse "John 3:16", "jn 3", "1 Cor 13:4-7", "Ps 23" against the given book names.
/// Returns (book, chapter, verse).
pub fn parse_reference(input: &str, names: &[(u8, String, String)]) -> Option<(u8, u16, Option<u16>)> {
    let s = input.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    // split into the name part and the number part (last token(s) with digits/colon)
    let mut split = s.len();
    for (i, ch) in s.char_indices().rev() {
        if ch.is_ascii_digit() || ch == ':' || ch == '-' || ch == ' ' || ch == '.' {
            split = i;
        } else {
            break;
        }
    }
    let (name, nums) = s.split_at(split);
    let name = name.trim().trim_end_matches('.').replace('.', "");
    if name.is_empty() {
        return None;
    }
    let nums = nums.trim();
    let mut chapter = 1u16;
    let mut verse = None;
    if !nums.is_empty() {
        let mut parts = nums.split(':');
        chapter = parts.next()?.trim().trim_matches('.').parse().ok()?;
        if let Some(v) = parts.next() {
            verse = v.split('-').next().and_then(|v| v.trim().parse().ok());
        }
    }
    let norm = |x: &str| x.to_lowercase().replace('.', "").replace(' ', "");
    let target = name.replace(' ', "");
    let mut best: Option<u8> = None;
    for (num, full, abbr) in names {
        if norm(full) == target || norm(abbr) == target {
            best = Some(*num);
            break;
        }
    }
    if best.is_none() {
        for (num, full, _) in names {
            if norm(full).starts_with(&target) && target.len() >= 2 {
                best = Some(*num);
                break;
            }
        }
    }
    best.map(|b| (b, chapter.max(1), verse))
}
