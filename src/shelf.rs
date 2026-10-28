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
            for e in entries.flatten() {
                if let Some(name) = e.file_name().to_str() {
                    self.files.push(name.to_string());
                }
            }
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
        let content = format!("[InternetShortcut]\r\nURL={}\r\n", url);
        let path = dir.join(format!("{}.url", title));
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
            let mut buf = vec![0u16; 260];
            let len = DragQueryFileW(windows::Win32::UI::Shell::HDROP(hdrop as *mut _), i, Some(&mut buf));
            if len > 0 {
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                let _ = Shelf::new().add_file(std::path::Path::new(&path));
            }
        }
        DragFinish(windows::Win32::UI::Shell::HDROP(hdrop as *mut _));
    }
}
