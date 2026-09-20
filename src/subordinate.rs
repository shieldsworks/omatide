//! Predicting a station that has no constants of its own, only offsets
//! against one that has.
//!
//! NOAA publishes the turns — high and low water, or slack and maximum
//! stream — and nothing in between, because the offsets are only defined
//! at the turns. To draw a curve, omatide warps the reference station's
//! own curve onto the subordinate's turns: time is stretched so each pair
//! of turns lines up, and the height is stretched so the two ends meet.
//! That keeps the shape of the real tide, including the shallow-water
//! lopsidedness that is everywhere in San Francisco Bay, instead of
//! replacing it with a plain cosine.

use crate::predict::{self, Extreme, Harmonics, Point, Turn};
use crate::station::{CurrentOffsets, TideOffsets};

/// Enough either side of a window to be sure of bracketing it: the
/// longest run between turns is a little over half a day, and the
/// largest offset in the bay is a little over two hours.
const MARGIN: i64 = 86_400;

/// The subordinate station's highs and lows over a window.
pub fn tide_extremes(
    reference: &Harmonics,
    offsets: &TideOffsets,
    start: i64,
    end: i64,
) -> Vec<Extreme> {
    shift_tide(reference, offsets, start - MARGIN, end + MARGIN)
        .into_iter()
        .filter(|e| (start..=end).contains(&e.time))
        .collect()
}

/// The subordinate station's slacks and maximums over a window.
pub fn current_turns(
    reference: &Harmonics,
    offsets: &CurrentOffsets,
    start: i64,
    end: i64,
) -> Vec<Extreme> {
    shift_current(reference, offsets, start - MARGIN, end + MARGIN)
        .into_iter()
        .filter(|e| (start..=end).contains(&e.time))
        .collect()
}

/// The subordinate station's curve, every `step` seconds.
pub fn tide_series(
    reference: &Harmonics,
    offsets: &TideOffsets,
    start: i64,
    end: i64,
    step: i64,
) -> Vec<Point> {
    let wide = (start - MARGIN, end + MARGIN);
    let theirs = predict::extremes(reference, wide.0, wide.1);
    let ours = shift_tide(reference, offsets, wide.0, wide.1);
    warp(reference, &theirs, &ours, start, end, step)
}

pub fn current_series(
    reference: &Harmonics,
    offsets: &CurrentOffsets,
    start: i64,
    end: i64,
    step: i64,
) -> Vec<Point> {
    let wide = (start - MARGIN, end + MARGIN);
    let theirs = predict::current_turns(reference, wide.0, wide.1);
    let ours = shift_current(reference, offsets, wide.0, wide.1);
    warp(reference, &theirs, &ours, start, end, step)
}

/// Moves each of the reference's highs and lows by its own offset.
fn shift_tide(reference: &Harmonics, offsets: &TideOffsets, start: i64, end: i64) -> Vec<Extreme> {
    predict::extremes(reference, start, end)
        .into_iter()
        .map(|e| {
            let high = e.turn == Turn::High;
            Extreme {
                time: e.time + offsets.minutes(high) * 60,
                value: offsets.apply(e.value, high),
                turn: e.turn,
            }
        })
        .collect()
}

/// Moves each of the reference's four kinds of turn by its own offset.
///
/// A slack is named for what follows it, so which offset a slack takes
/// depends on whether the stream floods or ebbs next.
fn shift_current(
    reference: &Harmonics,
    offsets: &CurrentOffsets,
    start: i64,
    end: i64,
) -> Vec<Extreme> {
    let turns = predict::current_turns(reference, start, end);
    let mut out = Vec::with_capacity(turns.len());
    for (i, e) in turns.iter().enumerate() {
        let (minutes, value) = match e.turn {
            Turn::Flood => (offsets.max_flood_minutes, e.value * offsets.flood_factor),
            Turn::Ebb => (offsets.max_ebb_minutes, e.value * offsets.ebb_factor),
            _ => {
                let floods_next = turns[i + 1..]
                    .iter()
                    .find(|n| n.turn != Turn::Slack)
                    .map(|n| n.turn == Turn::Flood)
                    // Nothing after it in the window: fall back on what
                    // came before, which must be the other one.
                    .unwrap_or_else(|| {
                        turns[..i]
                            .iter()
                            .rev()
                            .find(|p| p.turn != Turn::Slack)
                            .is_some_and(|p| p.turn == Turn::Ebb)
                    });
                let minutes = if floods_next {
                    offsets.slack_before_flood_minutes
                } else {
                    offsets.slack_before_ebb_minutes
                };
                (minutes, 0.0)
            }
        };
        out.push(Extreme {
            time: e.time + minutes * 60,
            value,
            turn: e.turn,
        });
    }
    // Offsets differ per turn, so two turns can cross over. Keep them in
    // order; a curve that went backwards in time would be nonsense.
    out.sort_by_key(|e| e.time);
    out
}

