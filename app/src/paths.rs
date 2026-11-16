use std::path::PathBuf;

/// Where the app keeps its data: $XDG_DATA_HOME/study-library (usually ~/.local/share/study-library).
pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".local/share")
    });
    let d = base.join("study-library");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn sub(name: &str) -> PathBuf {
    let d = data_dir().join(name);
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn pubs_dir(lang: &str) -> PathBuf {
    let d = sub("publications").join(lang);
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn media_dir() -> PathBuf {
    sub("media")
}

pub fn cache_dir() -> PathBuf {
    sub("cache")
}

pub fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect()
}

/// Open a file or address with the desktop's default program.
pub fn open_external(target: &str) {
    let _ = std::process::Command::new("xdg-open")
        .arg(target)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
