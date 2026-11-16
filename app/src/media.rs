//! Built-in media playback with no outside player: MP4 (H.264 video + AAC sound) and MP3.
//! - files are streamed over HTTP with range requests (the publisher's MP4s keep their index at
//!   the front), or read from disk;
//! - pictures: the mp4 reader + openh264 decoder (bundled), converted to RGBA;
//! - sound: symphonia (pure Rust) decoders, played through the desktop's sound server
//!   (PulseAudio / PipeWire's PulseAudio service, loaded at run time). The sound position is the
//!   clock that pictures follow; without a sound server a wall clock is used instead.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- byte source

const CHUNK: u64 = 512 * 1024;

/// Read+Seek over a web address (range requests, cached chunks) or a local file.
pub struct Source {
    url: String,
    file: Option<std::fs::File>,
    len: u64,
    pos: u64,
    chunks: HashMap<u64, Arc<Vec<u8>>>,
}

impl Source {
    pub fn open(url: &str) -> Result<Source, String> {
        if !url.starts_with("http") {
            let f = std::fs::File::open(url).map_err(|e| e.to_string())?;
            let len = f.metadata().map(|m| m.len()).unwrap_or(0);
            return Ok(Source { url: url.into(), file: Some(f), len, pos: 0, chunks: HashMap::new() });
        }
        let r = agent().get(url).set("Range", "bytes=0-0").call().map_err(|e| e.to_string())?;
        let len = r
            .header("Content-Range")
            .and_then(|v| v.rsplit('/').next())
            .and_then(|v| v.parse::<u64>().ok())
            .or_else(|| r.header("Content-Length").and_then(|v| v.parse().ok()))
            .ok_or("the server did not report the file size")?;
        Ok(Source { url: url.into(), file: None, len, pos: 0, chunks: HashMap::new() })
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    fn chunk(&mut self, idx: u64) -> std::io::Result<Arc<Vec<u8>>> {
        if let Some(c) = self.chunks.get(&idx) {
            return Ok(c.clone());
        }
        let start = idx * CHUNK;
        let end = (start + CHUNK).min(self.len) - 1;
        let mut last_err = String::new();
        for _ in 0..3 {
            match agent().get(&self.url).set("Range", &format!("bytes={start}-{end}")).call() {
                Ok(r) => {
                    let mut v = Vec::with_capacity((end - start + 1) as usize);
                    if r.into_reader().read_to_end(&mut v).is_ok() {
                        let a = Arc::new(v);
                        if self.chunks.len() > 96 {
                            // keep memory bounded (~48 MB): drop chunks far from the reading position
                            let here = idx;
                            self.chunks.retain(|k, _| k.abs_diff(here) < 24);
                        }
                        self.chunks.insert(idx, a.clone());
                        return Ok(a);
                    }
                }
                Err(e) => last_err = e.to_string(),
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        Err(std::io::Error::other(last_err))
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout_read(Duration::from_secs(30)).build()
}

impl Read for Source {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(f) = &mut self.file {
            f.seek(SeekFrom::Start(self.pos))?;
            let n = f.read(buf)?;
            self.pos += n as u64;
            return Ok(n);
        }
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }
        let idx = self.pos / CHUNK;
        let c = self.chunk(idx)?;
        let off = (self.pos - idx * CHUNK) as usize;
        let n = buf.len().min(c.len().saturating_sub(off));
        buf[..n].copy_from_slice(&c[off..off + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for Source {
    fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
        let np = match p {
            SeekFrom::Start(x) => x as i64,
            SeekFrom::End(x) => self.len as i64 + x,
            SeekFrom::Current(x) => self.pos as i64 + x,
        };
        self.pos = np.max(0) as u64;
        Ok(self.pos)
    }
}

impl symphonia::core::io::MediaSource for Source {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

// ---------------------------------------------------------------- sound output

#[repr(C)]
struct PaSpec {
    format: i32,
    rate: u32,
    channels: u8,
}
#[repr(C)]
struct PaAttr {
    maxlength: u32,
    tlength: u32,
    prebuf: u32,
    minreq: u32,
    fragsize: u32,
}
type PaNew = unsafe extern "C" fn(*const std::ffi::c_char, *const std::ffi::c_char, i32, *const std::ffi::c_char, *const std::ffi::c_char, *const PaSpec, *const std::ffi::c_void, *const PaAttr, *mut i32) -> *mut std::ffi::c_void;
type PaWrite = unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_void, usize, *mut i32) -> i32;
type PaLatency = unsafe extern "C" fn(*mut std::ffi::c_void, *mut i32) -> u64;
type PaFree = unsafe extern "C" fn(*mut std::ffi::c_void);
type PaFlush = unsafe extern "C" fn(*mut std::ffi::c_void, *mut i32) -> i32;

struct PaApi {
    _l: libloading::Library,
    new: PaNew,
    write: PaWrite,
    latency: PaLatency,
    free: PaFree,
    flush: PaFlush,
}

fn pa() -> Option<&'static PaApi> {
    use std::sync::OnceLock;
    static A: OnceLock<Option<PaApi>> = OnceLock::new();
    A.get_or_init(|| unsafe {
        let l = ["libpulse-simple.so.0", "libpulse-simple.so"].iter().find_map(|n| libloading::Library::new(n).ok())?;
        Some(PaApi {
            new: *l.get(b"pa_simple_new\0").ok()?,
            write: *l.get(b"pa_simple_write\0").ok()?,
            latency: *l.get(b"pa_simple_get_latency\0").ok()?,
            free: *l.get(b"pa_simple_free\0").ok()?,
            flush: *l.get(b"pa_simple_flush\0").ok()?,
            _l: l,
        })
    })
    .as_ref()
}

struct PulseOut {
    h: *mut std::ffi::c_void,
}

impl PulseOut {
    fn new(rate: u32, channels: u8) -> Option<PulseOut> {
        let a = pa()?;
        let spec = PaSpec { format: 5 /* PA_SAMPLE_FLOAT32LE */, rate, channels };
        let bytes_per_sec = rate * channels as u32 * 4;
        let attr = PaAttr { maxlength: u32::MAX, tlength: bytes_per_sec / 5, prebuf: u32::MAX, minreq: u32::MAX, fragsize: u32::MAX };
        let name = std::ffi::CString::new("Study Library").unwrap();
        let stream = std::ffi::CString::new("Playback").unwrap();
        let mut err = 0;
        let h = unsafe { (a.new)(std::ptr::null(), name.as_ptr(), 1, std::ptr::null(), stream.as_ptr(), &spec, std::ptr::null(), &attr, &mut err) };
        // (not `then_some`: that would build — and then free — a Speaker around a null handle)
        if h.is_null() {
            None
        } else {
            Some(PulseOut { h })
        }
    }
    fn write(&self, data: &[f32]) {
        if let Some(a) = pa() {
            let mut e = 0;
            unsafe {
                (a.write)(self.h, data.as_ptr() as *const _, data.len() * 4, &mut e);
            }
        }
    }
    fn latency(&self) -> f64 {
        pa().map(|a| {
            let mut e = 0;
            unsafe { (a.latency)(self.h, &mut e) as f64 / 1_000_000.0 }
        })
        .unwrap_or(0.0)
    }
    fn flush(&self) {
        if let Some(a) = pa() {
            let mut e = 0;
            unsafe {
                (a.flush)(self.h, &mut e);
            }
        }
    }
}

impl Drop for PulseOut {
    fn drop(&mut self) {
        if self.h.is_null() {
            return;
        }
        if let Some(a) = pa() {
            unsafe { (a.free)(self.h) }
        }
    }
}

/// Sound through a player program reading raw samples (paplay, pw-cat or aplay). Used when the
/// sound library can't connect — for example inside a sandbox such as Craft Anything's window,
/// where the program on the host is reached through flatpak-spawn.
struct PipeOut {
    rate: u32,
    channels: u8,
    child: Option<std::process::Child>,
}

fn in_sandbox() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

impl PipeOut {
    fn spawn(rate: u32, channels: u8) -> Option<std::process::Child> {
        let r = rate.to_string();
        let c = channels.to_string();
        let candidates: Vec<Vec<String>> = vec![
            vec!["paplay".into(), "--raw".into(), "--format=float32le".into(), format!("--rate={r}"), format!("--channels={c}"), "--latency-msec=150".into()],
            vec!["pw-cat".into(), "--playback".into(), "--raw".into(), "--format".into(), "f32".into(), "--rate".into(), r.clone(), "--channels".into(), c.clone(), "-".into()],
            vec!["aplay".into(), "-q".into(), "-t".into(), "raw".into(), "-f".into(), "FLOAT_LE".into(), "-r".into(), r.clone(), "-c".into(), c.clone()],
        ];
        for cmd in candidates {
            let (prog, args): (String, Vec<String>) = if in_sandbox() {
                let mut a = vec!["--host".to_string()];
                a.extend(cmd.clone());
                ("flatpak-spawn".into(), a)
            } else {
                (cmd[0].clone(), cmd[1..].to_vec())
            };
            if !in_sandbox() && !std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(&cmd[0]).is_file())).unwrap_or(false) {
                continue;
            }
            if let Ok(mut ch) = std::process::Command::new(prog)
                .args(args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                std::thread::sleep(Duration::from_millis(120));
                if matches!(ch.try_wait(), Ok(None)) {
                    return Some(ch);
                }
            }
        }
        None
    }
    fn new(rate: u32, channels: u8) -> Option<PipeOut> {
        let ch = PipeOut::spawn(rate, channels)?;
        Some(PipeOut { rate, channels, child: Some(ch) })
    }
    fn write(&mut self, data: &[f32]) {
        use std::io::Write;
        if self.child.is_none() {
            self.child = PipeOut::spawn(self.rate, self.channels);
        }
        if let Some(ch) = &mut self.child {
            let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
            let ok = ch.stdin.as_mut().map(|s| s.write_all(&bytes).is_ok()).unwrap_or(false);
            if !ok {
                let _ = ch.kill();
                self.child = None;
            }
        }
    }
    fn flush(&mut self) {
        // drop what is queued: stop the player; a new one starts with the next samples
        if let Some(mut ch) = self.child.take() {
            let _ = ch.kill();
            let _ = ch.wait();
        }
    }
}

impl Drop for PipeOut {
    fn drop(&mut self) {
        self.flush();
    }
}

enum Speaker {
    Pulse(PulseOut),
    Pipe(PipeOut),
}

impl Speaker {
    fn new(rate: u32, channels: u8) -> Option<Speaker> {
        let dbg = std::env::var_os("STUDYLIB_DEBUG").is_some();
        if let Some(p) = PulseOut::new(rate, channels) {
            if dbg {
                eprintln!("sound: PulseAudio/PipeWire library");
            }
            return Some(Speaker::Pulse(p));
        }
        let r = PipeOut::new(rate, channels).map(Speaker::Pipe);
        if dbg {
            eprintln!("sound: {}", if r.is_some() { "player program" } else { "none" });
        }
        r
    }
    fn write(&mut self, data: &[f32]) {
        match self {
            Speaker::Pulse(p) => p.write(data),
            Speaker::Pipe(p) => p.write(data),
        }
    }
    /// Seconds of sound written but not heard yet (estimated for a player program).
    fn latency(&self) -> f64 {
        match self {
            Speaker::Pulse(p) => p.latency(),
            Speaker::Pipe(_) => 0.25,
        }
    }
    fn flush(&mut self) {
        match self {
            Speaker::Pulse(p) => p.flush(),
            Speaker::Pipe(p) => p.flush(),
        }
    }
}

// ---------------------------------------------------------------- shared state

fn f64_load(a: &AtomicU64) -> f64 {
    f64::from_bits(a.load(Ordering::Relaxed))
}
fn f64_store(a: &AtomicU64, v: f64) {
    a.store(v.to_bits(), Ordering::Relaxed)
}

pub struct Shared {
    stop: AtomicBool,
    paused: AtomicBool,
    /// increases on every seek; workers restart from `seek_to`
    gen: AtomicU64,
    seek_to: AtomicU64,
    /// playback position in seconds (the clock pictures follow)
    clock: AtomicU64,
    duration: AtomicU64,
    volume: AtomicU32,
    has_audio: AtomicBool,
    audio_done: AtomicBool,
    video_done: AtomicBool,
    pub error: Mutex<Option<String>>,
    started: Mutex<(Instant, f64)>,
}

pub struct Frame {
    pub pts: f64,
    pub gen: u64,
    pub image: egui::ColorImage,
}

/// One playing file.
pub struct Playback {
    sh: Arc<Shared>,
    frames: Receiver<Frame>,
    next: Option<Frame>,
    pub has_video: bool,
    pause_started: Option<Instant>,
}

impl Playback {
    pub fn start(url: &str, start_at: f64, want_video: bool) -> Playback {
        let sh = Arc::new(Shared {
            stop: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            gen: AtomicU64::new(0),
            seek_to: AtomicU64::new(start_at.to_bits()),
            clock: AtomicU64::new(start_at.to_bits()),
            duration: AtomicU64::new(0f64.to_bits()),
            volume: AtomicU32::new(1.0f32.to_bits()),
            has_audio: AtomicBool::new(false),
            audio_done: AtomicBool::new(false),
            video_done: AtomicBool::new(!want_video),
            error: Mutex::new(None),
            started: Mutex::new((Instant::now(), start_at)),
        });
        let (tx, rx) = sync_channel::<Frame>(6);
        let is_video = want_video && (url.contains(".mp4") || url.contains(".m4v") || url.contains(".mov"));
        {
            let sh = sh.clone();
            let u = url.to_string();
            std::thread::spawn(move || audio_worker(&u, sh));
        }
        if is_video {
            let sh = sh.clone();
            let u = url.to_string();
            std::thread::spawn(move || video_worker(&u, sh, tx));
        } else {
            sh.video_done.store(true, Ordering::Relaxed);
        }
        Playback { sh, frames: rx, next: None, has_video: is_video, pause_started: None }
    }

    pub fn position(&self) -> f64 {
        if self.sh.has_audio.load(Ordering::Relaxed) {
            f64_load(&self.sh.clock)
        } else {
            // wall clock when there is no sound
            if self.paused() {
                return f64_load(&self.sh.clock);
            }
            let (t0, p0) = *self.sh.started.lock().unwrap();
            let p = p0 + t0.elapsed().as_secs_f64();
            f64_store(&self.sh.clock, p);
            p
        }
    }
    pub fn duration(&self) -> f64 {
        f64_load(&self.sh.duration)
    }
    pub fn paused(&self) -> bool {
        self.sh.paused.load(Ordering::Relaxed)
    }
    pub fn set_paused(&mut self, p: bool) {
        if p == self.paused() {
            return;
        }
        if p {
            self.position();
            self.pause_started = Some(Instant::now());
        } else {
            *self.sh.started.lock().unwrap() = (Instant::now(), f64_load(&self.sh.clock));
            self.pause_started = None;
        }
        self.sh.paused.store(p, Ordering::Relaxed);
    }
    pub fn volume(&self) -> f32 {
        f32::from_bits(self.sh.volume.load(Ordering::Relaxed))
    }
    pub fn set_volume(&self, v: f32) {
        self.sh.volume.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    pub fn seek(&mut self, t: f64) {
        let d = self.duration();
        let t = if d > 0.0 { t.clamp(0.0, (d - 0.5).max(0.0)) } else { t.max(0.0) };
        f64_store(&self.sh.seek_to, t);
        f64_store(&self.sh.clock, t);
        *self.sh.started.lock().unwrap() = (Instant::now(), t);
        self.sh.audio_done.store(false, Ordering::Relaxed);
        if self.has_video {
            self.sh.video_done.store(false, Ordering::Relaxed);
        }
        self.sh.gen.fetch_add(1, Ordering::Relaxed);
        self.next = None;
    }
    pub fn finished(&self) -> bool {
        let d = self.duration();
        let ad = self.sh.audio_done.load(Ordering::Relaxed) || !self.sh.has_audio.load(Ordering::Relaxed);
        let vd = self.sh.video_done.load(Ordering::Relaxed);
        // only once the length is known and the position has reached the end: at the very start
        // nothing is "done" yet (this made a playlist jump straight to its last item)
        d > 0.0 && ad && vd && self.position() >= d - 0.6
    }
    pub fn error(&self) -> Option<String> {
        self.sh.error.lock().unwrap().clone()
    }

    /// The picture to show now, if a new one is due.
    pub fn frame(&mut self) -> Option<egui::ColorImage> {
        let now = self.position();
        let gen = self.sh.gen.load(Ordering::Relaxed);
        let mut out = None;
        loop {
            if self.next.is_none() {
                self.next = self.frames.try_recv().ok();
            }
            let Some(f) = self.next.take() else { break };
            if f.gen != gen {
                continue; // from before a seek
            }
            if f.pts <= now + 0.02 {
                out = Some(f.image);
                continue;
            }
            self.next = Some(f);
            break;
        }
        out
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.sh.stop.store(true, Ordering::Relaxed);
    }
}

fn set_error(sh: &Shared, e: String) {
    let mut g = sh.error.lock().unwrap();
    if g.is_none() {
        *g = Some(e);
    }
}

// ---------------------------------------------------------------- sound worker

fn audio_worker(url: &str, sh: Arc<Shared>) {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    use symphonia::core::units::Time;

    let src = match Source::open(url) {
        Ok(s) => s,
        Err(e) => {
            set_error(&sh, e);
            sh.audio_done.store(true, Ordering::Relaxed);
            return;
        }
    };
    let mss = MediaSourceStream::new(Box::new(src), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = url.rsplit('.').next() {
        hint.with_extension(ext);
    }
    let probed = match symphonia::default::get_probe().format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default()) {
        Ok(p) => p,
        Err(e) => {
            set_error(&sh, format!("This file's sound can't be read ({e})"));
            sh.audio_done.store(true, Ordering::Relaxed);
            return;
        }
    };
    let mut format = probed.format;
    let Some(track) = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).cloned() else {
        sh.audio_done.store(true, Ordering::Relaxed);
        return;
    };
    let tid = track.id;
    let tb = track.codec_params.time_base;
    if let (Some(n), Some(tb)) = (track.codec_params.n_frames, tb) {
        let t = tb.calc_time(n);
        let d = t.seconds as f64 + t.frac;
        if d > f64_load(&sh.duration) {
            f64_store(&sh.duration, d);
        }
    }
    let mut decoder = match symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()) {
        Ok(d) => d,
        Err(e) => {
            set_error(&sh, format!("This sound format isn't supported ({e})"));
            sh.audio_done.store(true, Ordering::Relaxed);
            return;
        }
    };
    let rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2) as u8;
    let speaker = Speaker::new(rate, channels);
    if speaker.is_none() {
        // no sound server: pictures follow the wall clock
        sh.audio_done.store(true, Ordering::Relaxed);
        return;
    }
    let mut speaker = speaker.unwrap();
    sh.has_audio.store(true, Ordering::Relaxed);
    let mut gen = u64::MAX;
    let mut buf: Option<SampleBuffer<f32>> = None;
    let mut written_end = 0.0f64; // media time at the end of what was written
    loop {
        if sh.stop.load(Ordering::Relaxed) {
            speaker.flush();
            return;
        }
        let g = sh.gen.load(Ordering::Relaxed);
        if g != gen {
            gen = g;
            let t = f64_load(&sh.seek_to);
            speaker.flush();
            if t > 0.05 || g > 0 {
                let _ = format.seek(SeekMode::Coarse, SeekTo::Time { time: Time::from(t), track_id: Some(tid) });
                decoder.reset();
            }
            written_end = t;
            sh.audio_done.store(false, Ordering::Relaxed);
        }
        if sh.paused.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(15));
            continue;
        }
        // the clock: what has been written minus what is still waiting in the speaker
        // until the speaker has really started, the position stays where playback began
        let lat = speaker.latency();
        if lat > 0.0 {
            f64_store(&sh.clock, (written_end - lat).max(0.0));
        }
        if sh.audio_done.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => {
                sh.audio_done.store(true, Ordering::Relaxed);
                continue;
            }
        };
        if packet.track_id() != tid {
            continue;
        }
        let pts = tb.map(|tb| {
            let t = tb.calc_time(packet.ts());
            t.seconds as f64 + t.frac
        });
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let frames = decoded.frames();
                if buf.as_ref().map(|b| b.capacity() < decoded.capacity() * spec.channels.count()).unwrap_or(true) {
                    buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                }
                let b = buf.as_mut().unwrap();
                b.copy_interleaved_ref(decoded);
                let vol = f32::from_bits(sh.volume.load(Ordering::Relaxed));
                let mut samples = b.samples().to_vec();
                if vol < 0.999 {
                    for s in &mut samples {
                        *s *= vol;
                    }
                }
                speaker.write(&samples);
                written_end = pts.unwrap_or(written_end) + frames as f64 / rate as f64;
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(_) => {
                sh.audio_done.store(true, Ordering::Relaxed);
            }
        }
    }
}

