//! The stations omatide predicts from, and what NOAA knows about each.
//!
//! There are two sorts. A **harmonic** station has its own constants,
//! measured over years, and can be predicted at any moment. A
//! **subordinate** station has only a set of offsets against a harmonic
//! one: so many minutes later at high water, so much of the height. Most
//! of the places a sailor names in San Francisco Bay — Red Rock, Point
//! Blunt, Sausalito — are subordinate, which is why omatide carries both.

use crate::json::{self, Json};
use crate::predict::Harmonics;
use std::collections::BTreeMap;

/// Whether a station reports a water level or a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Tide,
    Current,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Tide => "tide",
            Kind::Current => "current",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "tide" => Some(Kind::Tide),
            "current" => Some(Kind::Current),
            _ => None,
        }
    }
}

/// How a subordinate station's heights follow its reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeightRule {
    /// Multiply the reference's height above chart datum.
    Ratio,
    /// Add a fixed number of meters.
    Add,
}

/// A subordinate tide station's offsets from its reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TideOffsets {
    pub high_minutes: i64,
    pub low_minutes: i64,
    pub high_height: f64,
    pub low_height: f64,
    pub rule: HeightRule,
}

impl TideOffsets {
    /// The reference's high (or low) water, moved to this station.
    pub fn apply(&self, value: f64, high: bool) -> f64 {
        let (shift, _) = if high {
            (self.high_height, self.high_minutes)
        } else {
            (self.low_height, self.low_minutes)
        };
        match self.rule {
            HeightRule::Ratio => value * shift,
            HeightRule::Add => value + shift,
        }
    }

    pub fn minutes(&self, high: bool) -> i64 {
        if high {
            self.high_minutes
        } else {
            self.low_minutes
        }
    }
}

/// A subordinate current station's offsets. A stream has four turns in a
/// cycle, not two, and NOAA gives each its own shift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurrentOffsets {
    pub slack_before_flood_minutes: i64,
    pub max_flood_minutes: i64,
    pub slack_before_ebb_minutes: i64,
    pub max_ebb_minutes: i64,
    pub flood_factor: f64,
    pub ebb_factor: f64,
}

/// Where a station's predictions come from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// Its own harmonic constants.
    Harmonic,
    /// Offsets against another station.
    Tide {
        reference: String,
        offsets: TideOffsets,
    },
    Current {
        reference: String,
        offsets: CurrentOffsets,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Station {
    pub id: String,
    /// Current stations are measured in bins down the water column; a
    /// tide station has none.
    pub bin: Option<u32>,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub kind: Kind,
    /// Meters below the surface, for a current bin.
    pub depth: Option<f64>,
    /// True directions the stream sets on the flood and the ebb.
    pub flood_direction: Option<f64>,
    pub ebb_direction: Option<f64>,
    pub source: Source,
}

impl Station {
    /// `9414290`, or `SFB1212-9` for a current bin: how a station is
    /// named in the cache, the protocol and on the command line.
    pub fn key(&self) -> String {
        match self.bin {
            Some(bin) => format!("{}-{bin}", self.id),
            None => self.id.clone(),
        }
    }

    /// The reference station's key, for a subordinate station.
    pub fn reference(&self) -> Option<&str> {
        match &self.source {
            Source::Harmonic => None,
            Source::Tide { reference, .. } | Source::Current { reference, .. } => Some(reference),
        }
    }

    /// Great-circle distance in nautical miles.
    pub fn distance_from(&self, lat: f64, lon: f64) -> f64 {
        haversine_nm(lat, lon, self.lat, self.lon)
    }
}

/// Great-circle distance in nautical miles, on a sphere. Good to a few
/// meters over a bay, which is all any of this needs.
pub fn haversine_nm(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_NM: f64 = 3440.065;
    let to_rad = std::f64::consts::PI / 180.0;
    let (p1, p2) = (lat1 * to_rad, lat2 * to_rad);
    let dp = (lat2 - lat1) * to_rad;
    let dl = (lon2 - lon1) * to_rad;
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_NM * a.sqrt().asin().min(std::f64::consts::FRAC_PI_2)
}

/// Every station omatide has, and the constants for the harmonic ones.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub stations: Vec<Station>,
    constants: BTreeMap<String, Harmonics>,
}

