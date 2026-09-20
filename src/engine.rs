//! `omatide run`: follows the boat through omakeel and serves the tide
//! and the stream over the Unix socket in docs/protocol.md.
//!
//! Unlike the other Omahoy engines this one never needs the network
//! while it runs. The station catalog is fetched once and is good for
//! years, so everything here is arithmetic: the same predictions at the
//! dock, at sea, and with the dish off.

use crate::bay;
use crate::cache;
use crate::config::{self, Settings};
use crate::keel::{self, Boat, Update};
use crate::predict::{Extreme, Turn};
use crate::station::{Catalog, Kind};
use crate::tides;
use crate::time;
use serde_json::{Map, Value, json};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io,
    os::unix::{
        fs::{FileTypeExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    sync::mpsc,
    task::JoinHandle,
    time::{Duration, MissedTickBehavior, interval, timeout},
};

pub const VERSION: u32 = 1;
/// Messages queued for one app. An app this far behind is dropped.
const QUEUE: usize = 64;
/// An app that can't take one message in this long has stalled.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
/// The longest line an app may send.
const MAX_LINE: usize = 64 * 1024;
/// The state is re-sent this often even when nothing else happens, so a
/// countdown to the next slack stays honest.
const TICK: Duration = Duration::from_secs(30);
/// How far ahead `state` looks for turns.
const AHEAD: i64 = 3 * 86_400;
/// The most points a curve may carry: a fortnight at ten minutes.
const MAX_POINTS: usize = 2016;
/// A station further off than this is not the tide where you are.
const TOO_FAR_NM: f64 = 60.0;
/// The most arrows one `streams` answer may carry. The whole bay is about
/// 150, so this only ever bites on a region far larger than a chart shows.
const MAX_STREAMS: usize = 400;

pub struct Config {
    pub socket: PathBuf,
    /// omakeel's socket, for the boat's position. None to never ask.
    pub keel: Option<PathBuf>,
    /// Where the station catalog lives.
    pub data: PathBuf,
    pub settings: PathBuf,
    /// Unix seconds now. Tests pin it.
    pub clock: fn() -> i64,
}

/// `$XDG_RUNTIME_DIR/omatide/tide.sock`.
pub fn default_socket() -> io::Result<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("omatide").join("tide.sock"))
        .ok_or_else(|| io::Error::other("XDG_RUNTIME_DIR must be set to an absolute path"))
}

enum Event {
    Tick,
    Keel(Update),
    Request { client: u64, message: Value },
}

struct Tide {
    config: Config,
    settings: Settings,
    problems: Vec<String>,
    settings_seen: Option<SystemTime>,
    catalog: Catalog,
    catalog_problem: Option<String>,
    boat: Option<Boat>,
    keel: &'static str,
}

impl Tide {
    fn now(&self) -> i64 {
        (self.config.clock)()
    }

    /// Where the tide is wanted: the boat if omakeel has a fix, else home.
    fn here(&self) -> (f64, f64, bool) {
        match self.boat {
            Some(b) => (b.lat, b.lon, true),
            None => (self.settings.home.0, self.settings.home.1, false),
        }
    }

    fn reload_settings(&mut self) {
        let seen = fs::metadata(&self.config.settings)
            .and_then(|m| m.modified())
            .ok();
        if seen == self.settings_seen && self.settings_seen.is_some() {
            return;
        }
        self.settings_seen = seen;
        let (settings, problems) = config::load(&self.config.settings);
        self.settings = settings;
        self.problems = problems;
    }

