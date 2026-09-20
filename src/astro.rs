//! The astronomy behind a tide: where the moon and sun are, and how the
//! moon's orbit tilts over its 18.6-year nodal cycle.
//!
//! The formulas are Schureman's (*Manual of Harmonic Analysis and
//! Prediction of Tides*, Special Publication 98, 1940), which is still
//! what NOAA's tide tables are built on.
//!
//! One trap is worth naming, because it costs 24 cm of tide. Schureman's
//! epoch is usually written "Julian Day 2415020.0", but his constants are
//! for **1900 January 1 at 0h** — JD 2415020.5. Half a day is 6.6° of
//! lunar motion, and every lunar constituent comes out of phase by that
//! much times its coefficient on `s`.

use std::f64::consts::PI;

const DEG: f64 = PI / 180.0;

/// Obliquity of the ecliptic and the inclination of the moon's orbit to
/// it, in degrees. Schureman, table 1.
const OBLIQUITY: f64 = 23.4522;
const LUNAR_INCLINATION: f64 = 5.1452;

/// The mean longitudes a tidal argument is built from, in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Longitudes {
    /// Moon.
    pub s: f64,
    /// Sun.
    pub h: f64,
    /// Lunar perigee.
    pub p: f64,
    /// Moon's ascending node.
    pub n: f64,
    /// Solar perigee.
    pub p1: f64,
}

/// Julian Day for a Unix time, counting from noon as Julian Days do.
pub fn julian_day(unix: i64) -> f64 {
    unix as f64 / 86_400.0 + 2_440_587.5
}

impl Longitudes {
    /// Schureman's mean longitudes at a Unix time.
    pub fn at(unix: i64) -> Longitudes {
        // Julian centuries from 1900 January 1, 0h UT. See the note above:
        // this is JD 2415020.5, not the 2415020.0 the epoch is often given
        // as.
        let t = (julian_day(unix) - 2_415_020.5) / 36_525.0;
        let (t2, t3) = (t * t, t * t * t);
        Longitudes {
            s: wrap(277.0248 + 481267.8906 * t + 0.0020 * t2 + 0.000002 * t3),
            h: wrap(280.1895 + 36000.7689 * t + 0.000303 * t2),
            p: wrap(334.3853 + 4069.0340 * t - 0.010325 * t2 - 0.0000125 * t3),
            n: wrap(259.1568 - 1934.1420 * t + 0.002078 * t2 + 0.0000022 * t3),
            p1: wrap(281.2209 + 1.7192 * t + 0.000453 * t2 + 0.000003 * t3),
        }
    }

    /// Mean lunar time at Greenwich, in degrees: the hour angle of the
    /// mean sun plus the sun's longitude less the moon's.
    pub fn tau(&self, unix: i64) -> f64 {
        let seconds = unix.rem_euclid(86_400) as f64;
        15.0 * seconds / 3600.0 + self.h - self.s
    }
}

/// The angles that describe the moon's orbit at one moment, from which
/// every constituent's node factor and nodal phase follows. Schureman
/// equations 71 to 79.
#[derive(Clone, Copy, Debug)]
pub struct Nodal {
    /// Inclination of the moon's orbit to the equator, radians.
    pub i: f64,
    /// Longitude in the moon's orbit of its intersection with the equator.
    pub xi: f64,
    /// Right ascension of that same intersection.
    pub nu: f64,
    /// The K1 and K2 corrections, ν′ and 2ν″.
    pub nu_prime: f64,
    pub nu_two_prime: f64,
}

impl Nodal {
    pub fn at(longitudes: &Longitudes) -> Nodal {
        let n = longitudes.n * DEG;
        let (inc, obl) = (LUNAR_INCLINATION * DEG, OBLIQUITY * DEG);
        let i = (inc.cos() * obl.cos() - inc.sin() * obl.sin() * n.cos()).acos();

        // Schureman writes these as arctangents of a multiple of
        // tan(N/2), which throws away the half-turn when N/2 leaves the
        // first quadrant. atan2 on (k·sin, cos) keeps it.
        let (sin_half, cos_half) = ((n / 2.0).sin(), (n / 2.0).cos());
        let e1 = (((obl - inc) / 2.0).cos() / ((obl + inc) / 2.0).cos() * sin_half).atan2(cos_half);
        let e2 = (((obl - inc) / 2.0).sin() / ((obl + inc) / 2.0).sin() * sin_half).atan2(cos_half);
        let xi = n - (e1 + e2);
        let nu = e1 - e2;

        let nu_prime = ((2.0 * i).sin() * nu.sin()).atan2((2.0 * i).sin() * nu.cos() + 0.3347);
        let sin_sq = i.sin() * i.sin();
        let nu_two_prime =
            0.5 * (sin_sq * (2.0 * nu).sin()).atan2(sin_sq * (2.0 * nu).cos() + 0.0727);
        Nodal {
            i,
            xi,
            nu,
            nu_prime,
            nu_two_prime,
        }
    }
}

/// Degrees, folded into 0..360.
pub fn wrap(degrees: f64) -> f64 {
    degrees.rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::unix;

    #[test]
    fn julian_day_matches_its_known_epochs() {
        assert!((julian_day(0) - 2_440_587.5).abs() < 1e-9);
        // 2000 January 1, 12h UT is the standard J2000.0 epoch.
        assert!((julian_day(unix(2000, 1, 1, 12, 0, 0)) - 2_451_545.0).abs() < 1e-6);
        // Schureman's own epoch.
        assert!((julian_day(unix(1900, 1, 1, 0, 0, 0)) - 2_415_020.5).abs() < 1e-6);
    }

    #[test]
    fn mean_longitudes_agree_with_modern_astronomy() {
        // Schureman's series are Newcomb's; a modern ephemeris puts the
        // moon and sun within a few hundredths of a degree of them. If
        // the epoch were wrong by half a day the moon would be out by
        // 6.6 degrees, which is what this guards.
        for t in [
            unix(1900, 1, 1, 0, 0, 0),
            unix(2000, 1, 1, 12, 0, 0),
            unix(2026, 9, 20, 0, 0, 0),
        ] {
            let l = Longitudes::at(t);
            let c = (julian_day(t) - 2_451_545.0) / 36_525.0;
            let moon = wrap(218.3164477 + 481267.88123421 * c - 0.0015786 * c * c);
            let sun = wrap(280.46646 + 36000.76983 * c + 0.0003032 * c * c);
            assert!((diff(l.s, moon)).abs() < 0.05, "moon {} vs {moon}", l.s);
            assert!((diff(l.h, sun)).abs() < 0.05, "sun {} vs {sun}", l.h);
        }
    }

    #[test]
    fn the_node_swings_through_its_whole_cycle() {
        // I runs between obliquity minus and plus the lunar inclination.
        let mut low = f64::MAX;
        let mut high = f64::MIN;
        for year in 2019..2038 {
            let l = Longitudes::at(unix(year, 7, 2, 12, 0, 0));
            let i = Nodal::at(&l).i / DEG;
            low = low.min(i);
            high = high.max(i);
        }
        assert!((low - (OBLIQUITY - LUNAR_INCLINATION)).abs() < 0.1, "{low}");
        assert!(
            (high - (OBLIQUITY + LUNAR_INCLINATION)).abs() < 0.1,
            "{high}"
        );
    }

    fn diff(a: f64, b: f64) -> f64 {
        (a - b + 180.0).rem_euclid(360.0) - 180.0
    }
}
