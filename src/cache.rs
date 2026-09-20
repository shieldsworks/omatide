//! Where omatide keeps what it has learned from NOAA, so that it works
//! at sea.
//!
//! The whole of San Francisco Bay — every station, every offset, every
//! constant — is a few hundred kilobytes, so it lives in one file that is
//! read at start-up and written whole. Nothing else is needed at sea: the
//! harmonic constants are good for years, and a tide is predicted, not
//! fetched.

use crate::station::Catalog;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// `~/.local/share/omatide`, or `$XDG_DATA_HOME/omatide`, or wherever
/// `OMATIDE_DATA_DIR` says.
pub fn data_dir() -> PathBuf {
    chosen_dir(
        std::env::var_os("OMATIDE_DATA_DIR").map(PathBuf::from),
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

/// The choice itself, so it can be tested without touching the
/// environment of a process running tests on several threads.
fn chosen_dir(over: Option<PathBuf>, xdg: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = over {
        return dir;
    }
    xdg.filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| h.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omatide")
}

pub fn catalog_path(dir: &Path) -> PathBuf {
    dir.join("stations.json")
}

pub fn load(dir: &Path) -> Result<Catalog, String> {
    let path = catalog_path(dir);
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Catalog::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes the catalog through a temporary file, so a run cut short
/// leaves the old one whole rather than half of a new one.
pub fn save(dir: &Path, catalog: &Catalog) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = catalog_path(dir);
    let temporary = path.with_extension("json.new");
    let write = || -> std::io::Result<()> {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(catalog.to_json().as_bytes())?;
        file.sync_all()
    };
    write().map_err(|e| format!("{}: {e}", temporary.display()))?;
    fs::rename(&temporary, &path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::station::{Kind, Source, Station};

    fn temporary_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("omatide-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_catalog_written_is_a_catalog_read_back() {
        let dir = temporary_dir("cache");
        let mut catalog = Catalog::default();
        catalog.stations.push(Station {
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
        assert!(load(&dir).is_err(), "nothing cached yet");
        save(&dir, &catalog).unwrap();
        assert_eq!(load(&dir).unwrap().stations, catalog.stations);
        // Saving again over the top leaves no leftovers behind.
        save(&dir, &catalog).unwrap();
        let left: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["stations.json"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_cache_is_an_error_not_an_empty_bay() {
        let dir = temporary_dir("damaged");
        fs::create_dir_all(&dir).unwrap();
        fs::write(catalog_path(&dir), "{ not json").unwrap();
        assert!(load(&dir).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_data_directory_follows_the_environment() {
        let p = |s: &str| Some(PathBuf::from(s));
        assert_eq!(
            chosen_dir(p("/tmp/over"), p("/tmp/xdg"), p("/home/c")),
            PathBuf::from("/tmp/over")
        );
        assert_eq!(
            chosen_dir(None, p("/tmp/xdg"), p("/home/c")),
            PathBuf::from("/tmp/xdg/omatide")
        );
        // A relative XDG_DATA_HOME is not one, by the spec.
        assert_eq!(
            chosen_dir(None, p("relative"), p("/home/c")),
            PathBuf::from("/home/c/.local/share/omatide")
        );
        assert_eq!(chosen_dir(None, None, None), PathBuf::from("./omatide"));
        assert!(data_dir().ends_with("omatide") || std::env::var_os("OMATIDE_DATA_DIR").is_some());
    }
}
