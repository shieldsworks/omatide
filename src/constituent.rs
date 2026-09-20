//! The 37 harmonic constituents NOAA publishes for a station, and the
//! node factor and nodal phase each one takes.
//!
//! A constituent is a cosine whose argument is a whole-number combination
//! of six slowly turning angles (Doodson's numbers). Its amplitude and
//! phase at a station come from NOAA; what we work out here is where in
//! its cycle it stands at a given moment, and the nodal correction that
//! rides on the moon's 18.6-year wobble.
//!
//! Every formula below was checked against NOAA's own published
//! predictions for San Francisco over seven years spread across the
//! nodal cycle — see `tests/predict.rs`. Three of them are not the
//! textbook shape, and are marked where they appear.

use crate::astro::{Longitudes, Nodal, wrap};
use std::f64::consts::PI;

const DEG: f64 = PI / 180.0;

/// How a constituent's node factor and nodal phase are worked out.
/// Shallow-water constituents take the product of their parents' factors
/// and the sum of their phases, which is what the compound kinds mean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Solar: no nodal correction at all.
    Fixed,
    M2,
    M2Squared,
    M2Cubed,
    M2Fourth,
    O1,
    K1,
    K2,
    /// M2 × K1, for MK3.
    M2K1,
    /// M2² ÷ K1, for 2MK3.
    M2SquaredOverK1,
    J1,
    Oo1,
    Mm,
    Mf,
    M3,
    M1,
    L2,
}

/// One of the 37, as NOAA names it.
#[derive(Clone, Copy, Debug)]
pub struct Constituent {
    pub name: &'static str,
    /// Coefficients on mean lunar time, then the moon, sun, lunar
    /// perigee, node and solar perigee.
    pub doodson: [i8; 6],
    /// The constant added to the argument, in degrees.
    pub phase: f64,
    pub kind: Kind,
}

impl Constituent {
    /// Degrees per hour, which is how NOAA lists a constituent's speed.
    pub fn speed(&self) -> f64 {
        const RATES: [f64; 6] = [
            14.492_052_11,
            0.549_016_53,
            0.041_068_64,
            0.004_641_83,
            -0.002_206_41,
            0.000_001_96,
        ];
        self.doodson
            .iter()
            .zip(RATES)
            .map(|(n, rate)| f64::from(*n) * rate)
            .sum()
    }

    /// The equilibrium argument V₀ at a moment, in degrees, before the
    /// nodal phase is added.
    pub fn argument(&self, unix: i64, l: &Longitudes) -> f64 {
        let n = self.doodson;
        let terms = [l.tau(unix), l.s, l.h, l.p, l.n, l.p1];
        let sum: f64 = n
            .iter()
            .zip(terms)
            .map(|(c, angle)| f64::from(*c) * angle)
            .sum();
        wrap(sum + self.phase)
    }
}

