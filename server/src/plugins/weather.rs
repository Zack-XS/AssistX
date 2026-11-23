//! Weather via Open-Meteo (no API key).

use super::{ActionInfo, Plugin, ToolResult, info, str_arg};
use serde_json::{Value, json};
use std::collections::BTreeMap;


pub struct Weather;

fn get_json(url: &str, query: &[(&str, &str)]) -> Result<Value, String> {
    let agent = super::http_agent(15, 3);
    let mut req = agent.get(url);
    for (k, v) in query {
        req = req.query(*k, *v);
    }
    let s = req.call().map_err(|e| e.to_string())?.body_mut().read_to_string().map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

impl Plugin for Weather {
    fn name(&self) -> &'static str {
        "weather"
    }
    fn actions(&self) -> BTreeMap<&'static str, ActionInfo> {
        BTreeMap::from([("now", info("Current weather and 3 day outlook", &[("place", "city name")]))])
    }
    fn call(&self, action: &str, args: &Value) -> Option<ToolResult> {
        if action != "now" {
            return None;
        }
        Some((|| {
            let place = str_arg(args, "place")?;
            let g = get_json(
                "https://geocoding-api.open-meteo.com/v1/search",
                &[("name", place), ("count", "1")],
            )?;
            let loc = g["results"].get(0).ok_or("place not found")?;
            let (lat, lon) = (loc["latitude"].to_string(), loc["longitude"].to_string());
            let w = get_json(
                "https://api.open-meteo.com/v1/forecast",
                &[
                    ("latitude", &lat),
                    ("longitude", &lon),
                    ("current", "temperature_2m,wind_speed_10m,precipitation,weather_code"),
                    ("daily", "temperature_2m_max,temperature_2m_min,precipitation_sum"),
                    ("timezone", "auto"),
                    ("forecast_days", "3"),
                ],
            )?;
            Ok(json!({
                "place": format!("{}, {}", loc["name"].as_str().unwrap_or(""), loc["country"].as_str().unwrap_or("")),
                "current": w["current"],
                "daily": w["daily"],
            }))
        })())
    }
}
