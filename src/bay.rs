//! San Francisco Bay.
//!
//! The bay is not one tide. High water at the Golden Gate reaches Port
//! Chicago an hour and three quarters later and a third of a meter
//! lower, and the stream that is slack off Alcatraz is still ebbing hard
//! through Raccoon Strait. NOAA measured all of it — a hundred and
//! thirty-one harmonic current bins and a hundred and twenty-six
//! subordinate ones inside the bay alone — and this module is the list
//! of the places a sailor actually names, pointed at the stations that
//! describe them.
//!
//! The order runs the way you sail it: in from the sea, up the bay to
//! the delta, then back down the east shore to the south bay.

use crate::station::{Catalog, Kind};

/// A place on the bay, and the station that speaks for it.
pub struct Place {
    pub name: &'static str,
    /// The NOAA station, without a bin: the bin is chosen by depth.
    pub station: &'static str,
    /// What a sailor should know about the stream here.
    pub note: &'static str,
}

const fn p(name: &'static str, station: &'static str, note: &'static str) -> Place {
    Place {
        name,
        station,
        note,
    }
}

/// The bay's narrows and reaches, where the stream is worth knowing.
pub const PLACES: &[Place] = &[
    p(
        "Point Bonita",
        "SFB1220",
        "the ebb sets out over the bar; the potato patch breaks north of it",
    ),
    p(
        "Golden Gate",
        "SFB1202",
        "the strongest stream in the bay, and the reference the rest follow",
    ),
    p(
        "Fort Point",
        "PCT0261",
        "a back eddy floods along the south shore while the channel still ebbs",
    ),
    p(
        "Alcatraz, south",
        "SFB1204",
        "where the Gate's stream splits for the south bay and the estuary",
    ),
    p("Alcatraz, north", "SFB1211", "the road to Raccoon Strait"),
    p(
        "Raccoon Strait",
        "SFB1212",
        "runs hard between Angel Island and Tiburon, and turns before the Gate",
    ),
    p(
        "Point Stuart",
        "PCT0606",
        "the west end of Raccoon Strait, off Angel Island",
    ),
    p(
        "Point Blunt",
        "PCT0561",
        "the southeast corner of Angel Island, with a rip on the ebb",
    ),
    p(
        "Southampton Shoal",
        "s08010",
        "the channel east of Angel Island",
    ),
    p(
        "Point Chauncey",
        "SFB1309",
        "the way into San Pablo Bay, west of the Brothers",
    ),
    p("Richmond", "PCT0666", "off the harbor entrance"),
    p(
        "Red Rock",
        "PCT0671",
        "under the Richmond bridge, where San Pablo Bay narrows",
    ),
    p(
        "Point San Pablo",
        "SFB1312",
        "midchannel into San Pablo Bay",
    ),
    p(
        "Pinole Shoal",
        "SFB1315",
        "the dredged channel across the shallows",
    ),
    p(
        "Carquinez Strait",
        "SFB1319",
        "the river's own current adds to the ebb all year",
    ),
    p("Benicia Bridge", "s06010", "the head of the strait"),
    p("Treasure Island", "SFB1210", "the north side of the island"),
    p(
        "Yerba Buena Island",
        "SFB1209",
        "midchannel, under the east span",
    ),
    p("Bay Bridge", "SFB1208", "between the western spans"),
    p("Hunters Point", "SFB1308", "the south bay above the shoals"),
    p("San Mateo Bridge", "SFB1305", "the south bay narrows"),
    p("Dumbarton Bridge", "SFB1301", "the head of the south bay"),
];

/// Tide stations that show the wave marching up the bay: high water
/// takes the best part of two hours to travel from the Gate to Suisun.
pub const TIDE_PLACES: &[Place] = &[
    p("Golden Gate", "9414290", "the bay's reference station"),
    p("Sausalito", "9414806", "ten minutes behind the Gate"),
    p("Alcatraz", "9414792", ""),
    p("Berkeley", "9414816", "the marina Dash lies in"),
    p("Alameda", "9414750", ""),
    p("Richmond", "9414863", ""),
    p("Point Chauncey", "9414837", ""),
    p("Pinole Point", "9415056", ""),
    p("Martinez", "9415102", ""),
    p(
        "Port Chicago",
        "9415144",
        "an hour and three quarters behind the Gate",
    ),
    p("Rio Vista", "9415316", "up the Sacramento, and river-fed"),
    p(
        "Redwood City",
        "9414523",
        "the south bay's range is the biggest",
    ),
    p("Dumbarton Bridge", "9414509", ""),
];