// ---------------------------------------------------------------- picture worker

#[allow(dead_code)]
fn avcc_to_annexb(sample: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i + 4 <= sample.len() {
        let n = u32::from_be_bytes([sample[i], sample[i + 1], sample[i + 2], sample[i + 3]]) as usize;
        i += 4;
        if i + n > sample.len() {
            break;
        }
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&sample[i..i + n]);
        i += n;
    }
}

/// Minimal MP4 index for the picture track: where each sample is, its times and key frames,
/// and the H.264 parameter sets (avcC may come after other entries such as pasp).
struct VideoIndex {
    timescale: f64,
    duration: f64,
    nal_len: usize,
    sps_pps: Vec<Vec<u8>>,
    /// (file offset, size, decode time, composition offset) per sample, in decode order
    samples: Vec<(u64, u32, u64, i64)>,
    sync: Vec<u32>,
}

fn boxes(data: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = vec![];
    let mut i = 0usize;
    while i + 8 <= data.len() {
        let mut size = u32::from_be_bytes(data[i..i + 4].try_into().unwrap()) as u64;
        let ty: [u8; 4] = data[i + 4..i + 8].try_into().unwrap();
        let mut hdr = 8u64;
        if size == 1 && i + 16 <= data.len() {
            size = u64::from_be_bytes(data[i + 8..i + 16].try_into().unwrap());
            hdr = 16;
        } else if size == 0 {
            size = (data.len() - i) as u64;
        }
        if size < hdr || i as u64 + size > data.len() as u64 {
            break;
        }
        out.push((ty, &data[i + hdr as usize..i + size as usize]));
        i += size as usize;
    }
    out
}