/// Stretches the reference's curve onto the subordinate's turns.
///
/// `theirs` and `ours` are the same turns, in the same order, before and
/// after the offsets. Between one pair and the next, time runs from
/// `ours[k]` to `ours[k+1]` while the reference runs from `theirs[k]` to
/// `theirs[k+1]`, and the value is shifted and scaled so both ends land
/// on the subordinate's own.
fn warp(
    reference: &Harmonics,
    theirs: &[Extreme],
    ours: &[Extreme],
    start: i64,
    end: i64,
    step: i64,
) -> Vec<Point> {
    let step = step.max(1);
    if theirs.len() != ours.len() || theirs.len() < 2 {
        // Not enough turns to warp between — fall back on the reference
        // itself rather than draw nothing.
        return predict::series(reference, start, end, step);
    }
    let mut out = Vec::new();
    let mut k = 0usize;
    let mut t = start;
    while t <= end {
        while k + 2 < ours.len() && t >= ours[k + 1].time {
            k += 1;
        }
        let (a, b) = (&ours[k], &ours[k + 1]);
        let (ra, rb) = (&theirs[k], &theirs[k + 1]);
        let span = (b.time - a.time) as f64;
        let their_span = (rb.time - ra.time) as f64;
        let value = if span <= 0.0 || their_span <= 0.0 {
            a.value
        } else {
            let fraction = (t - a.time) as f64 / span;
            let at = ra.time + (fraction * their_span).round() as i64;
            let raw = predict::series(reference, at, at, 1)[0].value;
            // Map the reference's own two ends onto ours.
            let their_rise = rb.value - ra.value;
            if their_rise.abs() < 1e-9 {
                a.value + (b.value - a.value) * fraction
            } else {
                a.value + (raw - ra.value) / their_rise * (b.value - a.value)
            }
        };
        out.push(Point { time: t, value });
        t += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constituent::index_of;
    use crate::station::HeightRule;
    use crate::time::unix;

    fn reference() -> Harmonics {
        let mut h = Harmonics::default();
        h.amplitude[index_of("M2").unwrap()] = 0.576;
        h.phase[index_of("M2").unwrap()] = 208.2;
        h.amplitude[index_of("K1").unwrap()] = 0.370;
        h.phase[index_of("K1").unwrap()] = 225.4;
        h.amplitude[index_of("O1").unwrap()] = 0.230;
        h.phase[index_of("O1").unwrap()] = 208.4;
        h.offset = 0.951;
        h
    }

    fn sausalito() -> TideOffsets {
        TideOffsets {
            high_minutes: 10,
            low_minutes: 14,
            high_height: 0.97,
            low_height: 1.0,
            rule: HeightRule::Ratio,
        }
    }

    #[test]
    fn turns_move_by_their_own_offset_and_scale() {
        let h = reference();
        let start = unix(2026, 9, 20, 0, 0, 0);
        let end = start + 3 * 86_400;
        let theirs = predict::extremes(&h, start, end);
        let ours = tide_extremes(&h, &sausalito(), start, end);
        assert!(!theirs.is_empty());
        // Every one of ours sits inside the window, so a couple at the
        // edges may drop; match on the ones that stayed.
        for mine in &ours {
            let base = theirs
                .iter()
                .min_by_key(|t| (t.time - mine.time).abs())
                .unwrap();
            assert_eq!(base.turn, mine.turn);
            let high = mine.turn == Turn::High;
            assert_eq!(mine.time - base.time, if high { 600 } else { 840 });
            let want = if high { base.value * 0.97 } else { base.value };
            assert!((mine.value - want).abs() < 1e-9);
        }
    }

    #[test]
    fn the_warped_curve_passes_through_its_own_turns() {
        let h = reference();
        let start = unix(2026, 9, 20, 0, 0, 0);
        let end = start + 2 * 86_400;
        let curve = tide_series(&h, &sausalito(), start, end, 60);
        assert_eq!(curve.len() as i64, (end - start) / 60 + 1);
        for turn in tide_extremes(&h, &sausalito(), start + 3600, end - 3600) {
            let at = curve
                .iter()
                .min_by_key(|p| (p.time - turn.time).abs())
                .unwrap();
            assert!(
                (at.value - turn.value).abs() < 0.01,
                "{:?}: curve {} vs turn {}",
                turn.turn,
                at.value,
                turn.value
            );
        }
    }

    #[test]
    fn the_warped_curve_keeps_the_references_shape() {
        // With no offsets at all, warping must give the reference back.
        let h = reference();
        let none = TideOffsets {
            high_minutes: 0,
            low_minutes: 0,
            high_height: 1.0,
            low_height: 1.0,
            rule: HeightRule::Ratio,
        };
        let start = unix(2026, 9, 20, 0, 0, 0);
        let end = start + 86_400;
        let warped = tide_series(&h, &none, start, end, 600);
        let plain = predict::series(&h, start, end, 600);
        for (a, b) in warped.iter().zip(&plain) {
            assert_eq!(a.time, b.time);
            assert!((a.value - b.value).abs() < 1e-6, "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn a_stream_takes_a_different_offset_at_each_kind_of_turn() {
        let mut h = reference();
        h.offset = 0.0;
        // Alcatraz, west of: NOAA's offsets against the Golden Gate.
        let offsets = CurrentOffsets {
            slack_before_flood_minutes: 15,
            max_flood_minutes: 0,
            slack_before_ebb_minutes: 24,
            max_ebb_minutes: 20,
            flood_factor: 0.8,
            ebb_factor: 1.1,
        };
        let start = unix(2026, 9, 20, 0, 0, 0);
        let end = start + 2 * 86_400;
        let theirs = predict::current_turns(&h, start, end);
        let ours = current_turns(&h, &offsets, start, end);
        assert!(ours.len() >= 10, "{} turns", ours.len());
        // Each of ours is one of theirs, moved by the offset its own
        // kind of turn takes. Matching on nearest time would go wrong
        // where two turns nearly coincide, so look for the one that
        // fits exactly.
        for mine in &ours {
            let (shifts, factor) = match mine.turn {
                Turn::Flood => (vec![0], 0.8),
                Turn::Ebb => (vec![20], 1.1),
                _ => (vec![15, 24], 0.0),
            };
            let found = theirs.iter().any(|base| {
                base.turn == mine.turn
                    && shifts.contains(&((mine.time - base.time) / 60))
                    && (mine.value - base.value * factor).abs() < 1e-9
            });
            assert!(found, "no base turn for {mine:?}");
        }
        // Slacks still separate the floods from the ebbs.
        for pair in ours.windows(2) {
            assert!(pair[0].time <= pair[1].time);
        }
        let curve = current_series(&h, &offsets, start, end, 300);
        assert_eq!(curve.len() as i64, (end - start) / 300 + 1);
    }

    #[test]
    fn a_station_with_no_turns_falls_back_on_its_reference() {
        let h = Harmonics::default();
        let start = unix(2026, 9, 20, 0, 0, 0);
        let curve = tide_series(&h, &sausalito(), start, start + 3600, 600);
        assert_eq!(curve.len(), 7);
        assert!(curve.iter().all(|p| p.value == 0.0));
        assert!(tide_extremes(&h, &sausalito(), start, start + 3600).is_empty());
    }
}
