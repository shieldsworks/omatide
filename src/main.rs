use omatide::{bay, cache, config, coops, engine, fetch, keel, station::Kind, tides, time};
use std::path::PathBuf;
use std::{env, process::ExitCode};

const USAGE: &str = "\
usage: omatide run [--socket PATH] [--keel PATH] [--no-keel]
       omatide watch [--socket PATH]
       omatide fetch [--region S,W,N,E]
       omatide stations [tide|current] [--near LAT,LON] [--within NM]
       omatide tide [STATION|LAT,LON] [--days N]
       omatide current [STATION|LAT,LON] [--days N]
       omatide bay [--at TIME]
       omatide --version

run       serve the tide and the stream to Omahoy apps, following the boat
          through omakeel. The socket defaults to
          $XDG_RUNTIME_DIR/omatide/tide.sock and omakeel's to
          $XDG_RUNTIME_DIR/omakeel/keel.sock.
watch     print the running engine's state as it changes.
fetch     download every station in the region from NOAA, once. Everything
          after it is worked out here, with no network: harmonic constants
          are good for years.
stations  list the stations omatide has.
tide      the tide at a station, or at the nearest one to a position:
          the height now and the highs and lows to come.
current   the stream at a station, or the nearest one: the set and drift
          now, and the slacks and maximums to come.
bay       the stream everywhere in San Francisco Bay at one moment.