    /// The whole picture, as one `state` message.
    fn state(&self) -> Value {
        let now = self.now();
        let (lat, lon, from_boat) = self.here();
        let mut m = Map::new();
        m.insert("type".into(), "state".into());
        m.insert("v".into(), VERSION.into());
        m.insert("time".into(), time::iso(now).into());
        m.insert("keel".into(), self.keel.into());
        m.insert(
            "here".into(),
            json!({"lat": round(lat, 5), "lon": round(lon, 5),
                   "at": if from_boat { "boat" } else { "home" }}),
        );
        m.insert(
            "stations".into(),
            json!({"tide": self.count(Kind::Tide), "current": self.count(Kind::Current)}),
        );
        if let Some(tide) = self.nearest(lat, lon, Kind::Tide, now) {
            m.insert("tide".into(), tide);
        }
        if let Some(current) = self.nearest(lat, lon, Kind::Current, now) {
            m.insert("current".into(), current);
        }
        m.insert("bay".into(), self.bay(now));
        let mut problems = self.problems.clone();
        if let Some(p) = &self.catalog_problem {
            problems.push(p.clone());
        }
        if !problems.is_empty() {
            m.insert("problems".into(), problems.into());
        }
        Value::Object(m)
    }

    fn count(&self, kind: Kind) -> usize {
        self.catalog
            .stations
            .iter()
            .filter(|s| s.kind == kind && self.catalog.is_ready(&s.key()))
            .count()
    }

    /// The nearest station of a kind, with what it says now and next.
    fn nearest(&self, lat: f64, lon: f64, kind: Kind, now: i64) -> Option<Value> {
        let (station, nm) = self.catalog.nearest(lat, lon, kind)?;
        if nm > TOO_FAR_NM {
            return None;
        }
        let key = station.key();
        let mut m = Map::new();
        m.insert("station".into(), key.clone().into());
        m.insert("name".into(), station.name.clone().into());
        m.insert("lat".into(), round(station.lat, 5).into());
        m.insert("lon".into(), round(station.lon, 5).into());
        m.insert("distanceNm".into(), round(nm, 2).into());
        if let Some(depth) = station.depth {
            m.insert("depthM".into(), round(depth, 1).into());
        }
        m.insert(
            "follows".into(),
            match station.reference() {
                Some(r) => r.into(),
                None => Value::Null,
            },
        );
        if m.get("follows") == Some(&Value::Null) {
            m.remove("follows");
        }
        match kind {
            Kind::Tide => {
                m.insert(
                    "heightM".into(),
                    round(tides::height(&self.catalog, &key, now).ok()?, 3).into(),
                );
                let turns = tides::extremes(&self.catalog, &key, now, now + AHEAD).ok()?;
                m.insert("turns".into(), turns_json(&turns, Kind::Tide));
                // Rising or falling, which is what a sailor asks first.
                m.insert(
                    "rising".into(),
                    matches!(turns.first().map(|t| t.turn), Some(Turn::High)).into(),
                );
            }
            Kind::Current => {
                let (speed, set) = tides::stream(&self.catalog, &key, now).ok()?;
                let knots = speed * tides::KNOTS;
                m.insert("knots".into(), round(knots.abs(), 2).into());
                m.insert("way".into(), way(knots).into());
                if let Some(set) = set {
                    m.insert("setDeg".into(), round(set, 0).into());
                }
                let turns = tides::current_turns(&self.catalog, &key, now, now + AHEAD).ok()?;
                m.insert("turns".into(), turns_json(&turns, Kind::Current));
            }
        }
        Some(Value::Object(m))
    }

    /// The stream at every named place in the bay at one moment: the
    /// picture that shows the Gate still flooding while Carquinez ebbs.
    fn bay(&self, at: i64) -> Value {
        let mut out = Vec::new();
        for (place, key) in bay::resolve(&self.catalog, bay::PLACES, self.settings.depth) {
            let Ok((speed, set)) = tides::stream(&self.catalog, &key, at) else {
                continue;
            };
            let Some(station) = self.catalog.get(&key) else {
                continue;
            };
            let knots = speed * tides::KNOTS;
            let mut m = json!({
                "place": place.name,
                "note": place.note,
                "station": key,
                "lat": round(station.lat, 5),
                "lon": round(station.lon, 5),
                "knots": round(knots.abs(), 2),
                "way": way(knots),
            });
            if let Some(set) = set {
                m["setDeg"] = round(set, 0).into();
            }
            if let Ok(turns) = tides::current_turns(&self.catalog, &key, at, at + 86_400)
                && let Some(next) = turns.first()
            {
                m["next"] = json!({
                    "turn": next.turn.name(),
                    "time": time::iso(next.time),
                    "knots": round((next.value * tides::KNOTS).abs(), 2),
                });
            }
            out.push(m);
        }
        Value::Array(out)
    }

