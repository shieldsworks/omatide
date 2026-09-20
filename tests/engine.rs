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
    // Fifteen miles down the bay, to have something to filter out.
    station("SOUTH", Kind::Current, 37.6255, -122.2961, Source::Harmonic);
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
    c.insert_harmonics("STREAM".into(), h.clone());
    c.insert_harmonics("SOUTH".into(), h);
    cache::save(dir, &c).unwrap();
}

struct Engine {
    socket: PathBuf,
    _thread: std::thread::JoinHandle<()>,
}

fn start(name: &str) -> Engine {
    start_with_keel(name, None)
}

fn start_with_keel(name: &str, keel: Option<PathBuf>) -> Engine {
    let dir = scratch(name);
    catalog(&dir);
    let socket = dir.join("tide.sock");
    let config = engine::Config {
        socket: socket.clone(),
        keel,
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

    /// The next `state` that satisfies a test, or a panic at the read
    /// timeout. The engine pushes one whenever anything changes.
    fn wait_for(&mut self, mut ready: impl FnMut(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let m = self.read();
            if m["type"] == "state" && ready(&m) {
                return m;
            }
        }
        panic!("the engine never sent the state the test was waiting for");
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
    assert_eq!(state["stations"]["current"], 2);

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

#[test]
fn streams_over_an_area_are_what_a_chart_layer_draws() {
    let engine = start("streams");
    let mut app = App::connect(&engine);
    app.read();
    app.read();

    // Everywhere.
    let all = app.ask(json!({"type": "streams", "id": 1}));
    assert_eq!(all["type"], "streams");
    assert_eq!(all["time"], "2026-09-20T12:00:00Z");
    let found = all["streams"].as_array().unwrap();
    assert_eq!(found.len(), 2);
    assert!(all.get("more").is_none(), "nothing was truncated");
    for s in found {
        // A stream's size is never negative; `way` says which way it runs.
        assert!(s["knots"].as_f64().unwrap() >= 0.0);
        assert!(["flood", "ebb", "slack"].contains(&s["way"].as_str().unwrap()));
        assert_eq!(s["setDeg"], if s["way"] == "ebb" { 238.0 } else { 52.0 });
        assert!(s["lat"].as_f64().is_some() && s["lon"].as_f64().is_some());
        // No position was asked from, so there is no distance to give.
        assert!(s.get("distanceNm").is_none());
    }
    // Tide stations are not streams.
    assert!(found.iter().all(|s| s["station"] != "GATE"));

    // A box keeps only what is inside it.
    let box_ = app.ask(
        json!({"type": "streams", "id": 2, "south": 37.8, "west": -122.5,
                              "north": 37.9, "east": -122.4}),
    );
    let found = box_["streams"].as_array().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["station"], "STREAM");

    // A position sorts nearest first and measures the range.
    let near = app.ask(json!({"type": "streams", "id": 3, "lat": 37.63, "lon": -122.30}));
    let found = near["streams"].as_array().unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0]["station"], "SOUTH");
    assert!(found[0]["distanceNm"].as_f64().unwrap() < 1.0);
    assert!(found[1]["distanceNm"].as_f64().unwrap() > 10.0);

    // With a radius, only what is inside it.
    let close = app.ask(
        json!({"type": "streams", "id": 4, "lat": 37.63, "lon": -122.30,
                               "withinNm": 5}),
    );
    assert_eq!(close["streams"].as_array().unwrap().len(), 1);

    // The stream moves with the time asked for.
    let later = app.ask(json!({"type": "streams", "id": 5, "time": "2026-09-20T15:00:00Z"}));
    assert_eq!(later["time"], "2026-09-20T15:00:00Z");
    let before = all["streams"][0]["knots"].as_f64().unwrap();
    let after = later["streams"][0]["knots"].as_f64().unwrap();
    assert!((before - after).abs() > 0.01, "{before} then {after}");
}

#[test]
fn a_streams_request_that_makes_no_sense_is_refused() {
    let engine = start("streams-bad");
    let mut app = App::connect(&engine);
    app.read();
    app.read();

    for (request, wanted) in [
        // Half a box is not a box.
        (
            json!({"type": "streams", "id": 1, "south": 37.8}),
            "must all be given",
        ),
        // Upside down.
        (
            json!({"type": "streams", "id": 2, "south": 37.9, "west": -122.5,
                   "north": 37.8, "east": -122.4}),
            "south below north",
        ),
        (
            json!({"type": "streams", "id": 3, "lat": 91, "lon": 0}),
            "on the earth",
        ),
        (
            json!({"type": "streams", "id": 4, "lat": "north"}),
            "must be a number",
        ),
        (
            json!({"type": "streams", "id": 5, "time": "soon"}),
            "isn't a UTC time",
        ),
    ] {
        let m = app.ask(request.clone());
        assert_eq!(m["type"], "error", "{request}");
        let said = m["message"].as_str().unwrap();
        assert!(said.contains(wanted), "{request} said {said}");
    }
}

/// omakeel, for as long as one fix: it says where the boat is, then goes
/// away, which is what a pulled plug looks like from here.
fn fake_keel(dir: &Path) -> PathBuf {
    let path = dir.join("keel.sock");
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let fix = json!({
                "type": "state", "v": 1,
                "fix": {"status": "ok", "lat": 37.8292, "lon": -122.462, "ageSeconds": 0},
                "sources": [],
            });
            let _ = stream.write_all((fix.to_string() + "\n").as_bytes());
            let _ = stream.flush();
            std::thread::sleep(Duration::from_millis(400));
            // Dropped: omakeel is gone.
        }
    });
    path
}

#[test]
fn a_lost_keel_falls_back_to_home() {
    let dir = scratch("lost-keel");
    let keel = fake_keel(&dir);
    let engine = start_with_keel("lost-keel-engine", Some(keel));
    let mut app = App::connect(&engine);
    assert_eq!(app.read()["type"], "hello");

    // While omakeel is there, the tide is at the boat.
    let boat = app.wait_for(|s| s["here"]["at"] == "boat");
    assert_eq!(boat["keel"], "connected");
    let at_boat = boat["tide"]["station"].clone();

    // When it goes, the last fix goes with it: a position that is only
    // getting older is not somewhere to keep answering for.
    let home = app.wait_for(|s| s["keel"] == "lost");
    assert_eq!(home["here"]["at"], "home");
    assert!(home["here"]["lat"].as_f64().is_some());
    // And the answers follow it home.
    assert_ne!(home["tide"]["station"], Value::Null);
    assert_eq!(at_boat, json!("GATE"));
}
