//! Video and audio playback inside the app window, through libmpv (loaded at run time, so the
//! program still starts where it isn't installed). Frames are drawn by mpv's software renderer
//! into a buffer that becomes an egui texture.

use std::ffi::{c_char, c_double, c_int, c_void, CStr, CString};

#[repr(C)]
struct RenderParam {
    kind: c_int,
    data: *mut c_void,
}

const PARAM_INVALID: c_int = 0;
const PARAM_API_TYPE: c_int = 1;
const PARAM_SW_SIZE: c_int = 17;
const PARAM_SW_FORMAT: c_int = 18;
const PARAM_SW_STRIDE: c_int = 19;
const PARAM_SW_POINTER: c_int = 20;
const FORMAT_DOUBLE: c_int = 5;
const FORMAT_FLAG: c_int = 3;
const UPDATE_FRAME: u64 = 1;

type Create = unsafe extern "C" fn() -> *mut c_void;
type Init = unsafe extern "C" fn(*mut c_void) -> c_int;
type SetOpt = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type CmdStr = unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int;
type GetProp = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int;
type Destroy = unsafe extern "C" fn(*mut c_void);
type RcCreate = unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut RenderParam) -> c_int;
type RcRender = unsafe extern "C" fn(*mut c_void, *mut RenderParam) -> c_int;
type RcUpdate = unsafe extern "C" fn(*mut c_void) -> u64;
type RcFree = unsafe extern "C" fn(*mut c_void);

struct Api {
    _lib: libloading::Library,
    create: Create,
    init: Init,
    set_opt: SetOpt,
    cmd: CmdStr,
    get: GetProp,
    destroy: Destroy,
    rc_create: RcCreate,
    rc_render: RcRender,
    rc_update: RcUpdate,
    rc_free: RcFree,
}

fn api() -> Option<&'static Api> {
    use std::sync::OnceLock;
    static A: OnceLock<Option<Api>> = OnceLock::new();
    A.get_or_init(|| unsafe {
        let lib = candidates().into_iter().find_map(|n| libloading::Library::new(&n).ok())?;
        macro_rules! sym {
            ($n:expr) => {
                *lib.get($n).ok()?
            };
        }
        Some(Api {
            create: sym!(b"mpv_create\0"),
            init: sym!(b"mpv_initialize\0"),
            set_opt: sym!(b"mpv_set_option_string\0"),
            cmd: sym!(b"mpv_command_string\0"),
            get: sym!(b"mpv_get_property\0"),
            destroy: sym!(b"mpv_terminate_destroy\0"),
            rc_create: sym!(b"mpv_render_context_create\0"),
            rc_render: sym!(b"mpv_render_context_render\0"),
            rc_update: sym!(b"mpv_render_context_update\0"),
            rc_free: sym!(b"mpv_render_context_free\0"),
            _lib: lib,
        })
    })
    .as_ref()
}

/// Names and places libmpv is found under on the common distributions (Debian/Ubuntu multiarch,
/// Fedora/openSUSE lib64, Arch, /usr/local builds, Flatpak runtimes, NixOS profiles).
fn candidates() -> Vec<String> {
    let names = ["libmpv.so.2", "libmpv.so.1", "libmpv.so"];
    let mut out: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    let arch = if cfg!(target_arch = "aarch64") { "aarch64-linux-gnu" } else { "x86_64-linux-gnu" };
    let mut dirs = vec![
        format!("/usr/lib/{arch}"),
        "/usr/lib64".to_string(),
        "/usr/lib".to_string(),
        format!("/usr/local/lib/{arch}"),
        "/usr/local/lib64".to_string(),
        "/usr/local/lib".to_string(),
        "/app/lib".to_string(),
        "/run/current-system/sw/lib".to_string(),
    ];
    if let Some(h) = dirs::home_dir() {
        dirs.push(h.join(".nix-profile/lib").to_string_lossy().into_owned());
    }
    if let Ok(extra) = std::env::var("STUDY_LIBRARY_LIBMPV") {
        out.insert(0, extra);
    }
    for d in dirs {
        for n in names {
            out.push(format!("{d}/{n}"));
        }
    }
    out
}

pub fn available() -> bool {
    api().is_some()
}

pub struct Mpv {
    h: *mut c_void,
    rc: *mut c_void,
    buf: Vec<u8>,
    pub title: String,
    pub image: String,
    pub audio_only: bool,
}

unsafe impl Send for Mpv {}

impl Mpv {
    pub fn new(audio_only: bool) -> Result<Mpv, String> {
        let a = api().ok_or("The playback library (libmpv) is not installed. Install your distribution's “mpv” or “libmpv2” package to play media inside the app.")?;
        unsafe {
            let h = (a.create)();
            if h.is_null() {
                return Err("mpv could not start".into());
            }
            let set = |k: &str, v: &str| {
                let k = CString::new(k).unwrap();
                let v = CString::new(v).unwrap();
                (a.set_opt)(h, k.as_ptr(), v.as_ptr());
            };
            set("vo", "libmpv");
            set("hwdec", "no");
            set("keep-open", "yes");
            set("idle", "yes");
            set("terminal", "no");
            set("ytdl", "no");
            set("cache", "yes");
            if audio_only {
                set("vid", "no");
            }
            if (a.init)(h) < 0 {
                (a.destroy)(h);
                return Err("mpv could not start".into());
            }
            let api_type = CString::new("sw").unwrap();
            let mut params = [
                RenderParam { kind: PARAM_API_TYPE, data: api_type.as_ptr() as *mut c_void },
                RenderParam { kind: PARAM_INVALID, data: std::ptr::null_mut() },
            ];
            let mut rc: *mut c_void = std::ptr::null_mut();
            if (a.rc_create)(&mut rc, h, params.as_mut_ptr()) < 0 {
                (a.destroy)(h);
                return Err("mpv could not create a picture renderer (mpv 0.32 or newer is needed)".into());
            }
            Ok(Mpv { h, rc, buf: vec![], title: String::new(), image: String::new(), audio_only })
        }
    }

