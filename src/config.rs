//! `~/.config/omatide/config.toml`: the waters to carry stations for,
//! home when there is no GPS, and how deep in the water column to read
//! the stream.

use crate::coops::Region;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub region: Region,
    pub home: (f64, f64),
    /// Meters below the surface. A current station is measured in bins
    /// down the water column and they differ: the stream near the bottom
    /// runs slower than the stream at the surface, and sometimes not
    /// quite the same way. A boat feels the top of it, so this is
    /// shallow by default — the keel of a Pacific Seacraft 25 draws less
    /// than a meter and a half.
    pub depth: f64,
}

impl Default for Settings {
    /// The Bay, and home at the Berkeley Marina, where omahelm opens.
    fn default() -> Self {
        Settings {
            region: Region::BAY,
            home: (37.8663, -122.3148),
            depth: 3.0,
        }
    }
}

impl Settings {
    /// Unknown keys and bad values are reported and left at their
    /// defaults, so a typo never stops the tide.
    pub fn parse(text: &str) -> (Settings, Vec<String>) {
        let mut s = Settings::default();
        let mut problems = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('[') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                problems.push(format!("line {}: expected key = value", n + 1));
                continue;
            };
            let (k, v) = (k.trim(), v.trim().trim_matches('"'));
            match k {
                "region" => match Region::parse(v) {
                    Some(r) => s.region = r,
                    None => {
                        problems.push("region: expected south, west, north, east in degrees".into())
                    }
                },
                "home" => match v.split_once(',').and_then(|(a, b)| {
                    let (lat, lon) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
                    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon))
                        .then_some((lat, lon))
                }) {
                    Some(home) => s.home = home,
                    None => problems.push("home: expected latitude, longitude in degrees".into()),
                },
                "depth" => match v.parse::<f64>() {
                    Ok(d) if (0.0..=200.0).contains(&d) => s.depth = d,
                    _ => problems.push("depth: expected meters, 0 to 200".into()),
                },
                other => problems.push(format!("unknown setting {other}")),
            }
        }
        (s, problems)
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("OMATIDE_CONFIG") {
        return PathBuf::from(p);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home_dir().join(".config"));
    base.join("omatide/config.toml")
}

/// The settings file, or the defaults when there isn't one.
pub fn load(path: &std::path::Path) -> (Settings, Vec<String>) {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let (s, p) = Settings::parse(&text);
            (
                s,
                p.into_iter().map(|p| format!("config.toml: {p}")).collect(),
            )
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Settings::default(), Vec::new()),
        Err(e) => (Settings::default(), vec![format!("config.toml: {e}")]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_region_home_and_depth() {
        let (s, p) = Settings::parse(
            "# Puget Sound\nregion = 47.0, -123.5, 48.8, -122.0\n\
             home = \"47.68, -122.41\"\ndepth = 5\n",
        );
        assert!(p.is_empty(), "{p:?}");
        assert_eq!(
            s.region,
            Region {
                south: 47.0,
                west: -123.5,
                north: 48.8,
                east: -122.0
            }
        );
        assert_eq!(s.home, (47.68, -122.41));
        assert_eq!(s.depth, 5.0);
    }

    #[test]
    fn reports_bad_settings_and_keeps_the_defaults() {
        let (s, p) =
            Settings::parse("region = 38, -123\nhome = north\ndepth = -2\ntide = high\nnonsense\n");
        assert_eq!(s, Settings::default());
        assert_eq!(p.len(), 5, "{p:?}");
        assert!(p[0].contains("region"));
        assert!(p[1].contains("home"));
        assert!(p[2].contains("depth"));
        assert_eq!(p[3], "unknown setting tide");
        assert!(p[4].contains("line 5"));
    }

    #[test]
    fn home_is_the_berkeley_marina_by_default() {
        let s = Settings::default();
        assert!(s.region.contains(s.home.0, s.home.1));
    }
}