/// The node factor `f` and nodal phase `u` (radians) for one kind.
pub fn node_factor(kind: Kind, nodal: &Nodal, l: &Longitudes) -> (f64, f64) {
    let Nodal {
        i,
        xi,
        nu,
        nu_prime,
        nu_two_prime,
    } = *nodal;
    let half = (i / 2.0).cos();
    let f_m2 = half.powi(4) / 0.9154;
    let u_m2 = 2.0 * xi - 2.0 * nu;
    let f_o1 = i.sin() * half * half / 0.3800;
    let u_o1 = 2.0 * xi - nu;
    let f_k1 =
        (0.8965 * (2.0 * i).sin().powi(2) + 0.6001 * (2.0 * i).sin() * nu.cos() + 0.1006).sqrt();
    let u_k1 = -nu_prime;
    let sin_sq = i.sin() * i.sin();
    let f_k2 = (19.0444 * sin_sq * sin_sq + 2.7702 * sin_sq * (2.0 * nu).cos() + 0.0981).sqrt();
    let u_k2 = -2.0 * nu_two_prime;

    match kind {
        Kind::Fixed => (1.0, 0.0),
        Kind::M2 => (f_m2, u_m2),
        Kind::M2Squared => (f_m2 * f_m2, 2.0 * u_m2),
        Kind::M2Cubed => (f_m2.powi(3), 3.0 * u_m2),
        Kind::M2Fourth => (f_m2.powi(4), 4.0 * u_m2),
        Kind::O1 => (f_o1, u_o1),
        Kind::K1 => (f_k1, u_k1),
        Kind::K2 => (f_k2, u_k2),
        Kind::M2K1 => (f_m2 * f_k1, u_m2 + u_k1),
        // 2MK3 is M2 twice against K1, so K1's nodal phase comes off
        // rather than on. Confirmed against NOAA over seven years.
        Kind::M2SquaredOverK1 => (f_m2 * f_m2 * f_k1, 2.0 * u_m2 - u_k1),
        Kind::J1 => ((2.0 * i).sin() / 0.7214, -nu),
        Kind::Oo1 => (i.sin() * (i / 2.0).sin().powi(2) / 0.0164, -2.0 * xi - nu),
        Kind::Mm => ((2.0 / 3.0 - sin_sq) / 0.5021, 0.0),
        Kind::Mf => (sin_sq / 0.1578, -2.0 * xi),
        Kind::M3 => (half.powi(6) / 0.8758, 1.5 * u_m2),
        Kind::M1 => {
            // f is Schureman's: f(O1) times Qa. The phase is not — his
            // ξ − ν + Q is out by up to 50° against NOAA's tables. What
            // NOAA uses is −ν − Q with a fixed 20.36° offset, which
            // holds to 0.04° over the whole nodal cycle.
            let p = l.p * DEG - xi;
            let qa = (2.310 + 1.435 * (2.0 * p).cos()).sqrt();
            let q = (0.5 * (2.0 * p).sin()).atan2(1.4353 + 0.5 * (2.0 * p).cos());
            (f_o1 * qa, -nu - q + 20.36 * DEG)
        }
        Kind::L2 => {
            // Schureman divides f(M2) by R; NOAA multiplies. The phase
            // is his.
            let p = l.p * DEG - xi;
            let tan_half = (i / 2.0).tan();
            let r_amp =
                (1.0 - 12.0 * tan_half.powi(2) * (2.0 * p).cos() + 36.0 * tan_half.powi(4)).sqrt();
            let r_phase = (2.0 * p)
                .sin()
                .atan2(1.0 / (6.0 * tan_half * tan_half) - (2.0 * p).cos());
            (f_m2 * r_amp, u_m2 - r_phase)
        }
    }
}

/// NOAA's 37, in the order its harmonic-constants service lists them.
pub const ALL: [Constituent; 37] = [
    c("M2", [2, 0, 0, 0, 0, 0], 0.0, Kind::M2),
    c("S2", [2, 2, -2, 0, 0, 0], 0.0, Kind::Fixed),
    c("N2", [2, -1, 0, 1, 0, 0], 0.0, Kind::M2),
    c("K1", [1, 1, 0, 0, 0, 0], 90.0, Kind::K1),
    c("M4", [4, 0, 0, 0, 0, 0], 0.0, Kind::M2Squared),
    c("O1", [1, -1, 0, 0, 0, 0], -90.0, Kind::O1),
    c("M6", [6, 0, 0, 0, 0, 0], 0.0, Kind::M2Cubed),
    c("MK3", [3, 1, 0, 0, 0, 0], 90.0, Kind::M2K1),
    c("S4", [4, 4, -4, 0, 0, 0], 0.0, Kind::Fixed),
    c("MN4", [4, -1, 0, 1, 0, 0], 0.0, Kind::M2Squared),
    c("NU2", [2, -1, 2, -1, 0, 0], 0.0, Kind::M2),
    c("S6", [6, 6, -6, 0, 0, 0], 0.0, Kind::Fixed),
    c("MU2", [2, -2, 2, 0, 0, 0], 0.0, Kind::M2),
    c("2N2", [2, -2, 0, 2, 0, 0], 0.0, Kind::M2),
    c("OO1", [1, 3, 0, 0, 0, 0], 90.0, Kind::Oo1),
    c("LAM2", [2, 1, -2, 1, 0, 0], 180.0, Kind::M2),
    // S1 and M3 both carry a half turn NOAA's tables imply and the
    // textbook tables leave out.
    c("S1", [1, 1, -1, 0, 0, 0], 180.0, Kind::Fixed),
    c("M1", [1, 0, 0, 1, 0, 0], 90.0, Kind::M1),
    c("J1", [1, 2, 0, -1, 0, 0], 90.0, Kind::J1),
    c("MM", [0, 1, 0, -1, 0, 0], 0.0, Kind::Mm),
    c("SSA", [0, 0, 2, 0, 0, 0], 0.0, Kind::Fixed),
    c("SA", [0, 0, 1, 0, 0, 0], 0.0, Kind::Fixed),
    c("MSF", [0, 2, -2, 0, 0, 0], 0.0, Kind::Mm),
    c("MF", [0, 2, 0, 0, 0, 0], 0.0, Kind::Mf),
    c("RHO", [1, -2, 2, -1, 0, 0], -90.0, Kind::O1),
    c("Q1", [1, -2, 0, 1, 0, 0], -90.0, Kind::O1),
    c("T2", [2, 2, -3, 0, 0, 1], 0.0, Kind::Fixed),
    c("R2", [2, 2, -1, 0, 0, -1], 180.0, Kind::Fixed),
    c("2Q1", [1, -3, 0, 2, 0, 0], -90.0, Kind::O1),
    c("P1", [1, 1, -2, 0, 0, 0], -90.0, Kind::Fixed),
    c("2SM2", [2, 4, -4, 0, 0, 0], 0.0, Kind::M2),
    c("M3", [3, 0, 0, 0, 0, 0], 180.0, Kind::M3),
    c("L2", [2, 1, 0, -1, 0, 0], 180.0, Kind::L2),
    c("2MK3", [3, -1, 0, 0, 0, 0], -90.0, Kind::M2SquaredOverK1),
    c("K2", [2, 2, 0, 0, 0, 0], 0.0, Kind::K2),
    c("M8", [8, 0, 0, 0, 0, 0], 0.0, Kind::M2Fourth),
    c("MS4", [4, 2, -2, 0, 0, 0], 0.0, Kind::M2),
];