    /// The tide marching up the bay: high water at each named place.
    fn bay_tide(&self, at: i64) -> Value {
        let mut out = Vec::new();
        for (place, key) in bay::resolve(&self.catalog, bay::TIDE_PLACES, self.settings.depth) {
            let (Ok(height), Some(station)) = (
                tides::height(&self.catalog, &key, at),
                self.catalog.get(&key),
            ) else {
                continue;
            };
            let mut m = json!({
                "place": place.name,
                "note": place.note,
                "station": key,
                "lat": round(station.lat, 5),
                "lon": round(station.lon, 5),
                "heightM": round(height, 3),
            });
            if let Ok(turns) = tides::extremes(&self.catalog, &key, at, at + 86_400)
                && let Some(next) = turns.first()
            {
                m["next"] = json!({
                    "turn": next.turn.name(),
                    "time": time::iso(next.time),
                    "heightM": round(next.value, 3),
                });
            }
            out.push(m);
        }
        Value::Array(out)
    }

    /// Answers one request from an app.
    fn answer(&self, message: &Value) -> Value {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let kind = message.get("type").and_then(Value::as_str).unwrap_or("");
        let with_id = |mut v: Value| {
            if id != Value::Null {
                v["id"] = id.clone();
            }
            v
        };
        match kind {
            "curve" => with_id(self.curve(message)),
            "bay" => {
                let at = match asked_time(message, self.now()) {
                    Ok(at) => at,
                    Err(e) => return with_id(error(&e)),
                };
                with_id(json!({
                    "type": "bay", "v": VERSION, "time": time::iso(at),
                    "current": self.bay(at), "tide": self.bay_tide(at),
                }))
            }
            "streams" => with_id(self.streams(message)),
            "stations" => with_id(self.stations(message)),
            "" => with_id(error("a request needs a type")),
            other => with_id(error(&format!("omatide doesn't know the request {other}"))),
        }
    }

    /// A curve to draw: heights or speeds at a fixed step.
    fn curve(&self, message: &Value) -> Value {
        let now = self.now();
        let start = match asked_time(message, now) {
            Ok(t) => t,
            Err(e) => return error(&e),
        };
        let hours = message
            .get("hours")
            .and_then(Value::as_i64)
            .unwrap_or(24)
            .clamp(1, 336);
        let step = message
            .get("stepSeconds")
            .and_then(Value::as_i64)
            .unwrap_or(600)
            .clamp(60, 3600);
        let key = match self.asked_station(message) {
            Ok(key) => key,
            Err(e) => return error(&e),
        };
        let Some(station) = self.catalog.get(&key) else {
            return error(&format!("no station {key}"));
        };
        let end = start + hours * 3600;
        // The step is stretched rather than the window cut, so a client
        // asking for a fortnight gets a fortnight, just coarser.
        let step = step.max((end - start) / MAX_POINTS as i64 + 1);
        let (values, turns) = match station.kind {
            Kind::Tide => (
                tides::heights(&self.catalog, &key, start, end, step),
                tides::extremes(&self.catalog, &key, start, end),
            ),
            Kind::Current => (
                tides::streams(&self.catalog, &key, start, end, step),
                tides::current_turns(&self.catalog, &key, start, end),
            ),
        };
        let (Ok(values), Ok(turns)) = (values, turns) else {
            return error(&format!("{key} can't be predicted"));
        };
        let scale = if station.kind == Kind::Tide {
            1.0
        } else {
            tides::KNOTS
        };
        json!({
            "type": "curve",
            "v": VERSION,
            "station": key,
            "name": station.name,
            "kind": station.kind.name(),
            "unit": if station.kind == Kind::Tide { "m" } else { "kn" },
            "start": time::iso(start),
            "stepSeconds": step,
            // Bare numbers: a fortnight of them is a tenth the size of
            // the same points as objects, and the times are implied.
            "values": values
                .iter()
                .map(|p| round(p.value * scale, 3))
                .collect::<Vec<f64>>(),
            "turns": turns_json(&turns, station.kind),
        })
    }

