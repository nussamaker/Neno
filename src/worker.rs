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
///   weather_lat=<-90..90>   latitude  (default: London)
///   weather_lon=<-180..180> longitude (default: London)
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
    let lat = coord("weather_lat", -90.0, 90.0, DEFAULT_LAT);
    let lon = coord("weather_lon", -180.0, 180.0, DEFAULT_LON);

    if let Some(w) = weather::fetch_weather(lat, lon) {
        notch::set_weather(&format!("{:.0}C {}", w.temperature, w.condition));
    }
}