Settings: ~/.config/omatide/config.toml (region, home, depth).
Stations: ~/.local/share/omatide/stations.json";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("omatide: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let Some((command, rest)) = args.split_first() else {
        println!("{USAGE}");
        return Ok(());
    };
    match command.as_str() {
        "run" => serve(rest),
        "watch" => watch(rest),
        "fetch" => fetch_now(rest),
        "stations" => stations(rest),
        "tide" => report(rest, Kind::Tide),
        "current" => report(rest, Kind::Current),
        "bay" => bay(rest),
        "--version" | "version" => {
            println!("omatide {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(format!("unexpected '{}'\n\n{USAGE}", args.join(" "))),
    }
}

/// What a command line came to: bare words, `--flag value` pairs, and
/// `--flag` switches, in the order they were given.
#[derive(Default)]
struct Parsed {
    words: Vec<String>,
    pairs: Vec<(String, String)>,
    switches: Vec<String>,
}

impl Parsed {
    fn value(&self, name: &str) -> Option<&str> {
        self.pairs
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    fn on(&self, name: &str) -> bool {
        self.switches.iter().any(|s| s == name)
    }

    fn no_words(&self) -> Result<(), String> {
        match self.words.first() {
            None => Ok(()),
            Some(word) => Err(format!("unexpected '{word}'\n\n{USAGE}")),
        }
    }
}

fn flags(args: &[String], pairs: &[&str], switches: &[&str]) -> Result<Parsed, String> {
    let mut out = Parsed::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let Some(name) = arg.strip_prefix("--") else {
            out.words.push(arg.clone());
            continue;
        };
        if switches.contains(&name) {
            out.switches.push(name.to_string());
        } else if pairs.contains(&name) {
            let value = it.next().ok_or_else(|| format!("{arg} needs a value"))?;
            out.pairs.push((name.to_string(), value.clone()));
        } else {
            return Err(format!("unexpected '{arg}'\n\n{USAGE}"));
        }
    }
    Ok(out)
}

fn serve(args: &[String]) -> Result<(), String> {
    let args = flags(args, &["socket", "keel"], &["no-keel"])?;
    args.no_words()?;
    let config = engine::Config {
        socket: match args.value("socket") {
            Some(s) => PathBuf::from(s),
            None => engine::default_socket().map_err(|e| e.to_string())?,
        },
        keel: if args.on("no-keel") {
            None
        } else {
            args.value("keel")
                .map(PathBuf::from)
                .or_else(keel::default_socket)
        },
        data: cache::data_dir(),
        settings: config::config_path(),
        clock: time::now,
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(engine::run(config))
        .map_err(|e| e.to_string())
}

/// Prints whatever the engine sends, as it sends it.
fn watch(args: &[String]) -> Result<(), String> {
    let args = flags(args, &["socket"], &[])?;
    args.no_words()?;
    let path = match args.value("socket") {
        Some(value) => PathBuf::from(value),
        None => engine::default_socket().map_err(|e| e.to_string())?,
    };
    use std::io::{BufRead, BufReader, Write};
    let stream = std::os::unix::net::UnixStream::connect(&path)
        .map_err(|e| format!("{}: {e}\n\nIs `omatide run` going?", path.display()))?;
    let mut out = std::io::stdout();
    for line in BufReader::new(stream).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if writeln!(out, "{line}").is_err() {
            break;
        }
    }
    Ok(())
}

fn settings() -> config::Settings {
    let (settings, problems) = config::load(&config::config_path());
    for p in problems {
        eprintln!("omatide: {p}");
    }
    settings
}

fn fetch_now(args: &[String]) -> Result<(), String> {
    let args = flags(args, &["region"], &[])?;
    args.no_words()?;
    let mut region = settings().region;
    if let Some(value) = args.value("region") {
        region = coops::Region::parse(value)
            .ok_or("region: expected south, west, north, east in degrees")?;
    }
    let report = fetch::Report {
        say: &|m: &str| println!("{m}"),
    };
    let catalog = fetch::build(region, &report)?;
    let dir = cache::data_dir();
    cache::save(&dir, &catalog)?;
    println!("saved to {}", cache::catalog_path(&dir).display());
    Ok(())
}

fn open() -> Result<omatide::station::Catalog, String> {
    cache::load(&cache::data_dir()).map_err(|e| format!("{e}\n\nRun `omatide fetch` first."))
}

fn stations(args: &[String]) -> Result<(), String> {
    let args = flags(args, &["near", "within"], &[])?;
    let only = match args.words.first().map(String::as_str) {
        None => None,
        Some("tide") => Some(Kind::Tide),
        Some("current") => Some(Kind::Current),
        Some(other) => return Err(format!("unexpected '{other}'\n\n{USAGE}")),
    };
    let catalog = open()?;
    let near = args.value("near").map(position).transpose()?;
    let within: f64 = match args.value("within") {
        Some(v) => v.parse().map_err(|_| "within: expected miles")?,
        None => f64::MAX,
    };
    let mut listed = 0;
    for s in &catalog.stations {
        if only.is_some_and(|k| k != s.kind) {
            continue;
        }
        let distance = near.map(|(lat, lon)| s.distance_from(lat, lon));
        if distance.is_some_and(|d| d > within) {
            continue;
        }
        listed += 1;
        // A dash marks a station that can't be predicted: no constants
        // of its own, and no reference in the catalog either.
        let ready = if catalog.is_ready(&s.key()) { ' ' } else { '-' };
        let depth = s.depth.map_or(String::new(), |d| format!(" {d:.1} m"));
        let away = distance.map_or(String::new(), |d| format!("  {d:.1} nm"));
        println!(
            "{ready} {:<12} {:<8} {}{depth}{away}",
            s.key(),
            s.kind.name(),
            s.name
        );
    }
    if listed == 0 {
        println!("no stations");
    }
    Ok(())
}

/// `37.87,-122.44` in degrees.
fn position(text: &str) -> Result<(f64, f64), String> {
    let (a, b) = text.split_once(',').ok_or("expected latitude, longitude")?;
    let lat: f64 = a.trim().parse().map_err(|_| "expected a latitude")?;
    let lon: f64 = b.trim().parse().map_err(|_| "expected a longitude")?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err("a position on the earth, in degrees".into());
    }
    Ok((lat, lon))
}