    /// The stream at every station over an area, at one moment: what a
    /// chart layer draws.
    ///
    /// One bin per station, the one nearest the depth asked for, because
    /// a chart wants one arrow in each channel and not three stacked on
    /// top of each other.
    fn streams(&self, message: &Value) -> Value {
        let at = match asked_time(message, self.now()) {
            Ok(at) => at,
            Err(e) => return error(&e),
        };
        let depth = message
            .get("depthM")
            .and_then(Value::as_f64)
            .filter(|d| (0.0..=200.0).contains(d))
            .unwrap_or(self.settings.depth);
        let area = match Area::read(message) {
            Ok(area) => area,
            Err(e) => return error(&e),
        };
        let mut found: Vec<(f64, Value)> = Vec::new();
        let mut more = false;
        for key in bay::all(&self.catalog, Kind::Current, depth) {
            let Some(station) = self.catalog.get(&key) else {
                continue;
            };
            if !area.holds(station.lat, station.lon) {
                continue;
            }
            let away = area.distance_from(station.lat, station.lon);
            if area.beyond(away) {
                continue;
            }
            let Ok((speed, set)) = tides::stream(&self.catalog, &key, at) else {
                continue;
            };
            let knots = speed * tides::KNOTS;
            let mut m = json!({
                "station": key,
                "name": station.name,
                "lat": round(station.lat, 5),
                "lon": round(station.lon, 5),
                "knots": round(knots.abs(), 2),
                "way": way(knots),
            });
            if let Some(set) = set {
                m["setDeg"] = round(set, 0).into();
            }
            if let Some(d) = station.depth {
                m["depthM"] = round(d, 1).into();
            }
            if let Some(nm) = away {
                m["distanceNm"] = round(nm, 2).into();
            }
            found.push((away.unwrap_or(0.0), m));
        }
        // Nearest first where a position was given, so a client that only
        // wants the closest can stop reading.
        if area.from.is_some() {
            found.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        if found.len() > MAX_STREAMS {
            found.truncate(MAX_STREAMS);
            more = true;
        }
        let mut answer = json!({
            "type": "streams",
            "v": VERSION,
            "time": time::iso(at),
            "depthM": round(depth, 1),
            "streams": found.into_iter().map(|(_, m)| m).collect::<Vec<Value>>(),
        });
        if more {
            answer["more"] = true.into();
        }
        answer
    }

    fn stations(&self, message: &Value) -> Value {
        let only = message
            .get("kind")
            .and_then(Value::as_str)
            .and_then(Kind::parse);
        let near = match (message.get("lat"), message.get("lon")) {
            (Some(lat), Some(lon)) => match (lat.as_f64(), lon.as_f64()) {
                (Some(lat), Some(lon)) => Some((lat, lon)),
                _ => return error("lat and lon must be numbers"),
            },
            _ => None,
        };
        let within = message
            .get("withinNm")
            .and_then(Value::as_f64)
            .unwrap_or(f64::MAX);
        let mut out = Vec::new();
        for s in &self.catalog.stations {
            if only.is_some_and(|k| k != s.kind) || !self.catalog.is_ready(&s.key()) {
                continue;
            }
            let away = near.map(|(lat, lon)| s.distance_from(lat, lon));
            if away.is_some_and(|d| d > within) {
                continue;
            }
            let mut m = json!({
                "station": s.key(), "name": s.name, "kind": s.kind.name(),
                "lat": round(s.lat, 5), "lon": round(s.lon, 5),
            });
            if let Some(d) = away {
                m["distanceNm"] = round(d, 2).into();
            }
            if let Some(d) = s.depth {
                m["depthM"] = round(d, 1).into();
            }
            if let Some(d) = s.flood_direction {
                m["floodDeg"] = round(d, 0).into();
            }
            out.push(m);
        }
        json!({"type": "stations", "v": VERSION, "stations": out})
    }

    /// The station a request names, or the nearest one to where we are.
    fn asked_station(&self, message: &Value) -> Result<String, String> {
        if let Some(key) = message.get("station").and_then(Value::as_str) {
            return Ok(key.to_string());
        }
        let kind = match message.get("kind").and_then(Value::as_str) {
            None | Some("tide") => Kind::Tide,
            Some("current") => Kind::Current,
            Some(other) => return Err(format!("kind must be tide or current, not {other}")),
        };
        let (lat, lon) = match (
            message.get("lat").and_then(Value::as_f64),
            message.get("lon").and_then(Value::as_f64),
        ) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => {
                let (lat, lon, _) = self.here();
                (lat, lon)
            }
        };
        self.catalog
            .nearest(lat, lon, kind)
            .map(|(s, _)| s.key())
            .ok_or_else(|| format!("no {} station in the catalog", kind.name()))
    }
}

