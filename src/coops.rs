//! Reading NOAA CO-OPS documents.
//!
//! Everything omatide knows about a station comes from the metadata
//! service at `api.tidesandcurrents.noaa.gov/mdapi`. The shapes it
//! returns are not quite consistent — a tide station's constants are
//! keyed differently from a current station's, and numbers arrive
//! sometimes as numbers and sometimes as strings — so all of that is
//! settled here, and the rest of omatide sees only its own types.

use crate::json::{self, Json};
use crate::predict::Harmonics;
use crate::station::{CurrentOffsets, HeightRule, Kind, Source, Station, TideOffsets};

pub const MDAPI: &str = "https://api.tidesandcurrents.noaa.gov/mdapi/prod/webapi/stations";

/// A box of sea, to choose stations inside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

impl Region {
    /// San Francisco Bay, the delta it runs into, and the approaches
    /// outside the Gate. The same box omawind forecasts wind over.
    pub const BAY: Region = Region {
        south: 36.8,
        west: -123.8,
        north: 38.8,
        east: -121.6,
    };

    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        (self.south..=self.north).contains(&lat) && (self.west..=self.east).contains(&lon)
    }

    pub fn parse(s: &str) -> Option<Region> {
        let n: Vec<f64> = s
            .split(',')
            .map(|p| p.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect::<Option<_>>()?;
        let [south, west, north, east] = n[..] else {
            return None;
        };
        (south < north && west < east).then_some(Region {
            south,
            west,
            north,
            east,
        })
    }
}

pub fn tide_stations_url() -> String {
    format!("{MDAPI}.json?type=tidepredictions")
}

pub fn current_stations_url() -> String {
    format!("{MDAPI}.json?type=currentpredictions&units=metric")
}

pub fn harcon_url(id: &str) -> String {
    format!("{MDAPI}/{id}/harcon.json?units=metric")
}

pub fn datums_url(id: &str) -> String {
    format!("{MDAPI}/{id}/datums.json?units=metric")
}

pub fn tide_offsets_url(id: &str) -> String {
    format!("{MDAPI}/{id}/tidepredoffsets.json?units=metric")
}

/// Current metadata is asked for per bin, with an underscore.
pub fn current_offsets_url(id: &str, bin: u32) -> String {
    format!("{MDAPI}/{id}_{bin}/currentpredictionoffsets.json")
}

