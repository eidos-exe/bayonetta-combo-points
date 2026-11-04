use std::{
    io::Write,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
static PATH: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);
static LINES: AtomicUsize = AtomicUsize::new(0);
pub fn init(folder: &std::path::Path) {
    let path = folder.join("bayonetta_runtime.log");
    let _ = std::fs::write(
        &path,
        format!("Bayonetta Combo Points {}\n", env!("CARGO_PKG_VERSION")),
    );
    *PATH.lock().unwrap() = Some(path);
}
pub fn trace(message: &str) {
    if LINES.fetch_add(1, Ordering::Relaxed) >= 128 {
        return;
    }
    if let Some(path) = PATH.lock().unwrap().as_ref() {
        if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(path) {
            let _ = writeln!(file, "{}", message);
        }
    }
}

// Reserve a separate small budget for match boundaries; waiting-room hits
// must not consume the information needed to diagnose the following match.
pub fn match_event(message: &str) {
    static EVENTS: AtomicUsize = AtomicUsize::new(0);
    if EVENTS.fetch_add(1, Ordering::Relaxed) >= 96 {
        return;
    }
    if let Some(path) = PATH.lock().unwrap().as_ref() {
        if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(path) {
            let _ = writeln!(file, "match {}", message);
        }
    }
}
