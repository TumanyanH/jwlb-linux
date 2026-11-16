//! Network access: the same public services the original uses
//! (b.jw-cdn.org pub-media and mediator APIs, app.jw-cdn.org catalogs, wol.jw.org).

use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const PUB_MEDIA: &str = "https://b.jw-cdn.org/apis/pub-media/GETPUBMEDIALINKS";
pub const MEDIATOR: &str = "https://b.jw-cdn.org/apis/mediator/v1";
pub const CATALOG_BASE: &str = "https://app.jw-cdn.org/catalogs/publications";
pub const WOL: &str = "https://wol.jw.org";

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent("StudyLibrary/1.0 (Linux)")
        .build()
}

pub fn get_string(url: &str) -> Result<String, String> {
    agent()
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())
}

pub fn get_json(url: &str) -> Result<serde_json::Value, String> {
    let s = get_string(url)?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub fn get_bytes(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let r = agent().get(url).call().map_err(|e| e.to_string())?;
    let mut v = Vec::new();
    r.into_reader().take(limit).read_to_end(&mut v).map_err(|e| e.to_string())?;
    Ok(v)
}

/// Get a page and also return the final address after redirects.
pub fn get_with_final_url(url: &str) -> Result<(String, String), String> {
    let r = agent().get(url).call().map_err(|e| e.to_string())?;
    let fin = r.get_url().to_string();
    let body = r.into_string().map_err(|e| e.to_string())?;
    Ok((fin, body))
}

/// Download to a file, reporting (done, total) bytes through the counters. Writes to a
/// temporary name first so a broken download never leaves a half file behind.
pub fn download(url: &str, dest: &std::path::Path, done: Arc<AtomicU64>, total: Arc<AtomicU64>) -> Result<(), String> {
    let r = agent().get(url).call().map_err(|e| e.to_string())?;
    if let Some(len) = r.header("Content-Length").and_then(|v| v.parse::<u64>().ok()) {
        total.store(len, Ordering::Relaxed);
    }
    let tmp = dest.with_extension("part");
    if let Some(p) = dest.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut rd = r.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = rd.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done.fetch_add(n as u64, Ordering::Relaxed);
    }
    f.flush().map_err(|e| e.to_string())?;
    drop(f);
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn urlencode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

/// The online library's address parts for a language: (locale, rsconf, lib), e.g. ("en", "r1", "lp-e").
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WolLocale {
    pub locale: String,
    pub r: String,
    pub lp: String,
}

pub fn wol_locale(lang_sym: &str) -> Result<WolLocale, String> {
    let cache = crate::paths::cache_dir().join(format!("wol_{}.json", crate::paths::safe_name(lang_sym)));
    if let Ok(s) = std::fs::read_to_string(&cache) {
        if let Ok(w) = serde_json::from_str::<WolLocale>(&s) {
            return Ok(w);
        }
    }
    if lang_sym == "E" {
        let w = WolLocale { locale: "en".into(), r: "r1".into(), lp: "lp-e".into() };
        return Ok(w);
    }
    // The original resolves languages through wol.jw.org/wol/finder (string in JWLibrary.dll).
    let (fin, body) = get_with_final_url(&format!("{WOL}/wol/finder?wtlocale={}", urlencode(lang_sym)))?;
    let mut found = parse_wol_url(&fin);
    if found.is_none() {
        // look for a canonical link in the page
        if let Some(i) = body.find("/wol/h/") {
            let start = body[..i].rfind('"').map(|x| x + 1).unwrap_or(i);
            let end = body[i..].find('"').map(|x| i + x).unwrap_or(body.len());
            found = parse_wol_url(&body[start..end]);
        }
    }
    let w = found.ok_or_else(|| "This language has no online library".to_string())?;
    let _ = std::fs::write(&cache, serde_json::to_string(&w).unwrap_or_default());
    Ok(w)
}

fn parse_wol_url(u: &str) -> Option<WolLocale> {
    let path = u.split("wol.jw.org").last().unwrap_or(u);
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    // /{locale}/wol/{kind}/{r}/{lp}/...
    let wi = parts.iter().position(|p| *p == "wol")?;
    let locale = if wi > 0 { parts[wi - 1].to_string() } else { return None };
    let r = parts.get(wi + 2)?.to_string();
    let lp = parts.get(wi + 3)?.to_string();
    if !r.starts_with('r') || !lp.starts_with("lp-") {
        return None;
    }
    Some(WolLocale { locale, r, lp })
}

/// Turn a catalog issue tag (20261100) into the API's issue parameter (202611).
pub fn issue_param(tag: i64) -> Option<String> {
    if tag <= 0 {
        None
    } else if tag % 100 == 0 {
        Some((tag / 100).to_string())
    } else {
        Some(tag.to_string())
    }
}

/// Ask pub-media for a publication's downloadable files in one of the given formats.
/// Returns (format, url, size).
pub fn find_pub_file(symbol: &str, issue: i64, lang: &str, formats: &[&str]) -> Result<(String, String, u64), String> {
    let mut url = format!(
        "{PUB_MEDIA}?output=json&pub={}&fileformat={}&alllangs=0&langwritten={}",
        urlencode(symbol),
        formats.join(","),
        urlencode(lang)
    );
    if let Some(i) = issue_param(issue) {
        url.push_str(&format!("&issue={i}"));
    }
    let v = get_json(&url)?;
    let files = v
        .get("files")
        .and_then(|f| f.get(lang))
        .ok_or_else(|| "Not available for download in this language".to_string())?;
    for fmt in formats {
        if let Some(arr) = files.get(*fmt).and_then(|a| a.as_array()) {
            // prefer the regular edition (not large print / braille)
            let pick = arr
                .iter()
                .find(|x| x.get("title").and_then(|t| t.as_str()).map(|t| t == "Regular").unwrap_or(false))
                .or_else(|| arr.first());
            if let Some(f) = pick {
                let u = f.pointer("/file/url").and_then(|u| u.as_str()).unwrap_or("").to_string();
                let size = f.get("filesize").and_then(|s| s.as_u64()).unwrap_or(0);
                if !u.is_empty() {
                    return Ok((fmt.to_string(), u, size));
                }
            }
        }
    }
    Err("Not available for download in this language".into())
}
