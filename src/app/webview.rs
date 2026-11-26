//! Printables.com inside SolidWork: the system web view (WebKit on macOS, WebView2 on Windows)
//! laid over the 3D-Druck viewport. Downloads are caught into the work folder so the model can
//! be imported, sliced and printed straight away. Where there is no web view (Linux) the page
//! opens in the system browser instead.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const PRINTABLES: &str = "https://www.printables.com/model";

/// What the web view reports back to the app (its callbacks run on the main thread).
#[derive(Default)]
pub struct Shared {
    /// download URL → where it is saved
    pending: HashMap<String, PathBuf>,
    /// files that finished downloading, not yet picked up
    pub done: Vec<PathBuf>,
    /// file names that started downloading (for the status line)
    pub started: Vec<String>,
    pub failed: Vec<String>,
    /// links that wanted a new window: opened in the same view
    pub open: Vec<String>,
    pub title: String,
    pub loading: bool,
}

pub struct Web {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    view: wry::WebView,
    pub shared: Arc<Mutex<Shared>>,
    pub dir: PathBuf,
    shown: bool,
}

/// A file name for a download: the one the site suggests, else the last part of the URL.
fn file_name(url: &str, suggested: &std::path::Path) -> String {
    let s = suggested.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let s = if s.is_empty() || !s.contains('.') { url.split(['?', '#']).next().unwrap_or("").rsplit('/').next().unwrap_or("").to_string() } else { s };
    let clean: String = s.chars().map(|c| if c == '/' || c == '\\' || c == ':' || c.is_control() { '_' } else { c }).collect();
    if clean.trim().is_empty() {
        "download.stl".into()
    } else {
        clean
    }
}

/// A path in `dir` that does not exist yet ("name (2).stl").
fn free_path(dir: &std::path::Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    if !p.exists() {
        return p;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) => (&name[..i], &name[i..]),
        None => (name, ""),
    };
    (2..).map(|k| dir.join(format!("{} ({}){}", stem, k, ext))).find(|p| !p.exists()).unwrap_or(p)
}

impl Web {
    /// Creates the view as a child of the app window, hidden until it gets its place.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub fn new(window: &impl wry::raw_window_handle::HasWindowHandle, url: &str, dir: PathBuf) -> Result<Web, String> {
        let _ = std::fs::create_dir_all(&dir);
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (s1, s2, s3, s4, s5) = (shared.clone(), shared.clone(), shared.clone(), shared.clone(), shared.clone());
        let d = dir.clone();
        let view = wry::WebViewBuilder::new()
            .with_url(url)
            .with_visible(false)
            .with_accept_first_mouse(true)
            .with_download_started_handler(move |url, path| {
                let name = file_name(&url, path);
                let target = free_path(&d, &name);
                *path = target.clone();
                if let Ok(mut s) = s1.lock() {
                    s.started.push(name);
                    s.pending.insert(url, target);
                }
                true
            })
            .with_download_completed_handler(move |url, path, ok| {
                if let Ok(mut s) = s2.lock() {
                    let target = s.pending.remove(&url).or(path);
                    match (ok, target) {
                        (true, Some(p)) => s.done.push(p),
                        (_, p) => {
                            let n = p.and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string())).unwrap_or(url);
                            s.failed.push(n)
                        }
                    }
                }
            })
            .with_new_window_req_handler(move |url, _| {
                if let Ok(mut s) = s3.lock() {
                    s.open.push(url);
                }
                wry::NewWindowResponse::Deny
            })
            .with_document_title_changed_handler(move |t| {
                if let Ok(mut s) = s4.lock() {
                    s.title = t;
                }
            })
            .with_on_page_load_handler(move |ev, _| {
                if let Ok(mut s) = s5.lock() {
                    s.loading = matches!(ev, wry::PageLoadEvent::Started);
                }
            })
            .build_as_child(window)
            .map_err(|e| format!("Webansicht konnte nicht geöffnet werden: {}", e))?;
        Ok(Web { view, shared, dir, shown: false })
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    pub fn new<W>(_window: &W, url: &str, _dir: PathBuf) -> Result<Web, String> {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
        Err("Printables wurde im Browser geöffnet. Heruntergeladene Dateien über Datei ▸ Öffnen laden.".into())
    }

    /// Places the view (logical pixels from the window's top left) and shows it.
    pub fn place(&mut self, x: f32, y: f32, w: f32, h: f32) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let r = wry::Rect { position: wry::dpi::LogicalPosition::new(x as f64, y as f64).into(), size: wry::dpi::LogicalSize::new(w.max(1.0) as f64, h.max(1.0) as f64).into() };
            let _ = self.view.set_bounds(r);
        }
        let _ = (x, y, w, h);
        self.show(true);
    }

    pub fn show(&mut self, on: bool) {
        if self.shown != on {
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            let _ = self.view.set_visible(on);
            self.shown = on;
        }
    }

    pub fn back(&self) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let _ = self.view.go_back();
    }
    pub fn forward(&self) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let _ = self.view.go_forward();
    }
    pub fn reload(&self) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let _ = self.view.reload();
    }
    pub fn load(&self, url: &str) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let _ = self.view.load_url(url);
        let _ = url;
    }
    pub fn url(&self) -> String {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        return self.view.url().unwrap_or_default();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        String::new()
    }

    /// Opens links that asked for a new window, and hands over finished downloads.
    pub fn poll(&self) -> (Vec<PathBuf>, Vec<String>, Vec<String>) {
        let (open, done, started, failed) = match self.shared.lock() {
            Ok(mut s) => (std::mem::take(&mut s.open), std::mem::take(&mut s.done), std::mem::take(&mut s.started), std::mem::take(&mut s.failed)),
            Err(_) => return (vec![], vec![], vec![]),
        };
        if let Some(u) = open.last() {
            self.load(u);
        }
        (done, started, failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_names() {
        assert_eq!(file_name("https://files.printables.com/media/prints/1/stls/abc/benchy.stl?x=1", std::path::Path::new("")), "benchy.stl");
        assert_eq!(file_name("https://x/y", std::path::Path::new("/tmp/Part A.3mf")), "Part A.3mf");
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.stl"), b"").unwrap();
        assert_eq!(free_path(d.path(), "a.stl"), d.path().join("a (2).stl"));
    }
}