impl Catalog {
    pub fn get(&self, key: &str) -> Option<&Station> {
        self.stations.iter().find(|s| s.key() == key)
    }

    pub fn harmonics(&self, key: &str) -> Option<&Harmonics> {
        self.constants.get(key)
    }

    pub fn insert_harmonics(&mut self, key: String, h: Harmonics) {
        self.constants.insert(key, h);
    }

    /// Can this station actually be predicted — does it, or the station
    /// it follows, have constants?
    pub fn is_ready(&self, key: &str) -> bool {
        let Some(station) = self.get(key) else {
            return false;
        };
        match station.reference() {
            None => self.constants.contains_key(key),
            Some(reference) => self.constants.contains_key(reference),
        }
    }

    /// The nearest station of a kind that can be predicted, and how far
    /// off it is in nautical miles.
    ///
    /// A current station's bins all sit at one position, so distance
    /// alone can't choose between them and whichever NOAA happened to
    /// list first would win. `depth` breaks that tie the way
    /// `bay::key_for` does: the bin nearest the depth wanted, with an
    /// unmeasured one last. A tide station has one gauge, so it has no
    /// bins to choose between and `depth` never comes into it.
    pub fn nearest(&self, lat: f64, lon: f64, kind: Kind, depth: f64) -> Option<(&Station, f64)> {
        self.stations
            .iter()
            .filter(|s| s.kind == kind && self.is_ready(&s.key()))
            .map(|s| {
                let off = s.depth.map_or(f64::MAX / 2.0, |d| (d - depth).abs());
                (s, s.distance_from(lat, lon), off)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
            .map(|(s, nm, _)| (s, nm))
    }

    /// Every predictable station of a kind within a distance, nearest
    /// first.
    pub fn within(&self, lat: f64, lon: f64, kind: Kind, nm: f64) -> Vec<(&Station, f64)> {
        let mut out: Vec<_> = self
            .stations
            .iter()
            .filter(|s| s.kind == kind && self.is_ready(&s.key()))
            .map(|s| (s, s.distance_from(lat, lon)))
            .filter(|(_, d)| *d <= nm)
            .collect();
        out.sort_by(|a, b| a.1.total_cmp(&b.1));
        out
    }
}

// ---------------------------------------------------------------- json

impl Catalog {
    /// Reads the catalog file. Unknown fields are ignored, so a newer
    /// omatide's cache still opens in an older one.
    pub fn from_json(text: &str) -> Result<Catalog, String> {
        let value = json::parse(text)?;
        let list = value
            .get("stations")
            .and_then(Json::as_array)
            .ok_or("no stations")?;
        let mut stations = Vec::with_capacity(list.len());
        for entry in list {
            stations.push(station_from_json(entry)?);
        }
        let mut constants = BTreeMap::new();
        if let Some(map) = value.get("harmonics").and_then(Json::as_object) {
            for (key, entry) in map {
                constants.insert(key.clone(), harmonics_from_json(entry)?);
            }
        }
        Ok(Catalog {
            stations,
            constants,
        })
    }

    pub fn to_json(&self) -> String {
        let mut out = String::from("{\n  \"stations\": [\n");
        for (i, s) in self.stations.iter().enumerate() {
            out.push_str("    ");
            out.push_str(&station_to_json(s));
            if i + 1 < self.stations.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  ],\n  \"harmonics\": {\n");
        for (i, (key, h)) in self.constants.iter().enumerate() {
            out.push_str(&format!(
                "    {}: {}",
                json::string(key),
                harmonics_to_json(h)
            ));
            if i + 1 < self.constants.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  }\n}\n");
        out
    }
}

