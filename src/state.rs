use std::path::PathBuf;

pub fn appdata_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| {
        // Fallback: %USERPROFILE%\AppData\Roaming
        std::env::var("USERPROFILE")
            .map(|u| format!("{}\\AppData\\Roaming", u))
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string())
    });
    PathBuf::from(base).join("Neno")
}

pub fn localappdata_shelf_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        std::env::var("USERPROFILE")
            .map(|u| format!("{}\\AppData\\Local", u))
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string())
    });
    PathBuf::from(base).join("Neno").join("Shelf")
}

pub fn ensure_dirs() {
    let _ = std::fs::create_dir_all(appdata_dir());
    let _ = std::fs::create_dir_all(localappdata_shelf_dir());
}

pub fn migrate_from_nusa() {
    // One-time migration of legacy Nusa data dirs to Neno.
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let old_app = PathBuf::from(&appdata).join("Nusa");
    let new_app = PathBuf::from(&appdata).join("Neno");
    if old_app.exists() && !new_app.exists() {
        let _ = std::fs::rename(&old_app, &new_app);
    }
    let old_shelf = PathBuf::from(&local).join("Nusa").join("Shelf");
    let new_shelf = PathBuf::from(&local).join("Neno").join("Shelf");
    if old_shelf.exists() && !new_shelf.exists() {
        let _ = std::fs::create_dir_all(new_shelf.parent().unwrap());
        let _ = std::fs::rename(&old_shelf, &new_shelf);
    }
}