fn report(args: &[String], kind: Kind) -> Result<(), String> {
    let args = flags(args, &["days"], &[])?;
    let days: i64 = match args.value("days") {
        Some(v) => v
            .parse::<i64>()
            .map_err(|_| "days: expected a number")?
            .clamp(1, 14),
        None => 2,
    };
    let catalog = open()?;
    let settings = settings();
    let key = match args.words.first() {
        None => nearest_key(&catalog, settings.home, kind)?,
        Some(word) if word.contains(',') => nearest_key(&catalog, position(word)?, kind)?,
        Some(word) => word.clone(),
    };
    let station = catalog
        .get(&key)
        .ok_or_else(|| format!("no station {key}"))?;
    let now = time::now();
    let end = now + days * 86_400;
    println!("{} — {}", station.key(), station.name);
    match kind {
        Kind::Tide => {
            let value = tides::height(&catalog, &key, now)?;
            println!("  now  {value:.2} m above chart datum");
            for e in tides::extremes(&catalog, &key, now, end)? {
                println!(
                    "  {}  {:<5} {:.2} m",
                    time::iso(e.time),
                    e.turn.name(),
                    e.value
                );
            }
        }
        Kind::Current => {
            let (speed, set) = tides::stream(&catalog, &key, now)?;
            let set = set.map_or(String::new(), |d| format!(" setting {d:.0}°"));
            println!("  now  {:.2} kn{set}", speed * tides::KNOTS);
            for e in tides::current_turns(&catalog, &key, now, end)? {
                println!(
                    "  {}  {:<5} {:.2} kn",
                    time::iso(e.time),
                    e.turn.name(),
                    e.value.abs() * tides::KNOTS
                );
            }
        }
    }
    Ok(())
}

fn nearest_key(
    catalog: &omatide::station::Catalog,
    at: (f64, f64),
    kind: Kind,
) -> Result<String, String> {
    catalog
        .nearest(at.0, at.1, kind)
        .map(|(s, _)| s.key())
        .ok_or_else(|| format!("no {} station in the catalog", kind.name()))
}

fn bay(args: &[String]) -> Result<(), String> {
    let args = flags(args, &["at"], &[])?;
    args.no_words()?;
    let at = match args.value("at") {
        Some(v) => time::parse_iso(v).ok_or("at: expected a UTC time like 2026-09-20T17:00:00Z")?,
        None => time::now(),
    };
    let catalog = open()?;
    let settings = settings();
    println!("San Francisco Bay at {}", time::iso(at));
    println!("\nThe tide, height above chart datum:");
    for (place, key) in bay::resolve(&catalog, bay::TIDE_PLACES, settings.depth) {
        let height = tides::height(&catalog, &key, at)?;
        let coming = tides::extremes(&catalog, &key, at, at + 86_400)?
            .first()
            .map_or(String::new(), |e| {
                format!("  {} in {}", e.turn.name(), hours_and_minutes(e.time - at))
            });
        println!("  {:<22} {height:>5.2} m{coming}", place.name);
    }
    println!("\nThe stream:");
    for (place, key) in bay::resolve(&catalog, bay::PLACES, settings.depth) {
        let (speed, set) = tides::stream(&catalog, &key, at)?;
        let knots = speed * tides::KNOTS;
        let way = if knots.abs() < 0.05 {
            "slack"
        } else if knots > 0.0 {
            "flood"
        } else {
            "ebb  "
        };
        let set = set.map_or(String::new(), |d| format!(" {d:03.0}°"));
        let coming = tides::current_turns(&catalog, &key, at, at + 86_400)?
            .first()
            .map_or(String::new(), |e| {
                format!("  {} in {}", e.turn.name(), hours_and_minutes(e.time - at))
            });
        println!(
            "  {:<22} {:>5.1} kn {way}{set}{coming}",
            place.name,
            knots.abs()
        );
    }
    Ok(())
}

/// `1h 45m`, for how long until a turn.
fn hours_and_minutes(seconds: i64) -> String {
    let minutes = seconds / 60;
    match minutes / 60 {
        0 => format!("{minutes}m"),
        hours => format!("{hours}h {:02}m", minutes % 60),
    }
}
