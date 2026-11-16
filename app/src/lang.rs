//! Publication languages: MEPS id, symbol, names. Table taken from the language codes the original ships.

#[derive(Clone, Debug)]
pub struct Language {
    pub id: i64,
    pub symbol: String,
    pub english: String,
    pub vernacular: String,
    pub ietf: String,
    pub sign: bool,
    pub rtl: bool,
}

static TABLE: &str = include_str!("../data/languages.tsv");

pub fn all() -> &'static Vec<Language> {
    use std::sync::OnceLock;
    static L: OnceLock<Vec<Language>> = OnceLock::new();
    L.get_or_init(|| {
        TABLE
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                if f.len() < 7 {
                    return None;
                }
                Some(Language {
                    id: f[0].parse().ok()?,
                    symbol: f[1].to_string(),
                    english: f[2].to_string(),
                    vernacular: f[3].to_string(),
                    ietf: f[4].to_string(),
                    sign: f[5] == "1",
                    rtl: f[6] == "1",
                })
            })
            .collect()
    })
}

pub fn by_symbol(sym: &str) -> Option<&'static Language> {
    all().iter().find(|l| l.symbol == sym)
}

pub fn by_id(id: i64) -> Option<&'static Language> {
    all().iter().find(|l| l.id == id)
}

pub fn meps_id(sym: &str) -> i64 {
    by_symbol(sym).map(|l| l.id).unwrap_or(0)
}