const fn c(name: &'static str, doodson: [i8; 6], phase: f64, kind: Kind) -> Constituent {
    Constituent {
        name,
        doodson,
        phase,
        kind,
    }
}

/// Where a constituent sits in [`ALL`], by NOAA's name for it.
pub fn index_of(name: &str) -> Option<usize> {
    ALL.iter().position(|c| c.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NOAA publishes each constituent's speed alongside its amplitude.
    /// If our Doodson numbers were wrong the speeds would not line up.
    #[test]
    fn speeds_match_the_ones_noaa_publishes() {
        let published = [
            ("M2", 28.984_104),
            ("S2", 30.0),
            ("N2", 28.439_730),
            ("K1", 15.041_069),
            ("M4", 57.968_208),
            ("O1", 13.943_035),
            ("M6", 86.952_313),
            ("MK3", 44.025_173),
            ("S4", 60.0),
            ("MN4", 57.423_834),
            ("NU2", 28.512_583),
            ("S6", 90.0),
            ("MU2", 27.968_208),
            ("2N2", 27.895_355),
            ("OO1", 16.139_101),
            ("LAM2", 29.455_625),
            ("S1", 15.0),
            ("M1", 14.496_694),
            ("J1", 15.585_443),
            ("MM", 0.544_375),
            ("SSA", 0.082_137),
            ("SA", 0.041_069),
            ("MSF", 1.015_896),
            ("MF", 1.098_033),
            ("RHO", 13.471_515),
            ("Q1", 13.398_661),
            ("T2", 29.958_933),
            ("R2", 30.041_067),
            ("2Q1", 12.854_286),
            ("P1", 14.958_931),
            ("2SM2", 31.015_896),
            ("M3", 43.476_156),
            ("L2", 29.528_479),
            ("2MK3", 42.927_140),
            ("K2", 30.082_137),
            ("M8", 115.936_416),
            ("MS4", 58.984_104),
        ];
        assert_eq!(published.len(), ALL.len());
        for (name, speed) in published {
            let c = ALL[index_of(name).unwrap_or_else(|| panic!("{name} missing"))];
            assert!(
                (c.speed() - speed).abs() < 2e-5,
                "{name}: {} vs {speed}",
                c.speed()
            );
        }
    }

    #[test]
    fn every_name_is_unique_and_findable() {
        for (i, c) in ALL.iter().enumerate() {
            assert_eq!(index_of(c.name), Some(i), "{}", c.name);
        }
        assert_eq!(index_of("m2"), Some(0));
        assert_eq!(index_of("nope"), None);
    }
}
