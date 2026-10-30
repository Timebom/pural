use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;

use crate::interfaces::{FollowUp, ToolResult};

const GEOCODE_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";

// WMO weather codes as noun phrases
fn sky() -> &'static HashMap<u32, &'static str> {
    static MAP: OnceLock<HashMap<u32, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            (0, "clear skies"),
            (1, "mostly clear skies"),
            (2, "partly cloudy skies"),
            (3, "overcast skies"),
            (45, "fog"),
            (48, "freezing fog"),
            (51, "light drizzle"),
            (53, "drizzle"),
            (55, "heavy drizzle"),
            (56, "light freezing drizzle"),
            (57, "freezing drizzle"),
            (61, "light rain"),
            (63, "rain"),
            (65, "heavy rain"),
            (66, "light freezing rain"),
            (67, "freezing rain"),
            (71, "light snow"),
            (73, "snow"),
            (75, "heavy snow"),
            (77, "snow grains"),
            (80, "light rain showers"),
            (81, "rain showers"),
            (82, "heavy rain showers"),
            (85, "light snow showers"),
            (86, "heavy snow showers"),
            (95, "a thunderstorm"),
            (96, "a thunderstorm with hail"),
            (99, "a heavy thunderstorm with hail"),
        ])
    })
}

fn days() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| HashSet::from(["today", "tomorrow"]))
}

fn noise() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| HashSet::from(["now", "right", "currently", "please"]))
}

fn leading() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        HashSet::from([
            "in", "at", "for", "the", "of", "like", "weather", "forecast",
        ])
    })
}

/// `'in mumbai tomorrow' -> ("mumbai", Some("tomorrow"))`
fn split_request(text: &str) -> (String, Option<String>) {
    let mut day = None;
    let mut words = Vec::new();

    for word in text.to_lowercase().split_whitespace() {
        if days().contains(word) {
            day = Some(word.to_string());
        } else if !noise().contains(word) {
            words.push(word.to_string());
        }
    }

    while words.first().map(|w| leading().contains(w.as_str())).unwrap_or(false) {
        words.remove(0);
    }

    (words.join(" "), day)
}

fn pick<'a>(results: &'a [Value], hint: &str) -> &'a Value {
    if !hint.is_empty() {
        for r in results {
            let labels = [
                r.get("country").and_then(|v| v.as_str()).unwrap_or(""),
                r.get("country_code").and_then(|v| v.as_str()).unwrap_or(""),
                r.get("admin1").and_then(|v| v.as_str()).unwrap_or(""),
            ];
            if labels.iter().any(|l| {
                !l.is_empty() && (l.to_lowercase().contains(hint) || hint.contains(&l.to_lowercase()))
            }) {
                return r;
            }
        }
    }
    &results[0]
}

fn deg(value: f64) -> String {
    let n = value.round() as i32;
    if n < 0 {
        format!("minus {}", n.abs())
    } else {
        n.to_string()
    }
}

pub struct WeatherTool {
    default_location: String,
    client: Client,
    geo_cache: std::sync::Mutex<HashMap<String, Value>>,
}