fn station_from_json(v: &Json) -> Result<Station, String> {
    let id = v
        .get("id")
        .and_then(Json::as_str)
        .ok_or("station has no id")?;
    let kind = v
        .get("kind")
        .and_then(Json::as_str)
        .and_then(Kind::parse)
        .ok_or_else(|| format!("{id}: no kind"))?;
    let source = match v.get("reference").and_then(Json::as_str) {
        None => Source::Harmonic,
        Some(reference) => {
            let o = v
                .get("offsets")
                .ok_or_else(|| format!("{id}: no offsets"))?;
            let number = |name: &str| o.get(name).and_then(Json::as_f64).unwrap_or(0.0);
            let minutes = |name: &str| number(name).round() as i64;
            match kind {
                Kind::Tide => Source::Tide {
                    reference: reference.to_string(),
                    offsets: TideOffsets {
                        high_minutes: minutes("high_minutes"),
                        low_minutes: minutes("low_minutes"),
                        high_height: number("high_height"),
                        low_height: number("low_height"),
                        rule: match o.get("rule").and_then(Json::as_str) {
                            Some("add") => HeightRule::Add,
                            _ => HeightRule::Ratio,
                        },
                    },
                },
                Kind::Current => Source::Current {
                    reference: reference.to_string(),
                    offsets: CurrentOffsets {
                        slack_before_flood_minutes: minutes("slack_before_flood_minutes"),
                        max_flood_minutes: minutes("max_flood_minutes"),
                        slack_before_ebb_minutes: minutes("slack_before_ebb_minutes"),
                        max_ebb_minutes: minutes("max_ebb_minutes"),
                        flood_factor: o.get("flood_factor").and_then(Json::as_f64).unwrap_or(1.0),
                        ebb_factor: o.get("ebb_factor").and_then(Json::as_f64).unwrap_or(1.0),
                    },
                },
            }
        }
    };
    Ok(Station {
        id: id.to_string(),
        bin: v
            .get("bin")
            .and_then(Json::as_f64)
            .map(|b| b.round().max(0.0) as u32),
        name: v
            .get("name")
            .and_then(Json::as_str)
            .unwrap_or(id)
            .to_string(),
        lat: v.get("lat").and_then(Json::as_f64).unwrap_or_default(),
        lon: v.get("lon").and_then(Json::as_f64).unwrap_or_default(),
        kind,
        depth: v.get("depth").and_then(Json::as_f64),
        flood_direction: v.get("flood").and_then(Json::as_f64),
        ebb_direction: v.get("ebb").and_then(Json::as_f64),
        source,
    })
}

fn station_to_json(s: &Station) -> String {
    let mut fields = vec![
        format!("\"id\": {}", json::string(&s.id)),
        format!("\"kind\": \"{}\"", s.kind.name()),
        format!("\"name\": {}", json::string(&s.name)),
        format!("\"lat\": {:.5}", s.lat),
        format!("\"lon\": {:.5}", s.lon),
    ];
    if let Some(bin) = s.bin {
        fields.push(format!("\"bin\": {bin}"));
    }
    if let Some(depth) = s.depth {
        fields.push(format!("\"depth\": {depth:.2}"));
    }
    if let Some(d) = s.flood_direction {
        fields.push(format!("\"flood\": {d:.1}"));
    }
    if let Some(d) = s.ebb_direction {
        fields.push(format!("\"ebb\": {d:.1}"));
    }
    match &s.source {
        Source::Harmonic => {}
        Source::Tide { reference, offsets } => {
            fields.push(format!("\"reference\": {}", json::string(reference)));
            fields.push(format!(
                "\"offsets\": {{\"high_minutes\": {}, \"low_minutes\": {}, \
                 \"high_height\": {:.4}, \"low_height\": {:.4}, \"rule\": \"{}\"}}",
                offsets.high_minutes,
                offsets.low_minutes,
                offsets.high_height,
                offsets.low_height,
                match offsets.rule {
                    HeightRule::Ratio => "ratio",
                    HeightRule::Add => "add",
                }
            ));
        }
        Source::Current { reference, offsets } => {
            fields.push(format!("\"reference\": {}", json::string(reference)));
            fields.push(format!(
                "\"offsets\": {{\"slack_before_flood_minutes\": {}, \
                 \"max_flood_minutes\": {}, \"slack_before_ebb_minutes\": {}, \
                 \"max_ebb_minutes\": {}, \"flood_factor\": {:.4}, \"ebb_factor\": {:.4}}}",
                offsets.slack_before_flood_minutes,
                offsets.max_flood_minutes,
                offsets.slack_before_ebb_minutes,
                offsets.max_ebb_minutes,
                offsets.flood_factor,
                offsets.ebb_factor
            ));
        }
    }
    format!("{{{}}}", fields.join(", "))
}