/// The station key for a place: the bin nearest the depth wanted.
///
/// A current station is measured in bins down the water column, and they
/// are not the same stream — near the bottom it runs slower, and in a
/// strait it can lag. A boat feels the top of it.
pub fn key_for(catalog: &Catalog, id: &str, depth: f64) -> Option<String> {
    let mut best: Option<(f64, String)> = None;
    for s in &catalog.stations {
        if s.id != id || !catalog.is_ready(&s.key()) {
            continue;
        }
        // An unmeasured depth sorts after every measured one.
        let distance = s.depth.map_or(f64::MAX / 2.0, |d| (d - depth).abs());
        if best.as_ref().is_none_or(|(d, _)| distance < *d) {
            best = Some((distance, s.key()));
        }
    }
    best.map(|(_, key)| key)
}

/// Every place that can be predicted, with the station standing in for
/// it. Places NOAA has no station for are simply left out.
pub fn resolve(
    catalog: &Catalog,
    places: &'static [Place],
    depth: f64,
) -> Vec<(&'static Place, String)> {
    places
        .iter()
        .filter_map(|place| Some((place, key_for(catalog, place.station, depth)?)))
        .collect()
}

/// How many of the bay's named places the catalog can speak for.
pub fn coverage(catalog: &Catalog, depth: f64) -> (usize, usize) {
    (
        resolve(catalog, PLACES, depth).len() + resolve(catalog, TIDE_PLACES, depth).len(),
        PLACES.len() + TIDE_PLACES.len(),
    )
}

/// Every station of a kind that can be predicted, for drawing the whole
/// bay rather than only its named places.
pub fn all(catalog: &Catalog, kind: Kind, depth: f64) -> Vec<String> {
    let mut ids: Vec<&str> = Vec::new();
    for s in &catalog.stations {
        if s.kind == kind && !ids.contains(&s.id.as_str()) {
            ids.push(&s.id);
        }
    }
    ids.into_iter()
        .filter_map(|id| key_for(catalog, id, depth))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::predict::Harmonics;
    use crate::station::{Source, Station};

    fn catalog() -> Catalog {
        let mut c = Catalog::default();
        for (id, bin, depth) in [
            ("SFB1212", Some(1), Some(21.6)),
            ("SFB1212", Some(6), Some(11.6)),
            ("SFB1212", Some(9), Some(5.8)),
            ("PCT0671", Some(1), Some(3.4)),
            ("SFB9999", Some(1), None),
        ] {
            c.stations.push(Station {
                id: id.into(),
                bin,
                name: id.into(),
                lat: 37.87,
                lon: -122.44,
                kind: Kind::Current,
                depth,
                flood_direction: None,
                ebb_direction: None,
                source: Source::Harmonic,
            });
        }
        let mut h = Harmonics::default();
        h.amplitude[0] = 70.0;
        for key in ["SFB1212-1", "SFB1212-6", "SFB1212-9", "PCT0671-1"] {
            c.insert_harmonics(key.into(), h.clone());
        }
        c
    }

    #[test]
    fn the_bin_chosen_is_the_one_nearest_the_depth_wanted() {
        let c = catalog();
        assert_eq!(key_for(&c, "SFB1212", 3.0).as_deref(), Some("SFB1212-9"));
        assert_eq!(key_for(&c, "SFB1212", 12.0).as_deref(), Some("SFB1212-6"));
        assert_eq!(key_for(&c, "SFB1212", 30.0).as_deref(), Some("SFB1212-1"));
        // A station with no constants is not offered at all.
        assert_eq!(key_for(&c, "SFB9999", 3.0), None);
        assert_eq!(key_for(&c, "nowhere", 3.0), None);
    }

    #[test]
    fn places_that_have_no_station_are_left_out() {
        let c = catalog();
        let found = resolve(&c, PLACES, 3.0);
        // Only Raccoon Strait and Red Rock are in this little catalog.
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0.name, "Raccoon Strait");
        assert_eq!(found[0].1, "SFB1212-9");
        assert_eq!(found[1].0.name, "Red Rock");
        let (have, total) = coverage(&c, 3.0);
        assert_eq!(have, 2);
        assert_eq!(total, PLACES.len() + TIDE_PLACES.len());
    }

    #[test]
    fn every_place_names_a_station_and_reads_as_english() {
        for place in PLACES.iter().chain(TIDE_PLACES) {
            assert!(!place.name.is_empty());
            assert!(!place.station.is_empty(), "{}", place.name);
            assert!(
                !place.station.contains('-'),
                "{}: bins are chosen, not named",
                place.name
            );
            assert!(place.name.len() < 30, "{}", place.name);
        }
        // Each place appears once in its own list.
        for list in [PLACES, TIDE_PLACES] {
            for (i, a) in list.iter().enumerate() {
                assert!(
                    !list[..i].iter().any(|b| b.station == a.station),
                    "{} twice",
                    a.station
                );
            }
        }
    }

    #[test]
    fn the_whole_bay_is_one_bin_per_station() {
        let c = catalog();
        let keys = all(&c, Kind::Current, 3.0);
        assert_eq!(keys, ["SFB1212-9", "PCT0671-1"]);
        assert!(all(&c, Kind::Tide, 3.0).is_empty());
    }
}
