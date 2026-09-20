//! Turning a station's harmonic constants into a tide: the height (or the
//! current) at any moment, and the turning points a sailor actually reads
//! off — high and low water, slack and maximum flood or ebb.

use crate::astro::{Longitudes, Nodal};
use crate::constituent::{ALL, node_factor};
use crate::time;
use std::f64::consts::PI;

const DEG: f64 = PI / 180.0;

/// The node factors for one calendar year.
///
/// NOAA works these out for the middle of the year and then holds them
/// for the whole of it, and its published tables are built that way. A
/// prediction that varied them properly with time would be a better
/// model of the ocean but a worse match to the tables everyone else is
/// reading, so this follows NOAA.
#[derive(Clone, Debug)]
pub struct Year {
    pub year: i64,
    /// Node factor and nodal phase (radians) per constituent.
    factors: [(f64, f64); ALL.len()],
}

impl Year {
    pub fn new(year: i64) -> Year {
        let longitudes = Longitudes::at(time::mid_year(year));
        let nodal = Nodal::at(&longitudes);
        let mut factors = [(1.0, 0.0); ALL.len()];
        for (slot, c) in factors.iter_mut().zip(ALL) {
            *slot = node_factor(c.kind, &nodal, &longitudes);
        }
        Year { year, factors }
    }
}

/// A station's constants: what each constituent contributes, and the
/// level the sum is measured from.
///
/// For a tide that level is the station's mean sea level above the chart
/// datum. For a current it is the mean flow along the channel, which is
/// rarely zero — rivers run downhill.
#[derive(Clone, Debug, PartialEq)]
pub struct Harmonics {
    /// Meters, or centimeters a second for a current.
    pub amplitude: [f64; ALL.len()],
    /// Greenwich epoch κ, degrees.
    pub phase: [f64; ALL.len()],
    pub offset: f64,
}

impl Default for Harmonics {
    fn default() -> Harmonics {
        // Arrays this long have no derived Default.
        Harmonics {
            amplitude: [0.0; ALL.len()],
            phase: [0.0; ALL.len()],
            offset: 0.0,
        }
    }
}

impl Harmonics {
    /// The height or speed at a moment.
    pub fn at(&self, unix: i64, year: &Year) -> f64 {
        let longitudes = Longitudes::at(unix);
        let mut sum = self.offset;
        for (i, c) in ALL.iter().enumerate() {
            let amplitude = self.amplitude[i];
            if amplitude == 0.0 {
                continue;
            }
            let (f, u) = year.factors[i];
            let argument = c.argument(unix, &longitudes) - self.phase[i];
            sum += f * amplitude * (argument * DEG + u).cos();
        }
        sum
    }

    /// How fast it is changing, per second. Used to find the turns.
    fn rate(&self, unix: i64, year: &Year) -> f64 {
        let longitudes = Longitudes::at(unix);
        let mut sum = 0.0;
        for (i, c) in ALL.iter().enumerate() {
            let amplitude = self.amplitude[i];
            if amplitude == 0.0 {
                continue;
            }
            let (f, u) = year.factors[i];
            let argument = c.argument(unix, &longitudes) - self.phase[i];
            let per_second = c.speed() * DEG / 3600.0;
            sum -= f * amplitude * per_second * (argument * DEG + u).sin();
        }
        sum
    }

    /// Is any of this station's constants actually filled in?
    pub fn is_empty(&self) -> bool {
        self.amplitude.iter().all(|a| *a == 0.0)
    }

    /// The sum of every amplitude: the furthest the tide could reach from
    /// its mean level, if every constituent peaked at once.
    pub fn range(&self) -> f64 {
        self.amplitude.iter().map(|a| a.abs()).sum()
    }
}

/// One moment of a predicted curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub time: i64,
    pub value: f64,
}

/// A turning point: high or low water, or a current's slack and maximum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    High,
    Low,
    Slack,
    Flood,
    Ebb,
}