/// Where a request wants stations from: a box, a position with a radius,
/// or both. Everything is optional; nothing given means everywhere.
struct Area {
    box_: Option<(f64, f64, f64, f64)>,
    from: Option<(f64, f64)>,
    within_nm: Option<f64>,
}

impl Area {
    fn read(m: &Value) -> Result<Area, String> {
        let number = |name: &str| -> Result<Option<f64>, String> {
            match m.get(name) {
                None | Some(Value::Null) => Ok(None),
                Some(v) => v
                    .as_f64()
                    .filter(|n| n.is_finite())
                    .map(Some)
                    .ok_or_else(|| format!("{name} must be a number")),
            }
        };
        let (south, west, north, east) = (
            number("south")?,
            number("west")?,
            number("north")?,
            number("east")?,
        );
        let box_ = match (south, west, north, east) {
            (Some(s), Some(w), Some(n), Some(e)) if s <= n && w <= e => Some((s, w, n, e)),
            (None, None, None, None) => None,
            _ => {
                return Err(
                    "south, west, north and east must all be given, south below north".into(),
                );
            }
        };
        let from = match (number("lat")?, number("lon")?) {
            (Some(lat), Some(lon))
                if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) =>
            {
                Some((lat, lon))
            }
            (None, None) => None,
            _ => return Err("lat and lon must be a position on the earth".into()),
        };
        let within_nm = number("withinNm")?.filter(|n| *n >= 0.0);
        Ok(Area {
            box_,
            from,
            within_nm,
        })
    }

    fn holds(&self, lat: f64, lon: f64) -> bool {
        match self.box_ {
            None => true,
            Some((s, w, n, e)) => (s..=n).contains(&lat) && (w..=e).contains(&lon),
        }
    }

    fn distance_from(&self, lat: f64, lon: f64) -> Option<f64> {
        self.from
            .map(|(from_lat, from_lon)| crate::station::haversine_nm(from_lat, from_lon, lat, lon))
    }

    /// Outside the radius, where one was asked for.
    fn beyond(&self, away: Option<f64>) -> bool {
        match (away, self.within_nm) {
            (Some(nm), Some(limit)) => nm > limit,
            _ => false,
        }
    }
}

fn way(knots: f64) -> &'static str {
    if knots.abs() < 0.05 {
        "slack"
    } else if knots > 0.0 {
        "flood"
    } else {
        "ebb"
    }
}