fn child<'a>(data: &'a [u8], ty: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data).into_iter().find(|(t, _)| t == ty).map(|(_, b)| b)
}

fn be32(b: &[u8], i: usize) -> u32 {
    b.get(i..i + 4).map(|x| u32::from_be_bytes(x.try_into().unwrap())).unwrap_or(0)
}

fn read_moov(src: &mut Source) -> Result<Vec<u8>, String> {
    let mut pos = 0u64;
    let len = src.len();
    while pos + 8 <= len {
        src.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
        let mut h = [0u8; 16];
        src.read_exact(&mut h[..8]).map_err(|e| e.to_string())?;
        let mut size = u32::from_be_bytes(h[0..4].try_into().unwrap()) as u64;
        let mut hdr = 8u64;
        if size == 1 {
            src.read_exact(&mut h[8..16]).map_err(|e| e.to_string())?;
            size = u64::from_be_bytes(h[8..16].try_into().unwrap());
            hdr = 16;
        } else if size == 0 {
            size = len - pos;
        }
        if &h[4..8] == b"moov" {
            let mut v = vec![0u8; (size - hdr) as usize];
            src.read_exact(&mut v).map_err(|e| e.to_string())?;
            return Ok(v);
        }
        if size < 8 {
            break;
        }
        pos += size;
    }
    Err("no index (moov) in this video".into())
}

