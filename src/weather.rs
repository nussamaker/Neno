use std::fs;
use crate::{network, state};

const CACHE_TTL_SECS: u64 = 30 * 60;

#[derive(Clone, Default)]
pub struct Weather {
    pub temperature: f64,
    pub condition: String,
    pub fetched_at: u64,
}

fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn same_place(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}

pub fn fetch_weather(lat: f64, lon: f64) -> Option<Weather> {
    let cache_path = state::appdata_dir().join("weather.txt");
    // Cache for 30 minutes, but only if it was fetched for the same coordinates.
    if let Ok(txt) = fs::read_to_string(&cache_path) {
        let mut temp = 0.0;
        let mut cond = String::new();
        let mut ts = 0u64;
        let mut c_lat = f64::NAN;
        let mut c_lon = f64::NAN;
        for line in txt.lines() {
            if let Some(v) = line.strip_prefix("temp=") { temp = v.parse().unwrap_or(0.0); }
            else if let Some(v) = line.strip_prefix("cond=") { cond = v.to_string(); }
            else if let Some(v) = line.strip_prefix("ts=") { ts = v.parse().unwrap_or(0); }
            else if let Some(v) = line.strip_prefix("lat=") { c_lat = v.parse().unwrap_or(f64::NAN); }
            else if let Some(v) = line.strip_prefix("lon=") { c_lon = v.parse().unwrap_or(f64::NAN); }
        }
        // saturating_sub: a clock that moved backwards must not underflow.
        let age = now_secs().saturating_sub(ts);
        if age < CACHE_TTL_SECS && !cond.is_empty() && same_place(c_lat, lat) && same_place(c_lon, lon) {
            return Some(Weather { temperature: temp, condition: cond, fetched_at: ts });
        }
    }

    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code",
        lat, lon
    );
    let resp = network::http_agent().get(&url).call().ok()?;
    let body = resp.into_string().ok()?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let current = json.get("current")?;
    let temp = current.get("temperature_2m")?.as_f64()?;
    let code = current.get("weather_code")?.as_i64().unwrap_or(0);
    let cond = weather_code_to_text(code);

    let w = Weather { temperature: temp, condition: cond.clone(), fetched_at: now_secs() };
    let _ = fs::create_dir_all(state::appdata_dir());
    let _ = fs::write(
        &cache_path,
        format!("temp={}\ncond={}\nts={}\nlat={}\nlon={}\n", temp, cond, w.fetched_at, lat, lon),
    );
    Some(w)
}

fn weather_code_to_text(code: i64) -> String {
    match code {
        0 => "Clear",
        1..=3 => "Partly cloudy",
        45 | 48 => "Foggy",
        51..=55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61..=65 => "Rain",
        66 | 67 => "Freezing rain",
        71..=75 | 77 => "Snow",
        80..=82 => "Showers",
        85 | 86 => "Snow showers",
        95 | 96 | 99 => "Thunderstorm",
        _ => "Unknown",
    }
    .to_string()
}
