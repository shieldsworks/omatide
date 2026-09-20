//! omatide's predictions against NOAA's own published tables.
//!
//! This is the test that matters. Everything else checks that the code
//! does what it was written to do; this checks that what it was written
//! to do is right, against the tables every other chart, almanac and app
//! is printing from.
//!
//! The windows are spread across seven years of the moon's 18.6-year
//! nodal cycle. A node factor can be wrong by twenty per cent and still
//! look perfect in a single year, so one year would prove very little.
//! Rebuild the fixtures with `scripts/reference.py`.

use omatide::coops;
use omatide::predict::{self, Extreme, Harmonics, Turn};
use omatide::station::{CurrentOffsets, HeightRule, TideOffsets};
use omatide::time;

fn fixture(path: &str) -> String {
    std::fs::read_to_string(format!("tests/fixtures/{path}")).expect(path)
}

/// `2026-09-20T01:00:00Z 1.503 H` → the time, the value, and the rest.
fn rows(name: &str) -> Vec<(i64, f64, String)> {
    fixture(&format!("noaa/{name}"))
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|line| {
            let mut parts = line.split_whitespace();
            let t = time::parse_iso(parts.next().expect("time")).expect("a UTC time");
            let v: f64 = parts.next().expect("value").parse().expect("a number");
            (t, v, parts.next().unwrap_or("").to_string())
        })
        .collect()
}

fn san_francisco() -> Harmonics {
    coops::tide_harmonics(
        &fixture("coops/9414290-harcon.json"),
        &fixture("coops/9414290-datums.json"),
    )
    .unwrap()
}

fn raccoon_strait() -> Harmonics {
    coops::current_harmonics(&fixture("coops/SFB1212-harcon.json"), 9)
        .unwrap()
        .0
}

fn golden_gate() -> Harmonics {
    coops::current_harmonics(&fixture("coops/SFB1202-harcon.json"), 17)
        .unwrap()
        .0
}

/// NOAA prints heights to the millimeter, so anything under about two
/// millimeters is agreement.
const HEIGHT_TOLERANCE: f64 = 0.003;
/// Speeds carry more slack than heights, and the reason is in NOAA's own
/// numbers: a current station's constants are published to a hundredth
/// of a centimeter a second and a tenth of a degree, and twenty-five of
/// them summing to nearly two meters a second leaves about two tenths of
/// a centimeter a second of rounding. Measured worst case is 0.216, or
/// four thousandths of a knot.
const SPEED_TOLERANCE: f64 = 0.4;
/// Turns are printed to the minute, and a turn is flat, so the minute
/// either side rounds differently.
const TURN_TOLERANCE: i64 = 90;

#[test]
fn heights_match_noaas_tables_across_the_nodal_cycle() {
    let h = san_francisco();
    let mut worst: f64 = 0.0;
    let mut sum = 0.0;
    let rows = rows("9414290-heights.txt");
    assert!(rows.len() > 600, "only {} rows", rows.len());
    for (t, want, _) in &rows {
        let got = predict::series(&h, *t, *t, 1)[0].value;
        let error = (got - want).abs();
        worst = worst.max(error);
        sum += error * error;
        assert!(
            error < HEIGHT_TOLERANCE,
            "{}: {got:.4} against NOAA's {want:.3}",
            time::iso(*t)
        );
    }
    let rms = (sum / rows.len() as f64).sqrt();
    assert!(rms < 0.001, "rms {rms:.4} m");
    println!(
        "heights: worst {worst:.4} m, rms {rms:.4} m over {} rows",
        rows.len()
    );
}

#[test]
fn high_and_low_water_match_noaas_tables() {
    let h = san_francisco();
    let published = rows("9414290-hilo.txt");
    check_turns(&published, &|start, end| predict::extremes(&h, start, end));
}

#[test]
fn a_subordinate_station_matches_noaas_tables_too() {
    // Sausalito: ten minutes after the Gate at high water, fourteen at
    // low, and 0.97 of the height.
    let h = san_francisco();
    let offsets = coops_tide_offsets("9414806-tidepredoffsets.json");
    let published = rows("9414806-hilo.txt");
    check_turns(&published, &|start, end| {
        omatide::subordinate::tide_extremes(&h, &offsets, start, end)
    });
}

#[test]
fn the_stream_through_raccoon_strait_matches_noaas_tables() {
    let h = raccoon_strait();
    let rows = rows("SFB1212-9-currents.txt");
    assert!(rows.len() > 300, "only {} rows", rows.len());
    let mut worst: f64 = 0.0;
    for (t, want, _) in &rows {
        let got = predict::series(&h, *t, *t, 1)[0].value;
        let error = (got - want).abs();
        worst = worst.max(error);
        assert!(
            error < SPEED_TOLERANCE,
            "{}: {got:.3} against NOAA's {want:.1} cm/s",
            time::iso(*t)
        );
    }
    println!(
        "raccoon strait: worst {worst:.3} cm/s over {} rows",
        rows.len()
    );
}

