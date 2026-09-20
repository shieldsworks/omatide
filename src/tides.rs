//! Predicting at a station, whichever sort it is.
//!
//! Everything above this module names a station and a time; whether that
//! station has its own constants or follows another one is settled here.

use crate::predict::{self, Extreme, Harmonics, Point};
use crate::station::{Catalog, Kind, Source, Station};
use crate::subordinate;

/// Centimeters a second to knots.
pub const KNOTS: f64 = 0.019_438_445;

/// The station, and the constants its predictions come from — its own,
/// or its reference's.
fn resolve<'a>(catalog: &'a Catalog, key: &str) -> Result<(&'a Station, &'a Harmonics), String> {
    let station = catalog
        .get(key)
        .ok_or_else(|| format!("no station {key}"))?;
    let constants_key = station.reference().unwrap_or(key);
    let harmonics = catalog
        .harmonics(constants_key)
        .ok_or_else(|| match station.reference() {
            Some(r) => format!("{key} follows {r}, which isn't in the catalog"),
            None => format!("{key} has no harmonic constants"),
        })?;
    Ok((station, harmonics))
}

fn wrong_kind(station: &Station, wanted: Kind) -> Option<String> {
    (station.kind != wanted).then(|| {
        format!(
            "{} is a {} station, not a {} one",
            station.key(),
            station.kind.name(),
            wanted.name()
        )
    })
}

/// The height above chart datum, in meters.
pub fn height(catalog: &Catalog, key: &str, at: i64) -> Result<f64, String> {
    Ok(heights(catalog, key, at, at, 1)?
        .first()
        .ok_or("no prediction")?
        .value)
}

/// The tide curve, every `step` seconds.
pub fn heights(
    catalog: &Catalog,
    key: &str,
    start: i64,
    end: i64,
    step: i64,
) -> Result<Vec<Point>, String> {
    let (station, harmonics) = resolve(catalog, key)?;
    if let Some(e) = wrong_kind(station, Kind::Tide) {
        return Err(e);
    }
    Ok(match &station.source {
        Source::Harmonic => predict::series(harmonics, start, end, step),
        Source::Tide { offsets, .. } => {
            subordinate::tide_series(harmonics, offsets, start, end, step)
        }
        Source::Current { .. } => return Err(format!("{key} has current offsets on a tide")),
    })
}

/// High and low water between two times.
pub fn extremes(
    catalog: &Catalog,
    key: &str,
    start: i64,
    end: i64,
) -> Result<Vec<Extreme>, String> {
    let (station, harmonics) = resolve(catalog, key)?;
    if let Some(e) = wrong_kind(station, Kind::Tide) {
        return Err(e);
    }
    Ok(match &station.source {
        Source::Harmonic => predict::extremes(harmonics, start, end),
        Source::Tide { offsets, .. } => subordinate::tide_extremes(harmonics, offsets, start, end),
        Source::Current { .. } => return Err(format!("{key} has current offsets on a tide")),
    })
}

/// The stream: speed in centimeters a second along the channel, positive
/// on the flood, and the true direction it is setting.
pub fn stream(catalog: &Catalog, key: &str, at: i64) -> Result<(f64, Option<f64>), String> {
    let speed = streams(catalog, key, at, at, 1)?
        .first()
        .ok_or("no prediction")?
        .value;
    Ok((speed, set(catalog.get(key), speed)))
}

/// Which way a stream of this sign is setting, if the station says.
pub fn set(station: Option<&Station>, speed: f64) -> Option<f64> {
    let station = station?;
    if speed >= 0.0 {
        station.flood_direction
    } else {
        station.ebb_direction
    }
}

pub fn streams(
    catalog: &Catalog,
    key: &str,
    start: i64,
    end: i64,
    step: i64,
) -> Result<Vec<Point>, String> {
    let (station, harmonics) = resolve(catalog, key)?;
    if let Some(e) = wrong_kind(station, Kind::Current) {
        return Err(e);
    }
    Ok(match &station.source {
        Source::Harmonic => predict::series(harmonics, start, end, step),
        Source::Current { offsets, .. } => {
            subordinate::current_series(harmonics, offsets, start, end, step)
        }
        Source::Tide { .. } => return Err(format!("{key} has tide offsets on a current")),
    })
}

/// Slack water and maximum flood and ebb between two times.
///
/// A maximum under a fiftieth of a knot between two slacks is dropped:
/// the stream never really turned, and printing it would make a sailor
/// wait for a change that isn't coming.
pub fn current_turns(
    catalog: &Catalog,
    key: &str,
    start: i64,
    end: i64,
) -> Result<Vec<Extreme>, String> {
    let (station, harmonics) = resolve(catalog, key)?;
    if let Some(e) = wrong_kind(station, Kind::Current) {
        return Err(e);
    }
    let turns = match &station.source {
        Source::Harmonic => predict::current_turns(harmonics, start, end),
        Source::Current { offsets, .. } => {
            subordinate::current_turns(harmonics, offsets, start, end)
        }
        Source::Tide { .. } => return Err(format!("{key} has tide offsets on a current")),
    };
    Ok(predict::drop_weak_turns(turns, 1.0))
}

