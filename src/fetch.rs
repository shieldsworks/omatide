//! Building the station catalog from NOAA, once, so that everything
//! after it happens offline.
//!
//! For San Francisco Bay this is about three hundred small requests and
//! takes a couple of minutes. A station that fails is reported and
//! skipped rather than losing the rest, because a catalog missing one
//! slough is still a catalog.

use crate::coops::{self, Region};
use crate::http;
use crate::station::{Catalog, Kind, Source, Station};
use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::Duration;

/// Between one request and the next, to go easy on NOAA.
const PAUSE: Duration = Duration::from_millis(150);

/// What happened while fetching, for whoever is watching.
pub struct Report<'a> {
    pub say: &'a dyn Fn(&str),
}

impl Report<'_> {
    fn note(&self, message: &str) {
        (self.say)(message);
    }
}

/// Everything NOAA knows about the tides and streams in a region.
pub fn build(region: Region, report: &Report) -> Result<Catalog, String> {
    let mut catalog = Catalog::default();
    let mut trouble = Vec::new();

    report.note("fetching the tide stations");
    let tide = coops::stations(
        &http::get_text(&coops::tide_stations_url())?,
        Kind::Tide,
        region,
    )?;
    report.note(&format!("{} tide stations in the region", tide.len()));
    for (i, mut station) in tide.into_iter().enumerate() {
        report.note(&format!("  {} {}", station.id, station.name));
        if let Err(e) = tide_station(&mut station, &mut catalog) {
            trouble.push(format!("{}: {e}", station.id));
        }
        catalog.stations.push(station);
        if i % 8 == 7 {
            sleep(PAUSE);
        }
    }

    report.note("fetching the current stations");
    let current = coops::stations(
        &http::get_text(&coops::current_stations_url())?,
        Kind::Current,
        region,
    )?;
    // The listing has one row per bin; the constants come per station.
    let mut by_station: BTreeMap<String, Vec<Station>> = BTreeMap::new();
    for station in current {
        by_station
            .entry(station.id.clone())
            .or_default()
            .push(station);
    }
    report.note(&format!(
        "{} current stations in the region",
        by_station.len()
    ));
    for (id, bins) in by_station {
        report.note(&format!("  {id} {}", bins[0].name));
        let constants = http::get_text(&coops::harcon_url(&id)).ok();
        for mut station in bins {
            if let Err(e) = current_station(&mut station, constants.as_deref(), &mut catalog) {
                trouble.push(format!("{}: {e}", station.key()));
            }
            catalog.stations.push(station);
        }
        sleep(PAUSE);
    }

    // A station near the edge of the region can follow one outside it.
    // Without its reference it predicts nothing, so fetch the constants
    // for any that are missing, wherever they are.
    for key in missing_references(&catalog) {
        report.note(&format!("  {key} (a reference outside the region)"));
        if let Err(e) = reference_constants(&key, &mut catalog) {
            trouble.push(format!("{key}: {e}"));
        }
        sleep(PAUSE);
    }

    catalog.stations.sort_by_key(Station::key);
    for t in &trouble {
        report.note(&format!("skipped {t}"));
    }
    let ready = catalog
        .stations
        .iter()
        .filter(|s| catalog.is_ready(&s.key()))
        .count();
    report.note(&format!(
        "{} stations, {ready} of them predictable",
        catalog.stations.len()
    ));
    if ready == 0 {
        return Err("nothing in the region can be predicted".into());
    }
    Ok(catalog)
}

/// Reference stations named by a subordinate but not in the catalog.
fn missing_references(catalog: &Catalog) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for station in &catalog.stations {
        let Some(reference) = station.reference() else {
            continue;
        };
        if catalog.harmonics(reference).is_none() && !out.iter().any(|k| k == reference) {
            out.push(reference.to_string());
        }
    }
    out.sort();
    out
}