impl WeatherTool {
    pub fn new(default_location: &str, timeout_secs: f64) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs_f64(timeout_secs))
            .build()
            .expect("failed to build HTTP client");

        Self {
            default_location: default_location.trim().to_lowercase(),
            client,
            geo_cache: std::sync::Mutex::new(HashMap::new()),
        }
    }

    fn geocode(&self, place: &str) -> Option<Value> {
        {
            let cache = self.geo_cache.lock().unwrap();
            if let Some(hit) = cache.get(place) {
                return Some(hit.clone());
            }
        }

        let words: Vec<&str> = place.split_whitespace().collect();
        let min_end = words.len().saturating_sub(3).max(1);

        for end in (min_end..=words.len()).rev() {
            let name = words[..end].join(" ");
            let hint = words[end..].join(" ");

            let response = self
                .client
                .get(GEOCODE_URL)
                .query(&[
                    ("name", name.as_str()),
                    ("count", "5"),
                    ("language", "en"),
                    ("format", "json"),
                ])
                .send()
                .ok()?;

            if !response.status().is_success() {
                continue;
            }

            let body: Value = response.json().ok()?;
            let results = body.get("results")?.as_array()?;
            if results.is_empty() {
                continue;
            }

            let chosen = pick(results, &hint).clone();
            self.geo_cache
                .lock()
                .unwrap()
                .insert(place.to_string(), chosen.clone());
            return Some(chosen);
        }

        None
    }

    fn fetch(&self, geo: &Value, day: &str) -> Result<Value, ()> {
        let lat = geo.get("latitude")?.as_f64()?;
        let lon = geo.get("longitude")?.as_f64()?;

        let mut req = self.client.get(FORECAST_URL).query(&[
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            ("timezone", "auto".to_string()),
        ]);

        if day == "tomorrow" {
            req = req.query(&[
                (
                    "daily",
                    "weather_code,temperature_2m_max,temperature_2m_min,precipitation_sum",
                ),
                ("forecast_days", "2"),
            ]);
        } else {
            req = req.query(&[(
                "current",
                "temperature_2m,apparent_temperature,weather_code,relative_humidity_2m,wind_speed_10m,precipitation",
            )]);
        }

        let response = req.send().map_err(|_| ())?;
        if !response.status().is_success() {
            return Err(());
        }
        response.json().map_err(|_| ())
    }

    pub fn get_weather(&self, slots: &HashMap<String, String>) -> ToolResult {
        let (mut place, spoken_day) =
            split_request(slots.get("location").map(|s| s.as_str()).unwrap_or(""));

        let mut day = spoken_day
            .or_else(|| slots.get("day").cloned())
            .unwrap_or_else(|| "today".to_string());

        if !days().contains(day.as_str()) {
            day = "today".to_string();
        }

        if place.is_empty() {
            place = self.default_location.clone();
        }

        let ask = FollowUp::new("get_weather", "location")
            .with_slots(HashMap::from([("day".into(), day.clone())]));

        if place.is_empty() {
            return ToolResult::err("Which city?").with_follow_up(ask);
        }

        let geo = match self.geocode(&place) {
            Some(g) => g,
            None => {
                return ToolResult::err(format!(
                    "I couldn't find a place called {place}. Which city?"
                ))
                .with_follow_up(ask);
            }
        };

        let payload = match self.fetch(&geo, &day) {
            Ok(p) => p,
            Err(_) => {
                return ToolResult::err("I couldn't reach the weather service right now.");
            }
        };

        let name = geo
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&place)
            .to_string();

        if day == "tomorrow" {
            let daily = match payload.get("daily") {
                Some(d) => d,
                None => return ToolResult::err("I couldn't reach the weather service right now."),
            };

            let code = daily
                .get("weather_code")
                .and_then(|v| v.as_array())
                .and_then(|a| a.get(1))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;

            let sky_text = sky().get(&code).copied().unwrap_or("unsettled weather");

            let hi = daily
                .get("temperature_2m_max")
                .and_then(|v| v.as_array())
                .and_then(|a| a.get(1))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let lo = daily
                .get("temperature_2m_min")
                .and_then(|v| v.as_array())
                .and_then(|a| a.get(1))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let mut data = HashMap::new();
            data.insert("location".to_string(), name.clone());
            data.insert("high".to_string(), hi.to_string());
            data.insert("low".to_string(), lo.to_string());
            data.insert("code".to_string(), code.to_string());

            return ToolResult::ok(format!(
                "Tomorrow in {name}, expect {sky_text}, with a high of {} and a low of {} degrees.",
                deg(hi),
                deg(lo)
            ))
            .with_data(data);
        }

        // current conditions
        let cur = match payload.get("current") {
            Some(c) => c,
            None => return ToolResult::err("I couldn't reach the weather service right now."),
        };

        let temp = cur
            .get("temperature_2m")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let feels = cur
            .get("apparent_temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(temp);
        let code = cur
            .get("weather_code")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let sky_text = sky().get(&code).copied().unwrap_or("unsettled weather");

        let mut message = format!("In {name} it's {} degrees with {sky_text}.", deg(temp));
        if (feels.round() - temp.round()).abs() >= 2.0 {
            message.push_str(&format!(" It feels like {}.", deg(feels)));
        }

        let mut data = HashMap::new();
        data.insert("location".to_string(), name);
        data.insert("temperature".to_string(), temp.to_string());
        data.insert("feels_like".to_string(), feels.to_string());
        data.insert(
            "humidity".to_string(),
            cur.get("relative_humidity_2m")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
                .to_string(),
        );
        data.insert(
            "wind_speed".to_string(),
            cur.get("wind_speed_10m")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
                .to_string(),
        );
        data.insert("code".to_string(), code.to_string());

        ToolResult::ok(message).with_data(data)
    }
}
