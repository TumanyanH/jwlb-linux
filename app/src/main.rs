// Study Library: a Bible and publication reader for Linux.
mod app;
mod bible;
mod catalog;
mod content;
mod epub;
mod fonts;
mod lang;
mod net;
mod paths;
mod reader;
mod settings;
mod theme;
mod ui_home;
mod ui_media;
mod ui_library;
mod ui_meetings;
mod ui_pubs;
mod ui_search;
mod ui_settings;
mod ui_study;
mod userdata;
mod widgets;
mod wol;
mod translate;
mod icons;
mod mpv;
mod media;
mod ui_bible;
mod ui_player;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<std::path::PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" => {
                if let Some(p) = args.get(i + 1) {
                    screenshot = Some(p.into());
                    i += 1;
                }
            }
            "--open" => {
                if let Some(p) = args.get(i + 1) {
                    let _ = app::START_PAGE.set(p.clone());
                    i += 1;
                }
            }
            "--update-catalog" => {
                let st = settings::Settings::load();
                let done = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
                let total = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
                println!("Downloading the publication list for language {}…", st.lang);
                match catalog::update(&st.lang, done, total) {
                    Ok(n) => println!("Done: {n} publications."),
                    Err(e) => {
                        eprintln!("Failed: {e}");
                        std::process::exit(1);
                    }
                }
                return Ok(());
            }
            "--list-bibles" => {
                // prints the Bibles the app offers for a language symbol (e.g. REA)
                let l = args.get(i + 1).cloned().unwrap_or_else(|| "E".into());
                match wol::bibles(&l) {
                    Ok(v) => v.iter().for_each(|(s, t)| println!("{s}\t{t}")),
                    Err(e) => eprintln!("{e}"),
                }
                return Ok(());
            }
            "--test-media" => {
                // plays a file for a few seconds without a window and reports what happened
                let u = args.get(i + 1).cloned().unwrap_or_default();
                let mut pb = media::Playback::start(&u, 0.0, true);
                let t0 = std::time::Instant::now();
                let mut frames = 0;
                let mut size = (0, 0);
                while t0.elapsed().as_secs_f64() < 8.0 {
                    if let Some(f) = pb.frame() {
                        frames += 1;
                        size = (f.size[0], f.size[1]);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                let before = pb.position();
                pb.seek(60.0);
                std::thread::sleep(std::time::Duration::from_secs(3));
                println!("frames {frames} size {size:?} position {before:.2}s duration {:.1}s after-seek {:.2}s error {:?}", pb.duration(), pb.position(), pb.error());
                return Ok(());
            }
            "--test-translate" => {
                // --test-translate <to> <text>
                let to = args.get(i + 1).cloned().unwrap_or_else(|| "en".into());
                let text = args.get(i + 2).cloned().unwrap_or_default();
                println!("{:?}", translate::translate(&text, &to, ""));
                return Ok(());
            }
            "--write-icon" => {
                // used by the installer builder to save the app icon as a PNG
                if let Some(p) = args.get(i + 1) {
                    let ic = app::app_icon();
                    if let Some(img) = image::RgbaImage::from_raw(ic.width, ic.height, ic.rgba) {
                        let _ = img.save(p);
                    }
                }
                return Ok(());
            }
            "--version" | "-V" => {
                println!("Study Library {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("Study Library {}\n\nUsage: study-library [--screenshot FILE.png] [--open bible:BOOK:CHAPTER | TAB] [--update-catalog]", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    let icon = app::app_icon();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Study Library")
        .with_app_id("study-library")
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([720.0, 480.0])
        .with_icon(icon);
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let options = eframe::NativeOptions {
        viewport,
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "Study Library",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot)))),
    )
}