fn harmonics_from_json(v: &Json) -> Result<Harmonics, String> {
    let mut h = Harmonics {
        offset: v.get("offset").and_then(Json::as_f64).unwrap_or(0.0),
        ..Harmonics::default()
    };
    let list = v
        .get("constituents")
        .and_then(Json::as_array)
        .ok_or("no constituents")?;
    for entry in list {
        let name = entry.get("name").and_then(Json::as_str).unwrap_or("");
        let Some(i) = crate::constituent::index_of(name) else {
            // A constituent we don't carry: skip it rather than refuse
            // the whole station.
            continue;
        };
        h.amplitude[i] = entry.get("amplitude").and_then(Json::as_f64).unwrap_or(0.0);
        h.phase[i] = entry.get("phase").and_then(Json::as_f64).unwrap_or(0.0);
    }
    Ok(h)
}

fn harmonics_to_json(h: &Harmonics) -> String {
    let mut parts = Vec::new();
    for (i, c) in crate::constituent::ALL.iter().enumerate() {
        if h.amplitude[i] == 0.0 {
            continue;
        }
        parts.push(format!(
            "{{\"name\": \"{}\", \"amplitude\": {:.5}, \"phase\": {:.2}}}",
            c.name, h.amplitude[i], h.phase[i]
        ));
    }
    format!(
        "{{\"offset\": {:.5}, \"constituents\": [{}]}}",
        h.offset,
        parts.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constituent::index_of;

    fn sample() -> Catalog {
        let mut c = Catalog::default();
        c.stations.push(Station {
            id: "9414290".into(),
            bin: None,
            name: "San Francisco (Golden Gate)".into(),
            lat: 37.8063,
            lon: -122.4659,
            kind: Kind::Tide,
            depth: None,
            flood_direction: None,
            ebb_direction: None,
            source: Source::Harmonic,
        });
        c.stations.push(Station {
            id: "9414806".into(),
            bin: None,
            name: "Sausalito".into(),
            lat: 37.8467,
            lon: -122.477,
            kind: Kind::Tide,
            depth: None,
            flood_direction: None,
            ebb_direction: None,
            source: Source::Tide {
                reference: "9414290".into(),
                offsets: TideOffsets {
                    high_minutes: 10,
                    low_minutes: 14,
                    high_height: 0.97,
                    low_height: 1.0,
                    rule: HeightRule::Ratio,
                },
            },
        });
        c.stations.push(Station {
            id: "SFB1212".into(),
            bin: Some(9),
            name: "Raccoon Strait".into(),
            lat: 37.8719,
            lon: -122.442,
            kind: Kind::Current,
            depth: Some(5.79),
            flood_direction: Some(57.0),
            ebb_direction: Some(237.0),
            source: Source::Harmonic,
        });
        let mut h = Harmonics::default();
        h.amplitude[index_of("M2").unwrap()] = 0.576;
        h.phase[index_of("M2").unwrap()] = 208.2;
        h.offset = 0.951;
        c.insert_harmonics("9414290".into(), h);
        c
    }

    #[test]
    fn a_catalog_survives_a_round_trip_through_json() {
        let before = sample();
        let after = Catalog::from_json(&before.to_json()).unwrap();
        assert_eq!(before.stations, after.stations);
        for s in &before.stations {
            assert_eq!(before.harmonics(&s.key()), after.harmonics(&s.key()));
        }
        assert_eq!(after.get("SFB1212-9").unwrap().name, "Raccoon Strait");
        assert_eq!(after.get("9414806").unwrap().reference(), Some("9414290"));
        assert_eq!(after.get("9414290").unwrap().reference(), None);
    }

    #[test]
    fn a_station_is_ready_when_its_reference_has_constants() {
        let c = sample();
        assert!(c.is_ready("9414290"));
        // Sausalito has no constants of its own, but San Francisco does.
        assert!(c.is_ready("9414806"));
        // Raccoon Strait is harmonic and has none yet.
        assert!(!c.is_ready("SFB1212-9"));
        assert!(!c.is_ready("nowhere"));
    }

    #[test]
    fn the_nearest_current_is_the_bin_nearest_the_depth_wanted() {
        let mut c = Catalog::default();
        let mut bin = |id: &str, n: u32, lat, lon, depth| {
            c.stations.push(Station {
                id: id.into(),
                bin: Some(n),
                name: format!("{id} bin {n}"),
                lat,
                lon,
                kind: Kind::Current,
                depth: Some(depth),
                flood_direction: Some(57.0),
                ebb_direction: Some(237.0),
                source: Source::Harmonic,
            });
        };
        // Three bins of one station, all at one position, listed deep
        // first so the answer can't come from the order.
        bin("SFB1212", 1, 37.8719, -122.442, 30.0);
        bin("SFB1212", 2, 37.8719, -122.442, 12.0);
        bin("SFB1212", 3, 37.8719, -122.442, 4.6);
        // And a nearer station, whose only bin is deep.
        bin("SFB1203", 1, 37.8721, -122.4421, 40.0);
        let mut h = Harmonics::default();
        h.amplitude[index_of("M2").unwrap()] = 80.0;
        for key in ["SFB1212-1", "SFB1212-2", "SFB1212-3", "SFB1203-1"] {
            c.insert_harmonics(key.into(), h.clone());
        }

        // Distance wins first: the nearer station, whatever its depth.
        let (s, _) = c.nearest(37.872, -122.4421, Kind::Current, 3.0).unwrap();
        assert_eq!(s.key(), "SFB1203-1");
        // Between bins of one station distance is a tie, so the depth
        // asked for decides - a boat feels the top of the stream.
        let (s, _) = c.nearest(37.8719, -122.442, Kind::Current, 3.0).unwrap();
        assert_eq!(s.key(), "SFB1212-3");
        let (s, _) = c.nearest(37.8719, -122.442, Kind::Current, 25.0).unwrap();
        assert_eq!(s.key(), "SFB1212-1");
    }

    #[test]
    fn the_nearest_station_is_one_that_can_be_predicted() {
        let c = sample();
        // Off Sausalito: the nearest tide station is Sausalito itself.
        let (s, nm) = c.nearest(37.85, -122.48, Kind::Tide, 3.0).unwrap();
        assert_eq!(s.id, "9414806");
        assert!(nm < 1.0, "{nm} nm");
        // No current station is ready, so there is no nearest current.
        assert!(c.nearest(37.85, -122.48, Kind::Current, 3.0).is_none());
        let near = c.within(37.85, -122.48, Kind::Tide, 50.0);
        assert_eq!(near.len(), 2);
        assert!(near[0].1 <= near[1].1);
        assert!(c.within(37.85, -122.48, Kind::Tide, 0.1).is_empty());
    }

    #[test]
    fn distances_are_nautical_miles() {
        // A degree of latitude is 60 nautical miles, by definition.
        assert!((haversine_nm(37.0, -122.0, 38.0, -122.0) - 60.0).abs() < 0.2);
        assert_eq!(haversine_nm(37.0, -122.0, 37.0, -122.0), 0.0);
        // The Gate to Raccoon Strait is about four and a half miles.
        let d = haversine_nm(37.8063, -122.4659, 37.8719, -122.442);
        assert!((4.0..5.0).contains(&d), "{d} nm");
    }

    #[test]
    fn offsets_shift_a_height_by_ratio_or_by_meters() {
        let ratio = TideOffsets {
            high_minutes: 10,
            low_minutes: 14,
            high_height: 0.97,
            low_height: 1.0,
            rule: HeightRule::Ratio,
        };
        assert!((ratio.apply(1.561, true) - 1.514).abs() < 5e-4);
        assert!((ratio.apply(0.222, false) - 0.222).abs() < 1e-9);
        assert_eq!(ratio.minutes(true), 10);
        assert_eq!(ratio.minutes(false), 14);
        let add = TideOffsets {
            rule: HeightRule::Add,
            high_height: -0.06,
            low_height: 0.0,
            ..ratio
        };
        assert!((add.apply(1.561, true) - 1.501).abs() < 1e-9);
        assert!((add.apply(0.222, false) - 0.222).abs() < 1e-9);
    }
}