impl Turn {
    pub fn name(self) -> &'static str {
        match self {
            Turn::High => "high",
            Turn::Low => "low",
            Turn::Slack => "slack",
            Turn::Flood => "flood",
            Turn::Ebb => "ebb",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extreme {
    pub time: i64,
    pub value: f64,
    pub turn: Turn,
}

/// Predicts from `start` to `end` inclusive, every `step` seconds.
pub fn series(h: &Harmonics, start: i64, end: i64, step: i64) -> Vec<Point> {
    let step = step.max(1);
    let mut years = Years::default();
    let mut out = Vec::with_capacity(((end - start) / step + 1).max(0) as usize);
    let mut t = start;
    while t <= end {
        out.push(Point {
            time: t,
            value: h.at(t, years.for_time(t)),
        });
        t += step;
    }
    out
}

/// Highs and lows between two times.
///
/// The rate of change is a smooth sum of sinusoids, so its sign changes
/// are found by walking a coarse step and then closing in. A semidiurnal
/// tide turns about every six hours; ten minutes is far finer than the
/// shortest constituent in the set, which is M8 at just over three hours.
pub fn extremes(h: &Harmonics, start: i64, end: i64) -> Vec<Extreme> {
    let mut years = Years::default();
    turns_where(h, start, end, |h, t, y| h.rate(t, y))
        .into_iter()
        .map(|(time, rate_rising)| Extreme {
            time,
            value: h.at(time, years.for_time(time)),
            // The tide stops rising at a high, so the rate is falling
            // through zero there, not rising.
            turn: if rate_rising { Turn::Low } else { Turn::High },
        })
        .collect()
}

/// A current's slacks and maximums, in time order.
///
/// Slack is where the flow crosses zero, and the maximum flood or ebb is
/// where it stops changing. Both are turns of the same curve, one of the
/// value and one of its rate, so they interleave.
pub fn current_turns(h: &Harmonics, start: i64, end: i64) -> Vec<Extreme> {
    let mut years = Years::default();
    let mut out: Vec<Extreme> = turns_where(h, start, end, |h, t, y| h.at(t, y))
        .into_iter()
        .map(|(time, _)| Extreme {
            time,
            value: 0.0,
            turn: Turn::Slack,
        })
        .collect();
    out.extend(
        turns_where(h, start, end, |h, t, y| h.rate(t, y))
            .into_iter()
            .map(|(time, _)| {
                let value = h.at(time, years.for_time(time));
                Extreme {
                    time,
                    value,
                    turn: if value >= 0.0 { Turn::Flood } else { Turn::Ebb },
                }
            }),
    );
    out.sort_by_key(|e| e.time);
    out
}

/// Where `f` crosses zero, to the second. The flag says whether it was
/// going up as it crossed.
fn turns_where(
    h: &Harmonics,
    start: i64,
    end: i64,
    f: impl Fn(&Harmonics, i64, &Year) -> f64,
) -> Vec<(i64, bool)> {
    const STEP: i64 = 600;
    if h.is_empty() || end <= start {
        return Vec::new();
    }
    let mut years = Years::default();
    let mut out = Vec::new();
    let mut previous_time = start;
    let mut previous = f(h, start, years.for_time(start));
    let mut t = start + STEP;
    while previous_time < end {
        let t_now = t.min(end);
        let value = f(h, t_now, years.for_time(t_now));
        if previous != 0.0 && (previous < 0.0) != (value < 0.0) {
            // Bisection, not Newton: the curve is well behaved and a
            // bracket that cannot escape is worth more than speed here.
            let (mut lo, mut hi) = (previous_time, t_now);
            let rising = previous < value;
            while hi - lo > 1 {
                let mid = lo + (hi - lo) / 2;
                let v = f(h, mid, years.for_time(mid));
                if (v < 0.0) == (previous < 0.0) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            out.push((hi, rising));
        }
        previous_time = t_now;
        previous = value;
        t += STEP;
    }
    out
}

/// Drops a flood or ebb too weak to be worth printing, and the extra
/// slack it created.
///
/// Where the stream barely reverses, the maths finds a maximum of a
/// fraction of a knot between two slacks minutes apart. That is true but
/// useless: what a sailor wants to read is one slack. The turn dropped
/// is the weak maximum, and of the two slacks around it the later one,
/// so the slack that remains is where the stream first died.
pub fn drop_weak_turns(turns: Vec<Extreme>, threshold: f64) -> Vec<Extreme> {
    let weak: Vec<usize> = turns
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            matches!(e.turn, Turn::Flood | Turn::Ebb)
                && e.value.abs() < threshold
                && *i > 0
                && i + 1 < turns.len()
                && turns[i - 1].turn == Turn::Slack
                && turns[i + 1].turn == Turn::Slack
        })
        .map(|(i, _)| i)
        .collect();
    if weak.is_empty() {
        return turns;
    }
    let mut drop = vec![false; turns.len()];
    for i in weak {
        drop[i] = true;
        drop[i + 1] = true;
    }
    turns
        .into_iter()
        .zip(drop)
        .filter(|(_, d)| !d)
        .map(|(e, _)| e)
        .collect()
}

/// Node factors, worked out once per year as they are needed.
#[derive(Default)]
struct Years {
    cached: Vec<Year>,
}

impl Years {
    fn for_time(&mut self, t: i64) -> &Year {
        let year = time::year_of(t);
        if let Some(i) = self.cached.iter().position(|y| y.year == year) {
            return &self.cached[i];
        }
        self.cached.push(Year::new(year));
        self.cached.last().expect("just pushed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constituent::index_of;
    use crate::time::unix;

    /// A single M2 with a known amplitude and phase: the height must be a
    /// clean cosine, high water every 12 h 25 m.
    fn just_m2() -> Harmonics {
        let mut h = Harmonics::default();
        h.amplitude[index_of("M2").unwrap()] = 1.0;
        h.phase[index_of("M2").unwrap()] = 0.0;
        h.offset = 2.0;
        h
    }

    #[test]
    fn a_lone_constituent_repeats_at_its_own_speed() {
        let h = just_m2();
        let year = Year::new(2026);
        let period = (360.0 / ALL[index_of("M2").unwrap()].speed() * 3600.0).round() as i64;
        let t = unix(2026, 9, 20, 0, 0, 0);
        for n in 1..6 {
            let a = h.at(t, &year);
            let b = h.at(t + period * n, &year);
            assert!((a - b).abs() < 2e-3, "cycle {n}: {a} vs {b}");
        }
    }

    #[test]
    fn highs_and_lows_alternate_and_sit_at_the_turns() {
        let h = just_m2();
        let start = unix(2026, 9, 20, 0, 0, 0);
        let found = extremes(&h, start, start + 4 * 86_400);
        assert!(found.len() >= 15, "only {} turns", found.len());
        let mut years = Years::default();
        for pair in found.windows(2) {
            assert_ne!(pair[0].turn, pair[1].turn, "two of a kind in a row");
            let gap = pair[1].time - pair[0].time;
            assert!((22_000..=23_000).contains(&gap), "gap {gap}s");
        }
        for e in &found {
            // A turn is flat, and either side of it is lower (or higher).
            let year = years.for_time(e.time);
            let before = h.at(e.time - 600, year);
            let after = h.at(e.time + 600, year);
            if e.turn == Turn::High {
                assert!(e.value > before && e.value > after, "{e:?}");
            } else {
                assert!(e.value < before && e.value < after, "{e:?}");
            }
        }
    }

    #[test]
    fn a_current_slacks_between_its_maximums() {
        let mut h = just_m2();
        h.offset = 0.0;
        let start = unix(2026, 9, 20, 0, 0, 0);
        let turns = current_turns(&h, start, start + 2 * 86_400);
        assert!(turns.len() >= 15, "only {} turns", turns.len());
        for pair in turns.windows(2) {
            assert!(pair[0].time < pair[1].time);
            let one_is_slack = (pair[0].turn == Turn::Slack) != (pair[1].turn == Turn::Slack);
            assert!(one_is_slack, "{:?} then {:?}", pair[0].turn, pair[1].turn);
        }
        for t in turns.iter().filter(|t| t.turn == Turn::Slack) {
            assert!(t.value.abs() < 1e-6);
        }
        // With a mean flow of zero, flood and ebb are the same size.
        let flood = turns.iter().find(|t| t.turn == Turn::Flood).unwrap();
        let ebb = turns.iter().find(|t| t.turn == Turn::Ebb).unwrap();
        assert!((flood.value + ebb.value).abs() < 1e-3);
    }

    #[test]
    fn a_mean_flow_that_never_slacks_has_no_slack() {
        let mut h = just_m2();
        h.amplitude[index_of("M2").unwrap()] = 0.2;
        h.offset = 1.0;
        let start = unix(2026, 9, 20, 0, 0, 0);
        let turns = current_turns(&h, start, start + 2 * 86_400);
        assert!(!turns.is_empty());
        assert!(turns.iter().all(|t| t.turn != Turn::Slack));
        assert!(turns.iter().all(|t| t.turn == Turn::Flood));
    }

    #[test]
    fn a_stream_that_barely_reverses_reads_as_one_slack() {
        let e = |time, value, turn| Extreme { time, value, turn };
        let turns = vec![
            e(0, 50.0, Turn::Flood),
            e(100, 0.0, Turn::Slack),
            e(200, -0.001, Turn::Ebb),
            e(300, 0.0, Turn::Slack),
            e(400, 60.0, Turn::Flood),
            e(500, 0.0, Turn::Slack),
            e(600, -40.0, Turn::Ebb),
        ];
        let kept = drop_weak_turns(turns.clone(), 1.0);
        assert_eq!(
            kept.iter().map(|e| e.time).collect::<Vec<_>>(),
            [0, 100, 400, 500, 600]
        );
        // The slack kept is the one where the stream first died.
        assert_eq!(kept[1].time, 100);
        // Nothing is weak, nothing goes.
        assert_eq!(drop_weak_turns(turns.clone(), 0.0005).len(), turns.len());
        // A weak maximum that is not between two slacks is left alone:
        // it is the whole of a run, not a blip inside one.
        let lone = vec![e(0, 0.0, Turn::Slack), e(100, 0.001, Turn::Flood)];
        assert_eq!(drop_weak_turns(lone.clone(), 1.0), lone);
        assert!(drop_weak_turns(Vec::new(), 1.0).is_empty());
    }

    #[test]
    fn an_empty_station_predicts_nothing_rather_than_hanging() {
        let h = Harmonics::default();
        assert!(h.is_empty());
        let start = unix(2026, 9, 20, 0, 0, 0);
        assert!(extremes(&h, start, start + 86_400).is_empty());
        assert!(current_turns(&h, start, start + 86_400).is_empty());
        assert_eq!(series(&h, start, start, 600).len(), 1);
    }

    #[test]
    fn a_series_covers_its_window_and_crosses_the_new_year() {
        let h = just_m2();
        let start = unix(2026, 12, 31, 21, 0, 0);
        let points = series(&h, start, start + 6 * 3600, 3600);
        assert_eq!(points.len(), 7);
        assert_eq!(points[0].time, start);
        assert_eq!(points[6].time, start + 6 * 3600);
        // The node factors step at midnight, so the curve has a seam
        // there, but it must stay small: NOAA's tables have it too.
        let seam = (points[3].value - points[2].value).abs();
        assert!(seam < 0.6, "seam {seam}");
        assert!(series(&h, start, start - 1, 600).is_empty());
    }
}