/// The constants for a station we hold nothing else about. `SFB1202-17`
/// names a current bin; anything without a dash is a tide station.
fn reference_constants(key: &str, catalog: &mut Catalog) -> Result<(), String> {
    let (id, bin) = match key.split_once('-') {
        Some((id, bin)) => (id, Some(bin.parse::<u32>().map_err(|_| "not a bin")?)),
        None => (key, None),
    };
    let harcon = http::get_text(&coops::harcon_url(id))?;
    let harmonics = match bin {
        Some(bin) => coops::current_harmonics(&harcon, bin)?.0,
        None => coops::tide_harmonics(&harcon, &http::get_text(&coops::datums_url(id))?)?,
    };
    catalog.insert_harmonics(key.to_string(), harmonics);
    Ok(())
}

fn tide_station(station: &mut Station, catalog: &mut Catalog) -> Result<(), String> {
    // Every tide station has an offsets document; a reference station's
    // simply names no reference.
    let offsets = http::get_text(&coops::tide_offsets_url(&station.id))?;
    coops::apply_tide_offsets(station, &offsets)?;
    if station.source != Source::Harmonic {
        return Ok(());
    }
    let harcon = http::get_text(&coops::harcon_url(&station.id))?;
    let datums = http::get_text(&coops::datums_url(&station.id))?;
    catalog.insert_harmonics(station.key(), coops::tide_harmonics(&harcon, &datums)?);
    Ok(())
}

fn current_station(
    station: &mut Station,
    constants: Option<&str>,
    catalog: &mut Catalog,
) -> Result<(), String> {
    let bin = station.bin.unwrap_or(1);
    // The offsets document carries which way the flood sets, whether or
    // not the station follows another.
    match http::get_text(&coops::current_offsets_url(&station.id, bin)) {
        Ok(text) => coops::apply_current_offsets(station, &text)?,
        // Without it a harmonic station can still be predicted, just
        // with no direction to draw an arrow in.
        Err(_) if station.source == Source::Harmonic => {}
        Err(e) => return Err(e),
    }
    if station.source != Source::Harmonic {
        return Ok(());
    }
    let harcon = constants.ok_or("no harmonic constants")?;
    let (harmonics, azimuth) = coops::current_harmonics(harcon, bin)?;
    // Where NOAA gave no mean flood direction, the major axis of the
    // tidal ellipse is the channel, and the flood runs along it.
    if station.flood_direction.is_none() {
        station.flood_direction = azimuth;
        station.ebb_direction = azimuth.map(|a| (a + 180.0).rem_euclid(360.0));
    }
    catalog.insert_harmonics(station.key(), harmonics);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::predict::Harmonics;
    use crate::station::{HeightRule, Source, TideOffsets};

    #[test]
    fn references_the_catalog_lacks_are_the_ones_it_goes_back_for() {
        let mut c = Catalog::default();
        let mut push = |id: &str, source: Source| {
            c.stations.push(Station {
                id: id.into(),
                bin: None,
                name: id.into(),
                lat: 37.8,
                lon: -122.4,
                kind: Kind::Tide,
                depth: None,
                flood_direction: None,
                ebb_direction: None,
                source,
            });
        };
        let follows = |id: &str| Source::Tide {
            reference: id.into(),
            offsets: TideOffsets {
                high_minutes: 0,
                low_minutes: 0,
                high_height: 1.0,
                low_height: 1.0,
                rule: HeightRule::Ratio,
            },
        };
        push("here", Source::Harmonic);
        push("a", follows("here"));
        push("b", follows("far"));
        push("c", follows("far"));
        c.insert_harmonics("here".into(), Harmonics::default());
        assert_eq!(missing_references(&c), ["far"]);
        c.insert_harmonics("far".into(), Harmonics::default());
        assert!(missing_references(&c).is_empty());
    }

    #[test]
    fn a_report_passes_its_notes_on() {
        let seen = std::cell::RefCell::new(Vec::new());
        let report = Report {
            say: &|m: &str| seen.borrow_mut().push(m.to_string()),
        };
        report.note("one");
        report.note("two");
        assert_eq!(*seen.borrow(), ["one", "two"]);
    }
}
