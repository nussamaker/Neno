//! Background worker: all slow or blocking lookups (WinRT, network) run here so
//! the UI thread can start pumping messages immediately and never stalls.
//! Results are published through the `notch::set_*` setters, which are
//! mutex-protected; the notch's 1-second timer picks them up on its next repaint.

use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::{bluetooth, lyrics, media, notch, notifications, settings, weather};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const DEVICE_REFRESH: Duration = Duration::from_secs(30);
const WEATHER_REFRESH: Duration = Duration::from_secs(10 * 60);

// Defaults used when `weather_lat` / `weather_lon` are not set in settings.txt.
const DEFAULT_LAT: f64 = 51.5074;
const DEFAULT_LON: f64 = -0.1278;

pub fn spawn() {
    let _ = thread::Builder::new()
        .name("neno-worker".into())
        .spawn(run);
}

fn due(last: &Option<Instant>, every: Duration) -> bool {
    last.map(|t| t.elapsed() >= every).unwrap_or(true)
}

fn run() {
    // WinRT calls (.get()) block, so this thread joins the MTA rather than the UI's apartment.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let _ = notifications::request_access();

    let mut media_key = String::new();
    let mut last_devices: Option<Instant> = None;
    let mut last_weather: Option<Instant> = None;

    loop {
        poll_media(&mut media_key);

        if due(&last_devices, DEVICE_REFRESH) {
            notch::set_bt_count(bluetooth::list_paired_devices().len());
            notch::set_notif_count(notifications::notification_count());
            last_devices = Some(Instant::now());
        }

        if due(&last_weather, WEATHER_REFRESH) {
            refresh_weather();
            last_weather = Some(Instant::now());
        }

        thread::sleep(POLL_INTERVAL);
    }
}

/// Update title/lyrics only when the track changes; clear them when playback ends.
fn poll_media(last_key: &mut String) {
    match media::MediaSession::playback() {
        Some(p) => notch::set_playback(p.position, p.duration, p.playing),
        None => notch::set_playback(0, 0, false),
    }
    match media::MediaSession::current() {
        Some(m) => {
            let key = format!("{}\u{1}{}", m.title, m.artist);
            if *last_key != key {
                notch::set_media_title(&format!("{} — {}", m.title, m.artist));
                notch::set_lyrics("");
                if let Some(l) = lyrics::fetch_lyrics(&m.title, &m.artist) {
                    notch::set_lyrics(&lyrics::preview_line(&l));
                }
                *last_key = key;
            }
        }
        None => {
            if !last_key.is_empty() {
                notch::set_media_title("");
                notch::set_lyrics("");
                last_key.clear();
            }
        }
    }
}

/// Settings keys (in %APPDATA%\Neno\settings.txt):
///   weather_enabled=false   disable weather (no network call)
///   weather_lat=<-90..90>   latitude  (overrides auto-location)
///   weather_lon=<-180..180> longitude (overrides auto-location)
/// Without explicit coordinates the worker asks ipwho.is once for an approximate
/// location and caches it in location.txt for 30 days.
/// Settings are re-read on every refresh, so edits apply within ~10 minutes.
fn refresh_weather() {
    let s = settings::Settings::load();
    if s.get("weather_enabled").map(|v| v.trim() == "false").unwrap_or(false) {
        notch::set_weather("");
        return;
    }
    let coord = |key: &str, lo: f64, hi: f64, default: f64| -> f64 {
        s.get(key)
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= lo && *v <= hi)
            .unwrap_or(default)
    };
    let explicit = s.get("weather_lat").is_some() && s.get("weather_lon").is_some();
    let (lat, lon) = if explicit {
        (
            coord("weather_lat", -90.0, 90.0, DEFAULT_LAT),
            coord("weather_lon", -180.0, 180.0, DEFAULT_LON),
        )
    } else {
        auto_location().unwrap_or((DEFAULT_LAT, DEFAULT_LON))
    };

    if let Some(w) = weather::fetch_weather(lat, lon) {
        notch::set_weather(&format!("{:.0}°C {}", w.temperature, w.condition));
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

const LOCATION_TTL: u64 = 30 * 24 * 60 * 60;

/// Approximate the user's coordinates from their public IP, cached for a month
/// so the service is contacted at most once per location change.
fn auto_location() -> Option<(f64, f64)> {
    let path = crate::state::appdata_dir().join("location.txt");
    if let Ok(txt) = std::fs::read_to_string(&path) {
        let mut lat = f64::NAN;
        let mut lon = f64::NAN;
        let mut ts = 0u64;
        for line in txt.lines() {
            if let Some(v) = line.strip_prefix("lat=") {
                lat = v.parse().unwrap_or(f64::NAN);
            } else if let Some(v) = line.strip_prefix("lon=") {
                lon = v.parse().unwrap_or(f64::NAN);
            } else if let Some(v) = line.strip_prefix("ts=") {
                ts = v.parse().unwrap_or(0);
            }
        }
        if lat.is_finite()
            && lon.is_finite()
            && now_secs().saturating_sub(ts) < LOCATION_TTL
        {
            return Some((lat, lon));
        }
    }

    let resp = crate::network::http_agent().get("https://ipwho.is/").call().ok()?;
    let body = resp.into_string().ok()?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    if json.get("success").and_then(|v| v.as_bool()) != Some(true) {
        return None;
    }
    let lat = json.get("latitude")?.as_f64()?;
    let lon = json.get("longitude")?.as_f64()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let _ = std::fs::create_dir_all(crate::state::appdata_dir());
    let _ = std::fs::write(&path, format!("lat={}\nlon={}\nts={}\n", lat, lon, now_secs()));
    Some((lat, lon))
}
