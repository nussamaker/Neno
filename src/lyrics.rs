use std::fs;
use crate::state;

pub fn cache_path(track: &str, artist: &str) -> std::path::PathBuf {
    let dir = state::appdata_dir().join("lyrics");
    let _ = fs::create_dir_all(&dir);
    // Use a hash to avoid collisions from punctuation differences.
    let key = format!("{}::{}", artist, track);
    let hash = simple_hash(&key);
    dir.join(format!("{:x}.lrc", hash))
}

fn simple_hash(s: &str) -> u64 {
    // FNV-1a 64-bit
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn fetch_lyrics(track: &str, artist: &str) -> Option<String> {
    let cp = cache_path(track, artist);
    // Cache: empty file = known miss; non-empty = lyrics.
    if let Ok(cached) = fs::read_to_string(&cp) {
        if cached == "MISS" {
            return None;
        }
        if !cached.is_empty() {
            return Some(cached);
        }
    }

    let url = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        urlencoding(track),
        urlencoding(artist)
    );
    let resp = ureq::get(&url)
        .set("User-Agent", "Neno-Notch/1.0 (Windows)")
        .call()
        .ok()?;
    let body = resp.into_string().ok()?;

    // Parse JSON array; take first result.
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let arr = json.as_array()?;
    let first = arr.first()?;
    // Prefer syncedLyrics, then plainLyrics.
    let lyrics = first.get("syncedLyrics")
        .or_else(|| first.get("plainLyrics"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty() && s != "null");

    // Cache result (including misses).
    let _ = fs::create_dir_all(cp.parent().unwrap());
    match &lyrics {
        Some(l) => { let _ = fs::write(&cp, l); }
        None => { let _ = fs::write(&cp, "MISS"); }
    }
    lyrics
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
