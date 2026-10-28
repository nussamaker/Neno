use std::fs;
use std::time::Duration;
use crate::{network, state};

/// A cached "no lyrics found" result is trusted for this long, then retried.
const MISS_TTL: Duration = Duration::from_secs(24 * 60 * 60);

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
    // Cache: a file containing "MISS" is a known miss (retried after MISS_TTL);
    // any other non-empty file is the lyrics themselves.
    if let Ok(cached) = fs::read_to_string(&cp) {
        if cached == "MISS" {
            let fresh = fs::metadata(&cp)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .map(|age| age < MISS_TTL)
                .unwrap_or(false);
            if fresh {
                return None;
            }
            // Stale miss: fall through and try the network again.
        } else if !cached.is_empty() {
            return Some(cached);
        }
    }

    let url = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        urlencoding(track),
        urlencoding(artist)
    );
    let resp = network::http_agent()
        .get(&url)
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

/// First human-readable line of an LRC/plain lyrics blob, with `[mm:ss.xx]` and
/// `[ar:...]`-style tags stripped. Used for the short snippet shown in the notch.
pub fn preview_line(lrc: &str) -> String {
    for line in lrc.lines() {
        let mut s = line.trim();
        while s.starts_with('[') {
            match s.find(']') {
                Some(i) => s = s[i + 1..].trim_start(),
                None => break,
            }
        }
        if !s.is_empty() {
            return s.to_string();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::preview_line;

    #[test]
    fn strips_timestamps_and_metadata_tags() {
        let lrc = "[ar:Someone]\n[ti:Song]\n[00:12.34] Hello there\n[00:15.00] Next line";
        assert_eq!(preview_line(lrc), "Hello there");
    }

    #[test]
    fn plain_lyrics_pass_through() {
        assert_eq!(preview_line("\n\nFirst line\nSecond"), "First line");
    }

    #[test]
    fn empty_when_no_text() {
        assert_eq!(preview_line("[00:01.00]\n[00:02.00]  "), "");
    }
}