fn video_index(moov: &[u8]) -> Option<VideoIndex> {
    for (ty, trak) in boxes(moov) {
        if &ty != b"trak" {
            continue;
        }
        let mdia = child(trak, b"mdia")?;
        let hdlr = child(mdia, b"hdlr")?;
        if hdlr.get(8..12) != Some(b"vide") {
            continue;
        }
        let mdhd = child(mdia, b"mdhd")?;
        let (timescale, duration) = if mdhd[0] == 1 {
            (be32(mdhd, 20) as f64, u64::from_be_bytes(mdhd[24..32].try_into().ok()?) as f64)
        } else {
            (be32(mdhd, 12) as f64, be32(mdhd, 16) as f64)
        };
        let stbl = child(child(mdia, b"minf")?, b"stbl")?;
        // codec settings
        let stsd = child(stbl, b"stsd")?;
        let entry = boxes(&stsd[8..]).into_iter().next()?;
        if &entry.0 != b"avc1" && &entry.0 != b"avc3" {
            return None;
        }
        let avcc = child(entry.1.get(78..)?, b"avcC")?;
        let nal_len = (avcc[4] & 3) as usize + 1;
        let mut sps_pps = vec![];
        let mut i = 5;
        let nsps = (avcc[i] & 31) as usize;
        i += 1;
        for _ in 0..nsps {
            let l = u16::from_be_bytes([avcc[i], avcc[i + 1]]) as usize;
            sps_pps.push(avcc.get(i + 2..i + 2 + l)?.to_vec());
            i += 2 + l;
        }
        let npps = *avcc.get(i)? as usize;
        i += 1;
        for _ in 0..npps {
            let l = u16::from_be_bytes([avcc[i], avcc[i + 1]]) as usize;
            sps_pps.push(avcc.get(i + 2..i + 2 + l)?.to_vec());
            i += 2 + l;
        }
        // sizes
        let stsz = child(stbl, b"stsz")?;
        let fixed = be32(stsz, 4);
        let count = be32(stsz, 8) as usize;
        let sizes: Vec<u32> = (0..count).map(|k| if fixed != 0 { fixed } else { be32(stsz, 12 + k * 4) }).collect();
        // decode times
        let mut dts = Vec::with_capacity(count);
        if let Some(stts) = child(stbl, b"stts") {
            let n = be32(stts, 4) as usize;
            let mut t = 0u64;
            for e in 0..n {
                let c = be32(stts, 8 + e * 8);
                let d = be32(stts, 12 + e * 8) as u64;
                for _ in 0..c {
                    dts.push(t);
                    t += d;
                }
            }
        }
        dts.resize(count, *dts.last().unwrap_or(&0));
        // composition offsets (B-frames)
        let mut cts = vec![0i64; count];
        if let Some(ctts) = child(stbl, b"ctts") {
            let n = be32(ctts, 4) as usize;
            let mut k = 0usize;
            for e in 0..n {
                let c = be32(ctts, 8 + e * 8);
                let o = be32(ctts, 12 + e * 8) as i32 as i64;
                for _ in 0..c {
                    if k < count {
                        cts[k] = o;
                    }
                    k += 1;
                }
            }
        }
        // chunk offsets
        let offsets: Vec<u64> = if let Some(stco) = child(stbl, b"stco") {
            (0..be32(stco, 4) as usize).map(|k| be32(stco, 8 + k * 4) as u64).collect()
        } else {
            let co = child(stbl, b"co64")?;
            (0..be32(co, 4) as usize).map(|k| u64::from_be_bytes(co[8 + k * 8..16 + k * 8].try_into().unwrap())).collect()
        };
        let stsc = child(stbl, b"stsc")?;
        let runs: Vec<(u32, u32)> = (0..be32(stsc, 4) as usize).map(|e| (be32(stsc, 8 + e * 12), be32(stsc, 12 + e * 12))).collect();
        let mut samples = Vec::with_capacity(count);
        let mut sidx = 0usize;
        for (ci, off) in offsets.iter().enumerate() {
            let chunk_no = ci as u32 + 1;
            let per = runs.iter().rev().find(|(first, _)| *first <= chunk_no).map(|r| r.1).unwrap_or(1);
            let mut o = *off;
            for _ in 0..per {
                if sidx >= count {
                    break;
                }
                samples.push((o, sizes[sidx], dts[sidx], cts[sidx]));
                o += sizes[sidx] as u64;
                sidx += 1;
            }
        }
        let sync = child(stbl, b"stss").map(|st| (0..be32(st, 4) as usize).map(|k| be32(st, 8 + k * 4)).collect()).unwrap_or_default();
        return Some(VideoIndex { timescale: timescale.max(1.0), duration: duration / timescale.max(1.0), nal_len, sps_pps, samples, sync });
    }
    None
}