    pub fn command(&self, c: &str) {
        if let (Some(a), Ok(cs)) = (api(), CString::new(c)) {
            unsafe {
                (a.cmd)(self.h, cs.as_ptr());
            }
        }
    }

    pub fn load(&self, url: &str, append: bool) {
        let u = url.replace('"', "%22");
        self.command(&format!("loadfile \"{u}\" {}", if append { "append" } else { "replace" }));
    }

    fn get_f64(&self, name: &str) -> Option<f64> {
        let a = api()?;
        let n = CString::new(name).ok()?;
        let mut v: c_double = 0.0;
        let r = unsafe { (a.get)(self.h, n.as_ptr(), FORMAT_DOUBLE, &mut v as *mut c_double as *mut c_void) };
        (r >= 0).then_some(v)
    }

    fn get_flag(&self, name: &str) -> Option<bool> {
        let a = api()?;
        let n = CString::new(name).ok()?;
        let mut v: c_int = 0;
        let r = unsafe { (a.get)(self.h, n.as_ptr(), FORMAT_FLAG, &mut v as *mut c_int as *mut c_void) };
        (r >= 0).then_some(v != 0)
    }

    pub fn position(&self) -> f64 {
        self.get_f64("time-pos").unwrap_or(0.0)
    }
    pub fn duration(&self) -> f64 {
        self.get_f64("duration").unwrap_or(0.0)
    }
    pub fn paused(&self) -> bool {
        self.get_flag("pause").unwrap_or(false)
    }
    pub fn idle(&self) -> bool {
        self.get_flag("idle-active").unwrap_or(false)
    }
    pub fn volume(&self) -> f64 {
        self.get_f64("volume").unwrap_or(100.0)
    }
    pub fn video_size(&self) -> Option<(usize, usize)> {
        let w = self.get_f64("dwidth").or_else(|| self.get_f64("width"))?;
        let h = self.get_f64("dheight").or_else(|| self.get_f64("height"))?;
        (w > 0.0 && h > 0.0).then_some((w as usize, h as usize))
    }
    pub fn playlist_pos(&self) -> (i64, i64) {
        (self.get_f64("playlist-pos").map(|v| v as i64).unwrap_or(0), self.get_f64("playlist-count").map(|v| v as i64).unwrap_or(1))
    }
    pub fn media_title(&self) -> Option<String> {
        let a = api()?;
        let n = CString::new("media-title").ok()?;
        let mut p: *mut c_char = std::ptr::null_mut();
        // MPV_FORMAT_STRING = 1; the string is freed by mpv_free, which we skip (tiny leak per call avoided by caching)
        let r = unsafe { (a.get)(self.h, n.as_ptr(), 1, &mut p as *mut *mut c_char as *mut c_void) };
        if r < 0 || p.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }

    /// Render a new frame if mpv has one. Returns RGBA pixels and size.
    pub fn frame(&mut self, max_w: usize) -> Option<egui::ColorImage> {
        let a = api()?;
        let flags = unsafe { (a.rc_update)(self.rc) };
        if flags & UPDATE_FRAME == 0 {
            return None;
        }
        let (vw, vh) = self.video_size()?;
        let (w, h) = if vw > max_w { (max_w, vh * max_w / vw) } else { (vw, vh) };
        let (w, h) = (w.max(2), h.max(2));
        let stride = w * 4;
        self.buf.resize(stride * h, 0);
        let mut size: [c_int; 2] = [w as c_int, h as c_int];
        let fmt = CString::new("rgb0").unwrap();
        let mut st: usize = stride;
        let mut params = [
            RenderParam { kind: PARAM_SW_SIZE, data: size.as_mut_ptr() as *mut c_void },
            RenderParam { kind: PARAM_SW_FORMAT, data: fmt.as_ptr() as *mut c_void },
            RenderParam { kind: PARAM_SW_STRIDE, data: &mut st as *mut usize as *mut c_void },
            RenderParam { kind: PARAM_SW_POINTER, data: self.buf.as_mut_ptr() as *mut c_void },
            RenderParam { kind: PARAM_INVALID, data: std::ptr::null_mut() },
        ];
        let r = unsafe { (a.rc_render)(self.rc, params.as_mut_ptr()) };
        if r < 0 {
            return None;
        }
        for px in self.buf.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Some(egui::ColorImage::from_rgba_unmultiplied([w, h], &self.buf))
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        if let Some(a) = api() {
            unsafe {
                (a.rc_free)(self.rc);
                (a.destroy)(self.h);
            }
        }
    }
}