fn turns_json(turns: &[Extreme], kind: Kind) -> Value {
    Value::Array(
        turns
            .iter()
            .map(|e| match kind {
                Kind::Tide => json!({
                    "turn": e.turn.name(),
                    "time": time::iso(e.time),
                    "heightM": round(e.value, 3),
                }),
                Kind::Current => json!({
                    "turn": e.turn.name(),
                    "time": time::iso(e.time),
                    "knots": round((e.value * tides::KNOTS).abs(), 2),
                }),
            })
            .collect(),
    )
}

fn error(message: &str) -> Value {
    json!({"type": "error", "v": VERSION, "message": message})
}

fn asked_time(m: &Value, now: i64) -> Result<i64, String> {
    match m.get("time") {
        None | Some(Value::Null) => Ok(now),
        Some(Value::String(s)) => {
            time::parse_iso(s).ok_or_else(|| format!("time: {s} isn't a UTC time"))
        }
        Some(_) => Err("time must be a UTC time like 2026-09-20T17:00:00Z".into()),
    }
}

fn round(v: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    (v * scale).round() / scale
}

fn encode(v: &Value) -> Arc<str> {
    let mut text = v.to_string();
    text.push('\n');
    Arc::from(text)
}

pub async fn run(config: Config) -> io::Result<()> {
    let (listener, _socket) = bind(&config.socket)?;
    let (catalog, catalog_problem) = match cache::load(&config.data) {
        Ok(c) => (c, None),
        Err(e) => (
            Catalog::default(),
            Some(format!("{e}; run `omatide fetch`")),
        ),
    };
    let mut tide = Tide {
        settings: Settings::default(),
        problems: Vec::new(),
        settings_seen: None,
        catalog,
        catalog_problem,
        boat: None,
        keel: if config.keel.is_some() {
            "connecting"
        } else {
            "off"
        },
        config,
    };
    tide.reload_settings();
    if let Some(p) = &tide.catalog_problem {
        eprintln!("omatide: {p}");
    }
    for p in &tide.problems {
        eprintln!("omatide: {p}");
    }

    let (events, mut rx) = mpsc::channel::<Event>(256);
    if let Some(path) = tide.config.keel.clone() {
        let (keel_tx, mut keel_rx) = mpsc::channel::<Update>(64);
        tokio::spawn(keel::follow(path, keel_tx));
        let events = events.clone();
        tokio::spawn(async move {
            while let Some(update) = keel_rx.recv().await {
                if events.send(Event::Keel(update)).await.is_err() {
                    return;
                }
            }
        });
    }
    {
        let events = events.clone();
        tokio::spawn(async move {
            let mut ticker = interval(TICK);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                if events.send(Event::Tick).await.is_err() {
                    return;
                }
            }
        });
    }

    let mut clients: Vec<Client> = Vec::new();
    let mut next_id = 0u64;
    let mut state = encode(&tide.state());
    let hello = encode(&json!({"type": "hello", "v": VERSION,
                               "tide": env!("CARGO_PKG_VERSION")}));
    let mut stop = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { continue };
                next_id += 1;
                let client = Client::spawn(stream, next_id, events.clone());
                let _ = client.tx.try_send(hello.clone());
                let _ = client.tx.try_send(state.clone());
                clients.push(client);
            }
            event = rx.recv() => {
                let Some(event) = event else { break };
                let mut changed = false;
                match event {
                    Event::Tick => {
                        tide.reload_settings();
                        changed = true;
                    }
                    Event::Keel(update) => {
                        match update {
                            Update::Boat(boat) => {
                                tide.keel = "connected";
                                if tide.boat != boat {
                                    tide.boat = boat;
                                    changed = true;
                                }
                            }
                            Update::Lost => {
                                if tide.keel != "lost" {
                                    tide.keel = "lost";
                                    changed = true;
                                }
                            }
                            Update::Incompatible(_) => {
                                if tide.keel != "incompatible" {
                                    tide.keel = "incompatible";
                                    changed = true;
                                }
                            }
                        }
                    }
                    Event::Request { client, message } => {
                        let reply = encode(&tide.answer(&message));
                        if let Some(c) = clients.iter().find(|c| c.id == client) {
                            let _ = c.tx.try_send(reply);
                        }
                    }
                }
                if changed {
                    state = encode(&tide.state());
                    clients.retain(|c| c.tx.try_send(state.clone()).is_ok());
                }
            }
            _ = tokio::signal::ctrl_c() => break,
            _ = stop.recv() => break,
        }
        clients.retain(|c| !c.tx.is_closed());
    }
    Ok(())
}

