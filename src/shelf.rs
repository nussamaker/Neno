use std::fs;
use std::path::Path;
use crate::state;

#[derive(Default, Clone)]
pub struct Shelf {
    pub files: Vec<String>,
}

impl Shelf {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.refresh();
        s
    }

    pub fn refresh(&mut self) {
        self.files.clear();
        let dir = state::localappdata_shelf_dir();
        if let Ok(entries) = fs::read_dir(&dir) {
            let mut items: Vec<(Option<std::time::SystemTime>, String)> = entries
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_str()?.to_string();
                    let modified = e.metadata().ok().and_then(|m| m.modified().ok());
                    Some((modified, name))
                })
                .collect();
            // Newest first; files without a timestamp go last, ties break by name.
            items.sort_by(|a, b| match (a.0, b.0) {
                (Some(x), Some(y)) => y.cmp(&x).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.1.to_lowercase().cmp(&b.1.to_lowercase()),
            });
            self.files = items.into_iter().map(|(_, name)| name).collect();
        }
    }

    pub fn add_file(&self, src: &Path) -> std::io::Result<()> {
        let dir = state::localappdata_shelf_dir();
        fs::create_dir_all(&dir)?;
        if let Some(name) = src.file_name() {
            let dest = dir.join(name);
            fs::copy(src, dest)?;
        }
        Ok(())
    }

    pub fn create_url_shortcut(&self, title: &str, url: &str) -> std::io::Result<()> {
        let dir = state::localappdata_shelf_dir();
        fs::create_dir_all(&dir)?;
        // Sanitize title: strip path separators and control chars.
        let safe_title: String = title.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect();
        let safe_title = if safe_title.is_empty() { "shortcut".to_string() } else { safe_title };
        // Sanitize url: strip CR/LF to prevent header injection.
        let safe_url: String = url.chars().filter(|c| *c != '\r' && *c != '\n').collect();
        let content = format!("[InternetShortcut]\r\nURL={}\r\n", safe_url);
        let path = dir.join(format!("{}.url", safe_title));
        fs::write(path, content)
    }
}

// Legacy Win32 drag-and-drop helpers (WM_DROPFILES). Call DragAcceptFiles in window creation.
pub fn handle_drop_files(hdrop: isize) {
    use windows::Win32::UI::Shell::DragQueryFileW;
    use windows::Win32::UI::Shell::DragFinish;
    unsafe {
        let count = DragQueryFileW(windows::Win32::UI::Shell::HDROP(hdrop as *mut _), 0xFFFFFFFF, None);
        for i in 0..count {
            let mut buf = vec![0u16; 1024];
            let len = DragQueryFileW(windows::Win32::UI::Shell::HDROP(hdrop as *mut _), i, Some(&mut buf));
            if len > 0 {
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                // Avoid silent overwrite: append suffix if dest exists.
                let src = std::path::Path::new(&path);
                let dir = state::localappdata_shelf_dir();
                let _ = std::fs::create_dir_all(&dir);
                if let Some(name) = src.file_name() {
                    let mut dest = dir.join(name);
                    if dest.exists() {
                        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
                        let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("");
                        let mut n = 1;
                        loop {
                            let newname = if ext.is_empty() {
                                format!("{}_{}", stem, n)
                            } else {
                                format!("{}_{}.{}", stem, n, ext)
                            };
                            dest = dir.join(newname);
                            if !dest.exists() { break; }
                            n += 1;
                        }
                    }
                    let _ = std::fs::copy(src, dest);
                }
            }
        }
        DragFinish(windows::Win32::UI::Shell::HDROP(hdrop as *mut _));
    }
}
