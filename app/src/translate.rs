//! Translation of selected text (online). Uses Google's public dictionary-extension endpoint
//! (no account; source language detected automatically), and MyMemory as a fallback.

/// Languages offered for translation: (code, English name).
pub const LANGS: [(&str, &str); 72] = [
    ("af", "Afrikaans"), ("sq", "Albanian"), ("am", "Amharic"), ("ar", "Arabic"), ("hy", "Armenian"),
    ("az", "Azerbaijani"), ("eu", "Basque"), ("be", "Belarusian"), ("bn", "Bengali"), ("bg", "Bulgarian"),
    ("ca", "Catalan"), ("zh-CN", "Chinese (Simplified)"), ("zh-TW", "Chinese (Traditional)"), ("hr", "Croatian"),
    ("cs", "Czech"), ("da", "Danish"), ("nl", "Dutch"), ("en", "English"), ("eo", "Esperanto"), ("et", "Estonian"),
    ("fi", "Finnish"), ("fr", "French"), ("ka", "Georgian"), ("de", "German"), ("el", "Greek"), ("gu", "Gujarati"),
    ("ht", "Haitian Creole"), ("he", "Hebrew"), ("hi", "Hindi"), ("hu", "Hungarian"), ("is", "Icelandic"),
    ("id", "Indonesian"), ("ga", "Irish"), ("it", "Italian"), ("ja", "Japanese"), ("kn", "Kannada"),
    ("kk", "Kazakh"), ("km", "Khmer"), ("ko", "Korean"), ("ky", "Kyrgyz"), ("lo", "Lao"), ("lv", "Latvian"),
    ("lt", "Lithuanian"), ("mk", "Macedonian"), ("ms", "Malay"), ("ml", "Malayalam"), ("mt", "Maltese"),
    ("mr", "Marathi"), ("mn", "Mongolian"), ("ne", "Nepali"), ("no", "Norwegian"), ("fa", "Persian"),
    ("pl", "Polish"), ("pt", "Portuguese"), ("pa", "Punjabi"), ("ro", "Romanian"), ("ru", "Russian"),
    ("sr", "Serbian"), ("si", "Sinhala"), ("sk", "Slovak"), ("sl", "Slovenian"), ("es", "Spanish"),
    ("sw", "Swahili"), ("sv", "Swedish"), ("tl", "Tagalog"), ("ta", "Tamil"), ("te", "Telugu"), ("th", "Thai"),
    ("tr", "Turkish"), ("uk", "Ukrainian"), ("ur", "Urdu"), ("vi", "Vietnamese"),
];

pub fn name(code: &str) -> String {
    LANGS.iter().find(|(c, _)| *c == code).map(|(_, n)| n.to_string()).unwrap_or_else(|| code.to_string())
}

/// The computer's language (from $LANG), used as the first default.
pub fn system_default() -> String {
    let l = std::env::var("LANG").unwrap_or_default();
    let code = l.split(['_', '.', '@']).next().unwrap_or("").to_lowercase();
    if !code.is_empty() && code != "c" && code != "posix" && LANGS.iter().any(|(c, _)| *c == code) {
        code
    } else {
        "en".into()
    }
}

/// Split long text into pieces of at most `max` bytes, at sentence or word ends.
fn chunks(text: &str, max: usize) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for part in text.split_inclusive(['.', '!', '?', '\n', ';']) {
        if cur.len() + part.len() > max && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if part.len() > max {
            for w in part.split_inclusive(' ') {
                if cur.len() + w.len() > max && !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                cur.push_str(w);
            }
        } else {
            cur.push_str(part);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn google(piece: &str, to: &str) -> Result<String, String> {
    let url = format!(
        "https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=auto&tl={}&q={}",
        crate::net::urlencode(to),
        crate::net::urlencode(piece)
    );
    let v = crate::net::get_json(&url)?;
    let mut out = String::new();
    if let Some(arr) = v.as_array() {
        for item in arr {
            match item {
                serde_json::Value::String(s) => out.push_str(s),
                serde_json::Value::Array(a) => {
                    if let Some(s) = a.first().and_then(|x| x.as_str()) {
                        out.push_str(s);
                    }
                }
                _ => {}
            }
        }
    }
    if out.trim().is_empty() {
        Err("no translation returned".into())
    } else {
        Ok(out)
    }
}

fn mymemory(piece: &str, from: &str, to: &str) -> Result<String, String> {
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={}|{}",
        crate::net::urlencode(piece),
        crate::net::urlencode(from),
        crate::net::urlencode(to)
    );
    let v = crate::net::get_json(&url)?;
    v.pointer("/responseData/translatedText").and_then(|t| t.as_str()).map(|s| s.to_string()).ok_or_else(|| "no translation returned".into())
}

/// Translate `text` into `to`. `from_hint` is the text's language when known (for the fallback).
pub fn translate(text: &str, to: &str, from_hint: &str) -> Result<String, String> {
    let text: String = text.chars().take(5000).collect();
    let mut out = vec![];
    for piece in chunks(&text, 1500) {
        match google(&piece, to) {
            Ok(t) => out.push(t),
            Err(_) => {
                let mut acc = String::new();
                for small in chunks(&piece, 450) {
                    acc.push_str(&mymemory(&small, if from_hint.is_empty() { "en" } else { from_hint }, to)?);
                    acc.push(' ');
                }
                out.push(acc.trim_end().to_string());
            }
        }
    }
    Ok(out.join(" ").split_whitespace().collect::<Vec<_>>().join(" "))
}
