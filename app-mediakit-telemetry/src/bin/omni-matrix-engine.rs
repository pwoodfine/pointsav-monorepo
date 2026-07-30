use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::Write;
use chrono::{DateTime, Utc, Datelike, TimeZone, Duration};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct MaxMindRecord {
    country: Option<Country>,
    subdivisions: Option<Vec<Subdivision>>,
    city: Option<City>,
    location: Option<Location>,
}
#[derive(Deserialize, Debug)] struct Country { names: Option<HashMap<String, String>> }
#[derive(Deserialize, Debug)] struct Subdivision { names: Option<HashMap<String, String>> }
#[derive(Deserialize, Debug)] struct City { names: Option<HashMap<String, String>> }
#[derive(Deserialize, Debug)] struct Location { time_zone: Option<String> }

struct TimeStats {
    events: u64,
    ips: HashSet<String>,
}

impl TimeStats {
    fn new() -> Self {
        Self { events: 0, ips: HashSet::new() }
    }
}

fn categorize_uri(uri: &str) -> &'static str {
    let u = uri.to_lowercase();
    if u.contains("file://") || u.contains("localhost") || u.contains("127.0.0.1") || u.contains("192.168.") || u.contains("10.0.0.") {
        "Local Staging (Offline / Localhost)"
    } else if u.contains("github.io") {
        "Edge Delivery (GitHub Pages)"
    } else if u.contains("woodfinegroup") {
        "Primary Domain (Woodfine)"
    } else if u.contains("pointsav") {
        "Primary Domain (PointSav)"
    } else {
        "Unmapped Routing Target"
    }
}

fn categorize_os(ua: &str) -> &'static str {
    let u = ua.to_lowercase();
    if u.contains("mac os x") { "macOS" }
    else if u.contains("windows") { "Windows" }
    else if u.contains("linux") { "Linux" }
    else if u.contains("iphone") || u.contains("ipad") { "iOS" }
    else if u.contains("android") { "Android" }
    else { "Unknown OS" }
}

fn categorize_device(ua: &str) -> &'static str {
    let u = ua.to_lowercase();
    if u.contains("mobi") || u.contains("android") || u.contains("iphone") { "Mobile / Tablet" }
    else { "Desktop / Server" }
}

fn format_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.chars().count());
            }
        }
    }
    
    let mut out = String::new();
    
    // Header row
    out.push('|');
    for (i, h) in headers.iter().enumerate() {
        let padding = widths[i].saturating_sub(h.chars().count());
        out.push_str(&format!(" {}{} |", h, " ".repeat(padding)));
    }
    out.push('\n');
    
    // Separator row
    out.push('|');
    for w in &widths {
        let dashes = if *w > 0 { *w - 1 } else { 0 };
        out.push_str(&format!(" :{} |", "-".repeat(dashes)));
    }
    out.push('\n');
    
    // Data rows
    if rows.is_empty() {
        out.push_str("| Awaiting Data");
        let padding = widths[0].saturating_sub(13);
        out.push_str(&" ".repeat(padding));
        out.push_str(" |");
        for w in widths.iter().skip(1) {
            let padding = w.saturating_sub(1);
            out.push_str(&format!(" 0{} |", " ".repeat(padding)));
        }
        out.push('\n');
    } else {
        for row in rows {
            out.push('|');
            for (i, cell) in row.iter().enumerate() {
                let w = *widths.get(i).unwrap_or(&0);
                let padding = w.saturating_sub(cell.chars().count());
                out.push_str(&format!(" {}{} |", cell, " ".repeat(padding)));
            }
            out.push('\n');
        }
    }
    out
}