/// One connected app: a task writes its queue and reads its requests.
/// Dropping it closes the app's socket.
struct Client {
    id: u64,
    tx: mpsc::Sender<Arc<str>>,
    task: JoinHandle<()>,
}

impl Client {
    fn spawn(stream: UnixStream, id: u64, requests: mpsc::Sender<Event>) -> Client {
        let (tx, mut rx) = mpsc::channel::<Arc<str>>(QUEUE);
        let task = tokio::spawn(async move {
            let (mut reader, mut writer) = stream.into_split();
            let mut scratch = [0u8; 4096];
            let mut line: Vec<u8> = Vec::new();
            // Past MAX_LINE: the rest of the line is thrown away.
            let mut skipping = false;
            loop {
                tokio::select! {
                    out = rx.recv() => {
                        let Some(out) = out else { break };
                        let written = timeout(WRITE_TIMEOUT, writer.write_all(out.as_bytes())).await;
                        if !matches!(written, Ok(Ok(()))) {
                            break;
                        }
                    }
                    read = reader.read(&mut scratch) => {
                        let n = match read {
                            Ok(n) if n > 0 => n,
                            _ => break,
                        };
                        let mut errors = Vec::new();
                        for &b in &scratch[..n] {
                            if b != b'\n' {
                                if !skipping {
                                    line.push(b);
                                    if line.len() > MAX_LINE {
                                        line.clear();
                                        skipping = true;
                                        errors.push("line too long");
                                    }
                                }
                                continue;
                            }
                            if std::mem::take(&mut skipping) {
                                continue;
                            }
                            let text = std::mem::take(&mut line);
                            if text.iter().all(u8::is_ascii_whitespace) {
                                continue;
                            }
                            match serde_json::from_slice::<Value>(&text) {
                                Ok(message @ Value::Object(_)) => {
                                    if requests
                                        .send(Event::Request { client: id, message })
                                        .await
                                        .is_err()
                                    {
                                        return;
                                    }
                                }
                                _ => errors.push("not a JSON object"),
                            }
                        }
                        for e in errors {
                            let reply = error(e).to_string() + "\n";
                            let written =
                                timeout(WRITE_TIMEOUT, writer.write_all(reply.as_bytes())).await;
                            if !matches!(written, Ok(Ok(()))) {
                                return;
                            }
                        }
                    }
                }
            }
        });
        Client { id, tx, task }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Takes the lock beside the socket, so one socket has one engine, then
/// binds, replacing a socket a crashed engine left behind. As omakeel does.
fn bind(path: &Path) -> io::Result<(UnixListener, SocketFile)> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
        && !dir.exists()
    {
        fs::create_dir_all(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    let lock = lock(path)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if !metadata.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} exists and isn't a socket", path.display()),
            ));
        }
        fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    Ok((
        listener,
        SocketFile {
            path: path.to_path_buf(),
            _lock: lock,
        },
    ))
}

fn lock_path(socket: &Path) -> PathBuf {
    let mut name = OsString::from(socket.as_os_str());
    name.push(".lock");
    PathBuf::from(name)
}

fn lock(socket: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(lock_path(socket))?;
    // SAFETY: `flock` on a descriptor `file` owns; the lock lasts until the
    // file is closed.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let e = io::Error::last_os_error();
        if e.kind() == io::ErrorKind::WouldBlock {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("omatide is already running on {}", socket.display()),
            ));
        }
        return Err(e);
    }
    Ok(file)
}

struct SocketFile {
    path: PathBuf,
    _lock: File,
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