/// The turn before and the turn after a moment, which is what a bar
/// widget has room for.
pub fn around(turns: &[Extreme], at: i64) -> (Option<&Extreme>, Option<&Extreme>) {
    let next = turns.iter().find(|e| e.time > at);
    let last = turns.iter().rev().find(|e| e.time <= at);
    (last, next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constituent::index_of;
    use crate::station::{HeightRule, Source, TideOffsets};
    use crate::time::unix;

    fn catalog() -> Catalog {
        let mut c = Catalog::default();
        let mut push = |id: &str, kind: Kind, source: Source| {
            c.stations.push(Station {
                id: id.into(),
                bin: None,
                name: id.into(),
                lat: 37.8,
                lon: -122.4,
                kind,
                depth: None,
                flood_direction: Some(70.0),
                ebb_direction: Some(266.0),
                source,
            });
        };
        push("ref", Kind::Tide, Source::Harmonic);
        push(
            "sub",
            Kind::Tide,
            Source::Tide {
                reference: "ref".into(),
                offsets: TideOffsets {
                    high_minutes: 10,
                    low_minutes: 14,
                    high_height: 0.97,
                    low_height: 1.0,
                    rule: HeightRule::Ratio,
                },
            },
        );
        push("stream", Kind::Current, Source::Harmonic);
        push("orphan", Kind::Tide, Source::Harmonic);
        let mut h = Harmonics::default();
        h.amplitude[index_of("M2").unwrap()] = 0.576;
        h.phase[index_of("M2").unwrap()] = 208.2;
        h.offset = 0.951;
        c.insert_harmonics("ref".into(), h.clone());
        h.offset = 0.0;
        c.insert_harmonics("stream".into(), h);
        c
    }

    #[test]
    fn a_subordinate_station_predicts_through_its_reference() {
        let c = catalog();
        let t = unix(2026, 9, 20, 12, 0, 0);
        assert!(height(&c, "ref", t).is_ok());
        assert!(height(&c, "sub", t).is_ok());
        let theirs = extremes(&c, "ref", t, t + 86_400).unwrap();
        let ours = extremes(&c, "sub", t, t + 86_400).unwrap();
        assert!(!ours.is_empty());
        assert_ne!(theirs[0].time, ours[0].time);
    }

    #[test]
    fn asking_the_wrong_kind_of_question_is_an_error() {
        let c = catalog();
        let t = unix(2026, 9, 20, 12, 0, 0);
        assert!(stream(&c, "ref", t).unwrap_err().contains("not a current"));
        assert!(height(&c, "stream", t).unwrap_err().contains("not a tide"));
        assert!(height(&c, "nowhere", t).unwrap_err().contains("no station"));
        assert!(
            height(&c, "orphan", t)
                .unwrap_err()
                .contains("no harmonic constants")
        );
    }

    #[test]
    fn a_stream_sets_the_way_it_is_running() {
        let c = catalog();
        let t = unix(2026, 9, 20, 12, 0, 0);
        let points = streams(&c, "stream", t, t + 43_200, 600).unwrap();
        let flood = points.iter().find(|p| p.value > 0.0).unwrap();
        let ebb = points.iter().find(|p| p.value < 0.0).unwrap();
        assert_eq!(stream(&c, "stream", flood.time).unwrap().1, Some(70.0));
        assert_eq!(stream(&c, "stream", ebb.time).unwrap().1, Some(266.0));
        assert_eq!(set(None, 1.0), None);
    }

    #[test]
    fn the_turns_around_a_moment_are_the_ones_either_side() {
        let c = catalog();
        let t = unix(2026, 9, 20, 0, 0, 0);
        let turns = extremes(&c, "ref", t, t + 2 * 86_400).unwrap();
        let middle = turns[2].time;
        let (last, next) = around(&turns, middle + 60);
        assert_eq!(last.unwrap().time, middle);
        assert_eq!(next.unwrap().time, turns[3].time);
        // Exactly on a turn, that turn has just passed.
        let (last, _) = around(&turns, middle);
        assert_eq!(last.unwrap().time, middle);
        assert!(around(&turns, t - 86_400).0.is_none());
        assert!(around(&turns, t + 10 * 86_400).1.is_none());
        assert_eq!(around(&[], t), (None, None));
    }
}
