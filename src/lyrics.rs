use std::fs;
use crate::state;

pub fn cache_path(track: &str, artist: &str) -> std::path::PathBuf {
    let dir = state::appdata_dir().join("lyrics");
    let _ = fs::create_dir_all(&dir);
    let safe = format!("{}_{}", artist, track)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>();
    dir.join(format!("{}.lrc", safe))
}

pub fn fetch_lyrics(track: &str, artist: &str) -> Option<String> {
    // Check cache first.
    let cp = cache_path(track, artist);
    if let Ok(cached) = fs::read_to_string(&cp) {
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
    // LRCLIB returns a JSON array; take first result's "plainLyrics" or "syncedLyrics".
    let text = extract_lyrics(&body);
    if let Some(ref l) = text {
        let _ = fs::write(&cp, l);
    }
    text
}

fn urlencoding(s: &str) -> String {
    // Minimal percent-encoding for query values.
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

fn extract_lyrics(json: &str) -> Option<String> {
    // Crude extraction: find "plainLyrics":"..." or "syncedLyrics":"...".
    for key in ["syncedLyrics", "plainLyrics"] {
        let needle = format!("\"{}\":\"", key);
        if let Some(i) = json.find(&needle) {
            let start = i + needle.len();
            // find closing quote (handle escaped \")
            let mut j = start;
            let bytes = json.as_bytes();
            while j < bytes.len() {
                if bytes[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if bytes[j] == b'"' {
                    break;
                }
                j += 1;
            }
            if j < bytes.len() {
                let raw = &json[start..j];
                // unescape minimal
                let unescaped = raw.replace("\\n", "\n").replace("\\\"", "\"");
                if !unescaped.trim().is_empty() && unescaped != "null" {
                    return Some(unescaped);
                }
            }
        }
    }
    None
}