/// The stations in a region, from a `stations.json` listing.
///
/// Offsets and constants are not in the listing, so subordinate stations
/// come back pointing at nothing; [`apply_tide_offsets`] and
/// [`apply_current_offsets`] fill them in.
pub fn stations(text: &str, kind: Kind, region: Region) -> Result<Vec<Station>, String> {
    let value = json::parse(text)?;
    let list = value
        .get("stations")
        .and_then(Json::as_array)
        .ok_or("no stations in the listing")?;
    let mut out = Vec::new();
    for entry in list {
        let Some(id) = json::text(entry.get("id")) else {
            continue;
        };
        let (Some(lat), Some(lon)) = (
            json::number(entry.get("lat")),
            json::number(entry.get("lng")),
        ) else {
            continue;
        };
        if !region.contains(lat, lon) {
            continue;
        }
        out.push(Station {
            id,
            bin: json::number(entry.get("currbin")).map(|b| b.round().max(0.0) as u32),
            name: json::text(entry.get("name")).unwrap_or_default(),
            lat,
            lon,
            kind,
            depth: json::number(entry.get("depth")),
            flood_direction: None,
            ebb_direction: None,
            // The listing's `type` says R or H for a station with its
            // own constants and S for one that follows another, but the
            // offsets are what settle it, so start everyone harmonic.
            source: Source::Harmonic,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id).then(a.bin.cmp(&b.bin)));
    out.dedup_by(|a, b| a.id == b.id && a.bin == b.bin);
    Ok(out)
}

/// A tide station's harmonic constants, with mean sea level above the
/// chart datum as the level they are measured from.
///
/// NOAA's amplitudes are about mean sea level, but a tide is read above
/// mean lower low water, the chart datum. The difference between the two
/// datums is the constant every prediction sits on.
pub fn tide_harmonics(harcon: &str, datums: &str) -> Result<Harmonics, String> {
    let mut h = constants(harcon, "HarmonicConstituents", "name", |c| {
        (
            json::number(c.get("amplitude")),
            json::number(c.get("phase_GMT")),
        )
    })?;
    h.offset = datum_offset(datums)?;
    Ok(h)
}

/// Mean sea level above mean lower low water, in meters.
pub fn datum_offset(text: &str) -> Result<f64, String> {
    let value = json::parse(text)?;
    let list = value
        .get("datums")
        .and_then(Json::as_array)
        .ok_or("no datums")?;
    let find = |name: &str| {
        list.iter()
            .find(|d| d.get("name").and_then(Json::as_str) == Some(name))
            .and_then(|d| json::number(d.get("value")))
    };
    let (Some(msl), Some(mllw)) = (find("MSL"), find("MLLW")) else {
        return Err("no MSL or MLLW datum".into());
    };
    Ok(msl - mllw)
}

/// One bin of a current station's constants, along its major axis.
///
/// NOAA gives the full tidal ellipse — a major and a minor axis — but
/// predicts, and publishes, only the flow along the major one, which is
/// the channel. That is what a sailor steers against, and what this
/// returns. The azimuth of the major axis comes back with it, as the
/// direction the flood sets.
pub fn current_harmonics(harcon: &str, bin: u32) -> Result<(Harmonics, Option<f64>), String> {
    let value = json::parse(harcon)?;
    let list = value
        .get("HarmonicConstituents")
        .and_then(Json::as_array)
        .ok_or("no constituents")?;
    let mut h = Harmonics::default();
    let mut azimuth = None;
    let mut found = false;
    for c in list {
        if json::number(c.get("binNbr")).map(|b| b.round() as u32) != Some(bin) {
            continue;
        }
        found = true;
        azimuth = azimuth.or_else(|| json::number(c.get("azi")));
        // Every constituent of a bin carries the same mean flow.
        h.offset = json::number(c.get("majorMeanSpeed")).unwrap_or(h.offset);
        let Some(name) = c.get("constituentName").and_then(Json::as_str) else {
            continue;
        };
        let Some(i) = crate::constituent::index_of(name) else {
            continue;
        };
        h.amplitude[i] = json::number(c.get("majorAmplitude")).unwrap_or(0.0);
        h.phase[i] = json::number(c.get("majorPhaseGMT")).unwrap_or(0.0);
    }
    if !found {
        return Err(format!("no bin {bin} in the constants"));
    }
    Ok((h, azimuth))
}

/// Which bins a current station's constants cover.
pub fn bins(harcon: &str) -> Result<Vec<(u32, Option<f64>)>, String> {
    let value = json::parse(harcon)?;
    let list = value
        .get("HarmonicConstituents")
        .and_then(Json::as_array)
        .ok_or("no constituents")?;
    let mut out: Vec<(u32, Option<f64>)> = Vec::new();
    for c in list {
        let Some(bin) = json::number(c.get("binNbr")).map(|b| b.round().max(0.0) as u32) else {
            continue;
        };
        if !out.iter().any(|(b, _)| *b == bin) {
            out.push((bin, json::number(c.get("binDepth"))));
        }
    }
    out.sort_by_key(|(b, _)| *b);
    Ok(out)
}

/// Turns a tide station subordinate, from its offsets document.
///
/// `heightAdjustedType` is `R` where the heights are a ratio of the
/// reference's and `F` where they are meters added to it.
pub fn apply_tide_offsets(station: &mut Station, text: &str) -> Result<(), String> {
    let value = json::parse(text)?;
    let Some(reference) =
        json::text(value.get("refStationId")).filter(|s| !s.is_empty() && *s != station.id)
    else {
        // A reference station's own offsets document names no reference,
        // or names itself.
        return Ok(());
    };
    let number = |name: &str| json::number(value.get(name)).unwrap_or(0.0);
    station.source = Source::Tide {
        reference,
        offsets: TideOffsets {
            high_minutes: number("timeOffsetHighTide").round() as i64,
            low_minutes: number("timeOffsetLowTide").round() as i64,
            high_height: number("heightOffsetHighTide"),
            low_height: number("heightOffsetLowTide"),
            rule: match value.get("heightAdjustedType").and_then(Json::as_str) {
                Some("R") => HeightRule::Ratio,
                _ => HeightRule::Add,
            },
        },
    };
    Ok(())
}

/// Reads a current station's offsets document.
///
/// Every current station has one, subordinate or not: a harmonic station
/// uses it only to say which way the flood and the ebb set.
pub fn apply_current_offsets(station: &mut Station, text: &str) -> Result<(), String> {
    let value = json::parse(text)?;
    station.flood_direction = json::number(value.get("meanFloodDir")).or(station.flood_direction);
    station.ebb_direction = json::number(value.get("meanEbbDir")).or(station.ebb_direction);
    let Some(reference) = json::text(value.get("refStationId")).filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    let bin = json::number(value.get("refStationBin")).map(|b| b.round().max(0.0) as u32);
    let reference = match bin {
        Some(bin) => format!("{reference}-{bin}"),
        None => reference,
    };
    // A harmonic current station names itself as its own reference,
    // with every adjustment left null. Following that would leave it
    // waiting on constants it is supposed to provide, and take every
    // station that follows it down too — which is how Red Rock and
    // Richmond first went missing from the bay.
    if reference == station.key() {
        return Ok(());
    }
    let number = |name: &str| json::number(value.get(name)).unwrap_or(0.0);
    let factor = |name: &str| json::number(value.get(name)).unwrap_or(1.0);
    station.source = Source::Current {
        reference,
        offsets: CurrentOffsets {
            slack_before_flood_minutes: number("sbfTimeAdjMin").round() as i64,
            max_flood_minutes: number("mfcTimeAdjMin").round() as i64,
            slack_before_ebb_minutes: number("sbeTimeAdjMin").round() as i64,
            max_ebb_minutes: number("mecTimeAdjMin").round() as i64,
            flood_factor: factor("mfcAmpAdj"),
            ebb_factor: factor("mecAmpAdj"),
        },
    };
    Ok(())
}

/// The shared shape of both harmonic-constant documents.
fn constants(
    text: &str,
    list_key: &str,
    name_key: &str,
    read: impl Fn(&Json) -> (Option<f64>, Option<f64>),
) -> Result<Harmonics, String> {
    let value = json::parse(text)?;
    let list = value
        .get(list_key)
        .and_then(Json::as_array)
        .ok_or("no constituents")?;
    let mut h = Harmonics::default();
    for c in list {
        let Some(name) = c.get(name_key).and_then(Json::as_str) else {
            continue;
        };
        let Some(i) = crate::constituent::index_of(name) else {
            continue;
        };
        let (amplitude, phase) = read(c);
        h.amplitude[i] = amplitude.unwrap_or(0.0);
        h.phase[i] = phase.unwrap_or(0.0);
    }
    if h.is_empty() {
        return Err("no constituents we know".into());
    }
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constituent::index_of;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/coops/{name}")).unwrap()
    }

    #[test]
    fn a_region_holds_the_bay_and_not_the_rest_of_the_coast() {
        let bay = Region::BAY;
        assert!(bay.contains(37.8063, -122.4659)); // the Gate
        assert!(bay.contains(38.0614, -122.2182)); // Carquinez Strait
        assert!(!bay.contains(34.05, -118.25)); // Los Angeles
        assert_eq!(Region::parse("36.8,-123.8,38.8,-121.6"), Some(bay));
        assert_eq!(Region::parse("38.8,-123.8,36.8,-121.6"), None);
        assert_eq!(Region::parse("1,2,3"), None);
        assert_eq!(Region::parse("a,b,c,d"), None);
    }

    #[test]
    fn san_franciscos_constants_match_what_noaa_publishes() {
        let h = tide_harmonics(
            &fixture("9414290-harcon.json"),
            &fixture("9414290-datums.json"),
        )
        .unwrap();
        assert!((h.amplitude[index_of("M2").unwrap()] - 0.576).abs() < 1e-9);
        assert!((h.phase[index_of("M2").unwrap()] - 208.2).abs() < 1e-9);
        assert!((h.amplitude[index_of("K1").unwrap()] - 0.370).abs() < 1e-9);
        // Mean sea level stands 0.951 m above mean lower low water.
        assert!((h.offset - 0.951).abs() < 1e-9);
        // NOAA publishes all 37, though a couple are zero here.
        let used = h.amplitude.iter().filter(|a| **a > 0.0).count();
        assert!(used >= 30, "only {used} constituents");
    }

    #[test]
    fn datums_without_what_we_need_are_refused() {
        assert!(datum_offset(r#"{"datums": [{"name": "MSL", "value": 2.0}]}"#).is_err());
        assert!(datum_offset(r#"{"datums": []}"#).is_err());
        assert!(datum_offset("{}").is_err());
        assert_eq!(
            datum_offset(r#"{"datums":[{"name":"MSL","value":2.0},{"name":"MLLW","value":0.5}]}"#),
            Ok(1.5)
        );
    }

    #[test]
    fn raccoon_straits_bins_each_have_their_own_stream() {
        let text = fixture("SFB1212-harcon.json");
        let found = bins(&text).unwrap();
        assert_eq!(found.iter().map(|(b, _)| *b).collect::<Vec<_>>(), [1, 6, 9]);
        // Bin 1 is the deepest, nearest the bottom.
        assert!(found[0].1.unwrap() > found[2].1.unwrap());

        let (h, azimuth) = current_harmonics(&text, 9).unwrap();
        assert!((h.amplitude[index_of("M2").unwrap()] - 71.56).abs() < 1e-9);
        assert!((h.phase[index_of("M2").unwrap()] - 156.5).abs() < 1e-9);
        // The strait runs northeast, and the mean flow ebbs out of it.
        assert!((azimuth.unwrap() - 56.6).abs() < 1e-9);
        assert!((h.offset - -4.78).abs() < 1e-9);
        // A bin that was never measured is an error, not an empty tide.
        assert!(current_harmonics(&text, 4).is_err());
    }

    #[test]
    fn sausalito_follows_san_francisco_by_a_ratio() {
        let mut s = bare("9414806", Kind::Tide);
        apply_tide_offsets(&mut s, &fixture("9414806-tidepredoffsets.json")).unwrap();
        let Source::Tide { reference, offsets } = &s.source else {
            panic!("not subordinate: {:?}", s.source);
        };
        assert_eq!(reference, "9414290");
        assert_eq!(offsets.rule, HeightRule::Ratio);
        assert_eq!((offsets.high_minutes, offsets.low_minutes), (10, 14));
        assert!((offsets.high_height - 0.97).abs() < 1e-9);
    }

    #[test]
    fn angel_island_follows_it_by_meters_instead() {
        let mut s = bare("9414817", Kind::Tide);
        apply_tide_offsets(&mut s, &fixture("9414817-tidepredoffsets.json")).unwrap();
        let Source::Tide { offsets, .. } = &s.source else {
            panic!("not subordinate");
        };
        assert_eq!(offsets.rule, HeightRule::Add);
        assert!((offsets.high_height - -0.06).abs() < 1e-9);
    }

    #[test]
    fn a_station_with_no_reference_stays_harmonic() {
        let mut s = bare("9414290", Kind::Tide);
        apply_tide_offsets(&mut s, r#"{"refStationId": "", "type": "R"}"#).unwrap();
        assert_eq!(s.source, Source::Harmonic);
        let mut c = bare("SFB1202", Kind::Current);
        apply_current_offsets(
            &mut c,
            r#"{"refStationId": "", "meanFloodDir": 52.0, "meanEbbDir": 238.0}"#,
        )
        .unwrap();
        assert_eq!(c.source, Source::Harmonic);
        assert_eq!(c.flood_direction, Some(52.0));
        assert_eq!(c.ebb_direction, Some(238.0));
    }

    #[test]
    fn alcatraz_follows_the_gate_with_a_shift_for_each_turn() {
        let mut s = bare("PCT0291", Kind::Current);
        s.bin = Some(1);
        apply_current_offsets(&mut s, &fixture("PCT0291-1-currentpredictionoffsets.json")).unwrap();
        let Source::Current { reference, offsets } = &s.source else {
            panic!("not subordinate: {:?}", s.source);
        };
        // The reference carries its bin, because bins differ.
        assert_eq!(reference, "SFB1202-17");
        assert_eq!(offsets.max_flood_minutes, 0);
        assert_eq!(offsets.slack_before_ebb_minutes, 24);
        assert_eq!(offsets.max_ebb_minutes, 20);
        assert_eq!(offsets.slack_before_flood_minutes, 15);
        assert!((offsets.flood_factor - 0.8).abs() < 1e-9);
        assert!((offsets.ebb_factor - 1.1).abs() < 1e-9);
        assert_eq!(s.flood_direction, Some(70.0));
        assert_eq!(s.ebb_direction, Some(266.0));
    }

    #[test]
    fn a_station_that_names_itself_is_its_own_authority() {
        // NOAA's harmonic current stations do exactly this.
        let mut s = bare("PCT0666", Kind::Current);
        s.bin = Some(2);
        apply_current_offsets(
            &mut s,
            r#"{"refStationId": "PCT0666", "refStationBin": 2,
                "meanFloodDir": 328.0, "meanEbbDir": 147.0}"#,
        )
        .unwrap();
        assert_eq!(s.source, Source::Harmonic);
        assert_eq!(s.flood_direction, Some(328.0));
        // Another bin of the same station is a real reference, though.
        let mut other = bare("PCT0666", Kind::Current);
        other.bin = Some(1);
        apply_current_offsets(
            &mut other,
            r#"{"refStationId": "PCT0666", "refStationBin": 2, "mfcTimeAdjMin": 10}"#,
        )
        .unwrap();
        assert_eq!(other.reference(), Some("PCT0666-2"));
        // And a tide station that names itself stays harmonic.
        let mut t = bare("9414290", Kind::Tide);
        apply_tide_offsets(&mut t, r#"{"refStationId": "9414290"}"#).unwrap();
        assert_eq!(t.source, Source::Harmonic);
    }

    #[test]
    fn a_listing_keeps_only_the_stations_in_the_region() {
        let text = r#"{"stations": [
          {"id": "9414290", "name": "SAN FRANCISCO", "lat": 37.8063, "lng": -122.4659},
          {"id": "9410170", "name": "San Diego", "lat": 32.7142, "lng": -117.1736},
          {"id": "SFB1212", "name": "Raccoon Strait", "lat": 37.8719, "lng": -122.442,
           "currbin": 9, "depth": 5.79},
          {"id": "SFB1212", "name": "Raccoon Strait", "lat": 37.8719, "lng": -122.442,
           "currbin": 9, "depth": 5.79},
          {"name": "no id", "lat": 37.8, "lng": -122.4},
          {"id": "nowhere"}
        ]}"#;
        let found = stations(text, Kind::Tide, Region::BAY).unwrap();
        assert_eq!(found.len(), 2, "{found:#?}");
        assert_eq!(found[0].id, "9414290");
        assert_eq!(found[1].key(), "SFB1212-9");
        assert_eq!(found[1].depth, Some(5.79));
        assert!(stations("{}", Kind::Tide, Region::BAY).is_err());
        assert!(stations("not json", Kind::Tide, Region::BAY).is_err());
    }

    fn bare(id: &str, kind: Kind) -> Station {
        Station {
            id: id.into(),
            bin: None,
            name: id.into(),
            lat: 37.8,
            lon: -122.4,
            kind,
            depth: None,
            flood_direction: None,
            ebb_direction: None,
            source: Source::Harmonic,
        }
    }
}
