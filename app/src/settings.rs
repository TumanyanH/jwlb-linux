use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Publication language symbol (e.g. "E" for English).
    pub lang: String,
    pub theme: ThemeChoice,
    /// Reading text size as a percentage of the 12 pt (16 px) base, like the original's
    /// FontSizePercentage setting (default "121", JWLibrary.dll 0x185a82d8e, getter 0x186399b30).
    pub font_percent: f32,
    /// Last highlight colour used (original setting DefaultHighlightColor, default "0" = first colour,
    /// JWLibrary.dll 0x185a827eb). Stored as 0-based index into 1..=6.
    pub default_highlight: i64,
    /// Key symbol of the Bible used on the Bible tab (older setting, kept for compatibility).
    pub bible: String,
    /// Bibles added on the Bible tab (any language) and the one shown.
    pub bibles: Vec<crate::wol::BibleRef>,
    pub bible_current: usize,
    /// Preferred video resolution label (e.g. "720p").
    pub video_quality: String,
    pub show_footnotes: bool,
    /// Multiplies mouse-wheel and touchpad scrolling.
    pub scroll_speed: f32,
    /// Language selected text is translated into (translation codes, e.g. "hy").
    pub translate_to: String,
    pub stream_media: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lang: "E".into(),
            theme: ThemeChoice::System,
            font_percent: FONT_PERCENT_DEFAULT,
            default_highlight: 0,
            bible: "nwt".into(),
            bibles: vec![],
            bible_current: 0,
            video_quality: "720p".into(),
            show_footnotes: true,
            scroll_speed: 2.5,
            translate_to: crate::translate::system_default(),
            stream_media: true,
        }
    }
}

/// Base reading size: body{font-size:12pt} in the original's reader (PrimaryPane.bundle.js) = 16 px.
pub const BASE_TEXT_PX: f32 = 16.0;
pub const FONT_PERCENT_DEFAULT: f32 = 121.0;
/// Steps and limits of the text-size control (estimated: not found in the code).
pub const FONT_PERCENT_STEP: f32 = 10.0;
pub const FONT_PERCENT_MIN: f32 = 60.0;
pub const FONT_PERCENT_MAX: f32 = 300.0;

impl Settings {
    /// Reading text size in points: base × percentage (getter at 0x186399b30 multiplies by the stored percentage).
    pub fn text_size(&self) -> f32 {
        BASE_TEXT_PX * self.font_percent.clamp(FONT_PERCENT_MIN, FONT_PERCENT_MAX) / 100.0
    }
    fn path() -> std::path::PathBuf {
        crate::paths::data_dir().join("settings.json")
    }
    pub fn load() -> Settings {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), s);
        }
    }
}
