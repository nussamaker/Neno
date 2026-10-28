use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use crate::state;

#[derive(Default, Clone)]
pub struct Settings {
    pub values: BTreeMap<String, String>,
}

impl Settings {
    pub fn load() -> Self {
        let path = state::appdata_dir().join("settings.txt");
        let mut s = Self::default();
        if let Ok(txt) = fs::read_to_string(&path) {
            for line in txt.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    s.values.insert(k.to_string(), v.to_string());
                }
            }
        }
        s
    }

    pub fn save(&self) {
        let dir = state::appdata_dir();
        let _ = fs::create_dir_all(&dir);
        let mut out = String::new();
        for (k, v) in &self.values {
            out.push_str(k);
            out.push('=');
            out.push_str(v);
            out.push('\n');
        }
        let _ = fs::write(dir.join("settings.txt"), out);
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.values.get(key)
    }

    pub fn set(&mut self, key: &str, val: &str) {
        self.values.insert(key.to_string(), val.to_string());
    }
}

pub fn settings_path() -> PathBuf {
    state::appdata_dir().join("settings.txt")
}