fn to_annexb(sample: &[u8], nal_len: usize, out: &mut Vec<u8>) {
    let mut i = 0;
    while i + nal_len <= sample.len() {
        let mut n = 0usize;
        for k in 0..nal_len {
            n = (n << 8) | sample[i + k] as usize;
        }
        i += nal_len;
        if i + n > sample.len() {
            break;
        }
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&sample[i..i + n]);
        i += n;
    }
}

/// The decoder must not be flushed after every picture, or streams with reordered pictures
/// (B-frames, used by the 480p/720p files) break.
fn new_decoder() -> Result<openh264::decoder::Decoder, openh264::Error> {
    use openh264::decoder::{Decoder, DecoderConfig, Flush};
    Decoder::with_api_config(openh264::OpenH264API::from_source(), DecoderConfig::new().flush_after_decode(Flush::NoFlush))
}

fn video_worker(url: &str, sh: Arc<Shared>, tx: SyncSender<Frame>) {
    use openh264::formats::YUVSource;
    let fail = |sh: &Shared, e: String| {
        set_error(sh, e);
        sh.video_done.store(true, Ordering::Relaxed);
    };
    let mut src = match Source::open(url) {
        Ok(s) => s,
        Err(e) => return fail(&sh, e),
    };
    let moov = match read_moov(&mut src) {
        Ok(m) => m,
        Err(e) => return fail(&sh, format!("This video can't be read ({e})")),
    };
    let Some(ix) = video_index(&moov) else {
        return fail(&sh, "This video's picture format isn't supported".into());
    };
    if ix.duration > f64_load(&sh.duration) {
        f64_store(&sh.duration, ix.duration);
    }
    let mut header = vec![];
    for ps in &ix.sps_pps {
        header.extend_from_slice(&[0, 0, 0, 1]);
        header.extend_from_slice(ps);
    }
    let count = ix.samples.len() as u32;
    let mut decoder = match new_decoder() {
        Ok(dc) => dc,
        Err(e) => return fail(&sh, format!("The video decoder could not start ({e})")),
    };
    // pictures come out of the decoder in display order; their times are the sorted
    // composition times of the samples fed in (handles B-frames)
    let mut pending: std::collections::BinaryHeap<std::cmp::Reverse<i64>> = Default::default();
    let mut gen = u64::MAX;
    let mut next: u32 = 1;
    let mut skip_until = 0.0f64;
    let mut rgba: Vec<u8> = vec![];
    let mut annexb = Vec::with_capacity(256 * 1024);
    let mut data = Vec::new();
    loop {
        if sh.stop.load(Ordering::Relaxed) {
            return;
        }
        let g = sh.gen.load(Ordering::Relaxed);
        if g != gen {
            gen = g;
            let t = f64_load(&sh.seek_to);
            let target = (t * ix.timescale) as u64;
            let guess = ix.samples.iter().position(|s| s.2 >= target).map(|p| p as u32 + 1).unwrap_or(count).max(1);
            next = ix.sync.iter().rev().find(|s| **s <= guess).copied().unwrap_or(1);
            skip_until = t - 0.05;
            if let Ok(dc) = new_decoder() {
                decoder = dc;
            }
            pending.clear();
            let _ = decoder.decode(&header);
            sh.video_done.store(false, Ordering::Relaxed);
        }
        if next > count {
            sh.video_done.store(true, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(30));
            continue;
        }
        let (off, size, dts, cto) = ix.samples[(next - 1) as usize];
        next += 1;
        data.resize(size as usize, 0);
        if src.seek(SeekFrom::Start(off)).is_err() || src.read_exact(&mut data).is_err() {
            return fail(&sh, "The video stopped loading. Check the internet connection.".into());
        }
        pending.push(std::cmp::Reverse(dts as i64 + cto));
        annexb.clear();
        to_annexb(&data, ix.nal_len, &mut annexb);
        let decoded = match decoder.decode(&annexb) {
            Ok(Some(y)) => y,
            other => {
                if std::env::var_os("STUDYLIB_DEBUG").is_some() {
                    eprintln!("sample {} -> {:?}", next - 1, other.as_ref().map(|_| "none").map_err(|e| e.to_string()));
                }
                continue;
            }
        };
        let pts = pending.pop().map(|r| r.0).unwrap_or(dts as i64) as f64 / ix.timescale;
        if std::env::var_os("STUDYLIB_DEBUG").is_some() {
            eprintln!("decoded sample {} pts {:.3} clock {:.3}", next - 1, pts, f64_load(&sh.clock));
        }
        if pts < skip_until {
            continue;
        }
        let (w, h) = decoded.dimensions();
        rgba.resize(w * h * 4, 0);
        decoded.write_rgba8(&mut rgba);
        let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
        let mut f = Frame { pts, gen, image };
        loop {
            if sh.stop.load(Ordering::Relaxed) || sh.gen.load(Ordering::Relaxed) != gen {
                break;
            }
            match tx.try_send(f) {
                Ok(()) => break,
                Err(std::sync::mpsc::TrySendError::Full(back)) => {
                    f = back;
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return,
            }
        }
    }
}
