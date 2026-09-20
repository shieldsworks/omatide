//! The engine over its socket: what an app actually sees.

use omatide::predict::Harmonics;
use omatide::station::{Catalog, Kind, Source, Station};
use omatide::{cache, engine};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A fixed moment, so the answers don't move under the test.
const NOON: i64 = 1_789_905_600; // 2026-09-20T12:00:00Z
fn clock() -> i64 {
    NOON
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omatide-engine-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A catalog with one of everything: a harmonic tide station, a
/// subordinate that follows it, and a harmonic current station.
fn catalog(dir: &Path) {
    let mut c = Catalog::default();
    let mut station = |id: &str, kind, lat, lon, source| {
        c.stations.push(Station {
            id: id.into(),
            bin: None,
            name: format!("{id} station"),
            lat,
            lon,
            kind,
            depth: None,
            flood_direction: Some(52.0),
            ebb_direction: Some(238.0),
            source,
        });
    };
    station("GATE", Kind::Tide, 37.8063, -122.4659, Source::Harmonic);
    station("STREAM", Kind::Current, 37.8292, -122.462, Source::Harmonic);
    let mut h = Harmonics::default();
    let m2 = omatide::constituent::index_of("M2").unwrap();
    let k1 = omatide::constituent::index_of("K1").unwrap();
    h.amplitude[m2] = 0.576;
    h.phase[m2] = 208.2;
    h.amplitude[k1] = 0.370;
    h.phase[k1] = 225.4;
    h.offset = 0.951;
    c.insert_harmonics("GATE".into(), h.clone());
    h.amplitude[m2] = 80.0;
    h.offset = 0.0;
    c.insert_harmonics("STREAM".into(), h);
    cache::save(dir, &c).unwrap();
}

struct Engine {
    socket: PathBuf,
    _thread: std::thread::JoinHandle<()>,
}

fn start(name: &str) -> Engine {
    let dir = scratch(name);
    catalog(&dir);
    let socket = dir.join("tide.sock");
    let config = engine::Config {
        socket: socket.clone(),
        keel: None,
        data: dir.clone(),
        settings: dir.join("config.toml"),
        clock,
    };
    let thread = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(engine::run(config))
            .unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !socket.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(socket.exists(), "the engine never bound its socket");
    Engine {
        socket,
        _thread: thread,
    }
}

struct App {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl App {
    fn connect(engine: &Engine) -> App {
        let stream = UnixStream::connect(&engine.socket).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        App {
            writer: stream.try_clone().unwrap(),
            reader: BufReader::new(stream),
        }
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"))
    }

    fn ask(&mut self, request: Value) -> Value {
        self.writer
            .write_all((request.to_string() + "\n").as_bytes())
            .unwrap();
        // `state` can arrive between the request and its answer.
        loop {
            let m = self.read();
            if m["type"] != "state" {
                return m;
            }
        }
    }
}

#[test]
fn an_app_is_greeted_and_told_the_whole_state() {
    let engine = start("state");
    let mut app = App::connect(&engine);
    let hello = app.read();
    assert_eq!(hello["type"], "hello");
    assert_eq!(hello["v"], 1);

    let state = app.read();
    assert_eq!(state["type"], "state");
    assert_eq!(state["keel"], "off");
    assert_eq!(state["time"], "2026-09-20T12:00:00Z");
    // No omakeel, so the tide is reported at home.
    assert_eq!(state["here"]["at"], "home");
    assert_eq!(state["stations"]["tide"], 1);
    assert_eq!(state["stations"]["current"], 1);

    let tide = &state["tide"];
    assert_eq!(tide["station"], "GATE");
    let height = tide["heightM"].as_f64().unwrap();
    assert!((0.0..3.0).contains(&height), "{height}");
    let turns = tide["turns"].as_array().unwrap();
    assert!(turns.len() >= 4, "{turns:?}");
    assert!(
        turns
            .iter()
            .all(|t| t["turn"] == "high" || t["turn"] == "low")
    );
    // In time order, and alternating.
    for pair in turns.windows(2) {
        assert!(pair[0]["time"].as_str() < pair[1]["time"].as_str());
        assert_ne!(pair[0]["turn"], pair[1]["turn"]);
    }

    let current = &state["current"];
    assert_eq!(current["station"], "STREAM");
    // A stream's size is never negative; `way` says which way it runs.
    assert!(current["knots"].as_f64().unwrap() >= 0.0);
    assert!(["flood", "ebb", "slack"].contains(&current["way"].as_str().unwrap()));
    assert_eq!(
        current["setDeg"],
        if current["way"] == "ebb" { 238.0 } else { 52.0 }
    );
}

#[test]
fn a_curve_carries_its_points_and_its_turns() {
    let engine = start("curve");
    let mut app = App::connect(&engine);
    app.read();
    app.read();

    let m = app.ask(json!({"type": "curve", "id": 7, "station": "GATE",
                           "hours": 24, "stepSeconds": 600}));
    assert_eq!(m["type"], "curve");
    assert_eq!(m["id"], 7);
    assert_eq!(m["kind"], "tide");
    assert_eq!(m["unit"], "m");
    assert_eq!(m["start"], "2026-09-20T12:00:00Z");
    assert_eq!(m["stepSeconds"], 600);
    let values = m["values"].as_array().unwrap();
    assert_eq!(values.len(), 24 * 6 + 1);
    assert!(
        values
            .iter()
            .all(|v| v.as_f64().is_some_and(f64::is_finite))
    );

    // A fortnight at a fine step is coarsened, not cut.
    let long = app.ask(json!({"type": "curve", "station": "GATE",
                              "hours": 336, "stepSeconds": 60}));
    let step = long["stepSeconds"].as_i64().unwrap();
    let points = long["values"].as_array().unwrap().len();
    assert!(step > 60, "step stayed at {step}");
    assert!(points <= 2016, "{points} points");
    let covered = (points as i64 - 1) * step;
    assert!(covered >= 336 * 3600 - step, "only {covered} s covered");

    // A current comes back signed, so a client can shade the ebb.
    let stream = app.ask(json!({"type": "curve", "station": "STREAM", "hours": 24}));
    assert_eq!(stream["unit"], "kn");
    let values = stream["values"].as_array().unwrap();
    assert!(values.iter().any(|v| v.as_f64().unwrap() > 0.0));
    assert!(values.iter().any(|v| v.as_f64().unwrap() < 0.0));
}

#[test]
fn a_request_the_engine_cant_serve_is_answered_with_why() {
    let engine = start("errors");
    let mut app = App::connect(&engine);
    app.read();
    app.read();

    for (request, wanted) in [
        (
            json!({"type": "curve", "id": 1, "station": "nowhere"}),
            "no station",
        ),
        (
            json!({"type": "curve", "id": 2, "time": "yesterday"}),
            "isn't a UTC time",
        ),
        (json!({"type": "wobble", "id": 3}), "doesn't know"),
        (json!({"id": 4}), "needs a type"),
    ] {
        let m = app.ask(request.clone());
        assert_eq!(m["type"], "error", "{request}");
        assert_eq!(m["id"], request["id"]);
        let said = m["message"].as_str().unwrap();
        assert!(said.contains(wanted), "{request} said {said}");
    }

    // Not JSON at all, and a line too long to be one.
    app.writer.write_all(b"not json\n").unwrap();
    assert_eq!(app.read()["message"], "not a JSON object");
    app.writer
        .write_all(format!("{}\n", "x".repeat(70 * 1024)).as_bytes())
        .unwrap();
    assert_eq!(app.read()["message"], "line too long");
}

#[test]
fn two_engines_cant_share_one_socket() {
    let engine = start("lock");
    let second = engine::Config {
        socket: engine.socket.clone(),
        keel: None,
        data: engine.socket.parent().unwrap().to_path_buf(),
        settings: engine.socket.with_file_name("config.toml"),
        clock,
    };
    let e = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(engine::run(second))
        .unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::AddrInUse, "{e}");
}