fn main() {
    let fleet_id = env::var("FLEET_ID").unwrap_or_else(|_| "UNKNOWN_FLEET".to_string());
    let ledger_path = "assets/ledger_telemetry.csv";
    let db_path = "assets/GeoLite2-City.mmdb";
    
    let now = Utc::now();
    let today_str = now.format("%Y-%m-%d").to_string();
    let outbox_path = format!("outbox/REPORT_{}_{}.md", fleet_id, today_str);

    let mut time_stats: HashMap<&str, TimeStats> = HashMap::new();
    let intervals = vec!["Yesterday", "7 Days", "30 Days", "60 Days", "90 Days", "YTD", "Inception"];
    for k in &intervals {
        time_stats.insert(k, TimeStats::new());
    }

    let mut country_state_counts: HashMap<String, u64> = HashMap::new();
    let mut metro_counts: HashMap<String, u64> = HashMap::new();
    let mut tz_counts: HashMap<String, u64> = HashMap::new();
    let mut uri_counts: HashMap<String, u64> = HashMap::new();
    let mut os_counts: HashMap<String, u64> = HashMap::new();
    let mut device_counts: HashMap<String, u64> = HashMap::new();
    let mut ua_counts: HashMap<String, u64> = HashMap::new();

    let mut total_events = 0;
    let mut total_geo_events = 0;

    let reader_db = maxminddb::Reader::open_readfile(db_path).ok();

    if let Ok(mut rdr) = csv::ReaderBuilder::new().has_headers(false).from_path(ledger_path) {
        for result in rdr.records() {
            let record = match result {
                Ok(r) => r,
                Err(_) => continue,
            };

            if record.len() < 2 { continue; }
            
            let ip = record[0].trim().to_string();
            let ts_str = record[1].trim();
            let uri_raw = if record.len() > 2 { record[2].trim() } else { "" };
            let ua_raw = if record.len() > 3 { record[3].trim() } else { "" };

            if ip == "0.0.0.0" || ip.to_lowercase() == "ip" { continue; }

            let ts_fixed = ts_str.replace("Z", "+00:00");
            let ts = match DateTime::parse_from_rfc3339(&ts_fixed) {
                Ok(d) => d.with_timezone(&Utc),
                Err(_) => continue,
            };

            total_events += 1;
            time_stats.get_mut("Inception").unwrap().events += 1;
            time_stats.get_mut("Inception").unwrap().ips.insert(ip.clone());

            *uri_counts.entry(categorize_uri(uri_raw).to_string()).or_insert(0) += 1;
            *os_counts.entry(categorize_os(ua_raw).to_string()).or_insert(0) += 1;
            *device_counts.entry(categorize_device(ua_raw).to_string()).or_insert(0) += 1;

            let clean_ua = if ua_raw.chars().count() > 65 {
                format!("{}...", ua_raw.chars().take(65).collect::<String>())
            } else if ua_raw.is_empty() {
                "Unknown Architecture".to_string()
            } else {
                ua_raw.to_string()
            };
            *ua_counts.entry(clean_ua).or_insert(0) += 1;

            let thresholds: HashMap<&str, DateTime<Utc>> = [
                ("Yesterday", now - Duration::days(1)),
                ("7 Days", now - Duration::days(7)),
                ("30 Days", now - Duration::days(30)),
                ("60 Days", now - Duration::days(60)),
                ("90 Days", now - Duration::days(90)),
                ("YTD", Utc.with_ymd_and_hms(now.year(), 1, 1, 0, 0, 0).unwrap()),
            ].iter().cloned().collect();

            for (key, threshold) in &thresholds {
                if ts >= *threshold {
                    if let Some(stats) = time_stats.get_mut(key) {
                        stats.events += 1;
                        stats.ips.insert(ip.clone());
                    }
                }
            }

            if let Some(ref db) = reader_db {
                let ip_addr: std::net::IpAddr = match ip.parse() {
                    Ok(addr) => addr,
                    Err(_) => continue,
                };
                
                match db.lookup::<MaxMindRecord>(ip_addr) {
                    Ok(geo) => {
                        let country = geo.country.and_then(|c| c.names).and_then(|n| n.get("en").cloned()).unwrap_or_else(|| "Unknown Country".to_string());
                        let state = geo.subdivisions.and_then(|mut s| if !s.is_empty() { s.remove(0).names } else { None }).and_then(|n| n.get("en").cloned()).unwrap_or_else(|| "Unknown Region".to_string());
                        let city = geo.city.and_then(|c| c.names).and_then(|n| n.get("en").cloned()).unwrap_or_else(|| "Unknown City".to_string());
                        let tz = geo.location.and_then(|l| l.time_zone).unwrap_or_else(|| "Unknown Timezone".to_string());

                        let country_str = if state != "Unknown Region" { format!("{} ({})", country, state) } else { country };
                        *country_state_counts.entry(country_str).or_insert(0) += 1;
                        *metro_counts.entry(city).or_insert(0) += 1;
                        *tz_counts.entry(tz).or_insert(0) += 1;
                    },
                    Err(_) => {
                        *country_state_counts.entry("Unknown Location".to_string()).or_insert(0) += 1;
                        *metro_counts.entry("Unknown Region".to_string()).or_insert(0) += 1;
                        *tz_counts.entry("Unknown Timezone".to_string()).or_insert(0) += 1;
                    }
                }
                total_geo_events += 1;
            }
        }
    }

    let sort_map = |map: HashMap<String, u64>, total: u64| -> Vec<Vec<String>> {
        let mut vec: Vec<_> = map.into_iter().collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        vec.into_iter().map(|(k, v)| {
            let density = if total > 0 { (v as f64 / total as f64) * 100.0 } else { 0.0 };
            vec![k, v.to_string(), format!("{:.1}%", density)]
        }).collect()
    };

    let mut md = String::new();
    md.push_str(&format!("# 📊 FLEET TELEMETRY LEDGER | {}\n", fleet_id));
    md.push_str("**Engine Version:** 1.2.0 (Compiled Rust Core)\n");
    md.push_str(&format!("**Generated:** {}\n", now.format("%Y-%m-%dT%H:%M:%SZ")));
    md.push_str("**Standard:** Sovereign Data Protocol (DS-ADR-06)\n\n---\n\n");

    md.push_str("## ⏱️ TIME MATRIX\n");
    let mut row_events = vec!["Total Network Events".to_string()];
    let mut row_ips = vec!["Unique Terminals".to_string()];
    for k in &intervals {
        let stats = time_stats.get(k).unwrap();
        row_events.push(stats.events.to_string());
        row_ips.push(stats.ips.len().to_string());
    }
    let mut t_headers = vec!["Metric"];
    t_headers.extend(intervals.clone());
    md.push_str(&format_table(&t_headers, &[row_events, row_ips]));
    md.push('\n');

    md.push_str("## 🌍 GLOBAL ROUTING MATRIX\n");
    let mut g_rows = Vec::new();
    let mut cs_vec: Vec<_> = country_state_counts.into_iter().collect();
    cs_vec.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, v) in cs_vec {
        let parts: Vec<&str> = k.split(" (").collect();
        let country = parts[0].to_string();
        let state = if parts.len() > 1 { parts[1].replace(")", "") } else { "Unknown".to_string() };
        let density = if total_geo_events > 0 { (v as f64 / total_geo_events as f64) * 100.0 } else { 0.0 };
        g_rows.push(vec![country, state, v.to_string(), format!("{:.1}%", density)]);
    }
    md.push_str(&format_table(&["Country", "Region / State", "Volume", "Density %"], &g_rows));
    md.push('\n');

    md.push_str("## 🏙️ METRO REGION MATRIX\n");
    md.push_str(&format_table(&["Metro Area", "Volume", "Density %"], &sort_map(metro_counts, total_geo_events)));
    md.push('\n');

    md.push_str("## 🕒 TIMEZONE ALIGNMENT MATRIX\n");
    md.push_str(&format_table(&["Timezone", "Volume", "Density %"], &sort_map(tz_counts, total_geo_events)));
    md.push('\n');

    md.push_str("## 📡 CONTENT MATRIX (TARGET URI)\n");
    md.push_str(&format_table(&["Routing Target", "Volume", "Density %"], &sort_map(uri_counts, total_events)));
    md.push('\n');

    md.push_str("## 📱 DEVICE FORM FACTOR MATRIX\n");
    md.push_str(&format_table(&["Hardware Type", "Volume", "Density %"], &sort_map(device_counts, total_events)));
    md.push('\n');

    md.push_str("## 💻 OPERATING SYSTEM MATRIX\n");
    md.push_str(&format_table(&["Platform", "Volume", "Density %"], &sort_map(os_counts, total_events)));
    md.push('\n');

    md.push_str("## 🖥️ RAW ARCHITECTURE SIGNATURES (TOP 5)\n");
    let mut ua_vec: Vec<_> = ua_counts.into_iter().collect();
    ua_vec.sort_by(|a, b| b.1.cmp(&a.1));
    ua_vec.truncate(5);
    let top_ua_map: HashMap<_, _> = ua_vec.into_iter().collect();
    md.push_str(&format_table(&["Terminal Signature", "Volume", "Density %"], &sort_map(top_ua_map, total_events)));
    md.push('\n');

    if let Some(parent) = std::path::Path::new(&outbox_path).parent() {
        fs::create_dir_all(parent).ok();
    }
    
    if let Ok(mut file) = File::create(&outbox_path) {
        file.write_all(md.as_bytes()).ok();
        println!("[SUCCESS] {} Matrix successfully compiled and forged.", fleet_id);
    }
}