#[test]
fn slack_and_maximum_stream_match_noaas_tables() {
    for (name, h) in [
        ("SFB1212-9-maxslack.txt", raccoon_strait()),
        ("SFB1202-17-maxslack.txt", golden_gate()),
    ] {
        let published = rows(name);
        check_turns(&published, &|start, end| {
            predict::current_turns(&h, start, end)
        });
    }
}

#[test]
fn a_subordinate_stream_matches_noaas_tables() {
    // Alcatraz, west of: it follows the Golden Gate, with a different
    // shift on each of the four turns and a different factor on the
    // flood and the ebb.
    let h = golden_gate();
    let offsets = coops_current_offsets("PCT0291-1-currentpredictionoffsets.json");
    let published = rows("PCT0291-1-maxslack.txt");
    check_turns(&published, &|start, end| {
        omatide::subordinate::current_turns(&h, &offsets, start, end)
    });
}

/// Every turn NOAA printed must be one omatide found, at the same minute
/// and the same height or speed — and omatide must not invent extra ones.
fn check_turns(published: &[(i64, f64, String)], predict: &dyn Fn(i64, i64) -> Vec<Extreme>) {
    assert!(published.len() > 100, "only {} turns", published.len());
    // The fixtures are several separate windows; walk each one.
    let mut i = 0;
    let mut checked = 0;
    while i < published.len() {
        let mut j = i;
        while j + 1 < published.len() && published[j + 1].0 - published[j].0 < 3 * 86_400 {
            j += 1;
        }
        let window = &published[i..=j];
        let (start, end) = (window[0].0 - 3600, window[window.len() - 1].0 + 3600);
        let mut ours = predict(start, end);
        // A weak maximum between two slacks is noise NOAA does not print.
        ours = predict::drop_weak_turns(ours, 1.0);
        for (time_, value, kind) in window {
            let found = ours
                .iter()
                .filter(|e| matches(e.turn, kind))
                .min_by_key(|e| (e.time - time_).abs())
                .unwrap_or_else(|| panic!("nothing like {kind} near {}", time::iso(*time_)));
            let off = (found.time - time_).abs();
            assert!(
                off <= TURN_TOLERANCE,
                "{kind} at {}: omatide says {} ({off} s out)",
                time::iso(*time_),
                time::iso(found.time)
            );
            // Slack is zero by definition; NOAA prints the rounding.
            if found.turn != Turn::Slack {
                let tolerance = if matches!(found.turn, Turn::High | Turn::Low) {
                    HEIGHT_TOLERANCE
                } else {
                    SPEED_TOLERANCE
                };
                assert!(
                    (found.value - value).abs() < tolerance,
                    "{kind} at {}: {} against NOAA's {value}",
                    time::iso(*time_),
                    found.value
                );
            }
            checked += 1;
        }
        // No invented turns: as many inside the window as NOAA printed.
        // A turn can fall a few seconds either side of the one NOAA
        // printed, so the count runs to the same tolerance as the times.
        let first = window[0].0 - TURN_TOLERANCE;
        let last = window[window.len() - 1].0 + TURN_TOLERANCE;
        let inside = ours
            .iter()
            .filter(|e| (first..=last).contains(&e.time))
            .count();
        assert_eq!(inside, window.len(), "turn counts differ in one window");
        i = j + 1;
    }
    println!("checked {checked} turns");
}

fn matches(turn: Turn, kind: &str) -> bool {
    match kind {
        "H" => turn == Turn::High,
        "L" => turn == Turn::Low,
        "slack" => turn == Turn::Slack,
        "flood" => turn == Turn::Flood,
        "ebb" => turn == Turn::Ebb,
        other => panic!("unknown turn {other}"),
    }
}

fn coops_tide_offsets(name: &str) -> TideOffsets {
    let mut s = blank();
    coops::apply_tide_offsets(&mut s, &fixture(&format!("coops/{name}"))).unwrap();
    match s.source {
        omatide::station::Source::Tide { offsets, .. } => offsets,
        other => panic!("{other:?}"),
    }
}

fn coops_current_offsets(name: &str) -> CurrentOffsets {
    let mut s = blank();
    s.kind = omatide::station::Kind::Current;
    coops::apply_current_offsets(&mut s, &fixture(&format!("coops/{name}"))).unwrap();
    match s.source {
        omatide::station::Source::Current { offsets, .. } => offsets,
        other => panic!("{other:?}"),
    }
}

fn blank() -> omatide::station::Station {
    omatide::station::Station {
        id: "x".into(),
        bin: None,
        name: "x".into(),
        lat: 37.8,
        lon: -122.4,
        kind: omatide::station::Kind::Tide,
        depth: None,
        flood_direction: None,
        ebb_direction: None,
        source: omatide::station::Source::Harmonic,
    }
}

/// Keeps the unused-import warning away when only some tests run.
#[allow(dead_code)]
fn _rules() -> HeightRule {
    HeightRule::Ratio
}
