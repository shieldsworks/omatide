# The bay

`omatide bay` prints the tide and the stream at the named places in San
Francisco Bay for one moment.

## Sub-features

- `bay-now` prints the heading and whatever places the catalog can speak for.
- `bay-at` pins the moment with `--at`.
- `bay-partial` a catalog that lacks a place omits that place.

## How to get to it (user POV)

- Run `omatide bay` in a terminal.
- Run `omatide bay --at 2026-09-20T17:00:00Z`.
- In the window, drag the bar under the bay, or press `[` and `]`. That paint is `window.md`.

## Driving it with the bay command

Preconditions.

- The baseline from `README.md` is enough to see Golden Gate's tide and to see a missing stream.
- A pass of every named narrows needs `omatide fetch` with no `--region`, into this run's `OMATIDE_DATA_DIR`. This proof did not run that fetch.

- **A fixed moment, small catalog.** Run `omatide bay --at 2026-09-20T17:00:00Z`. Exit 0. Stdout begins with `San Francisco Bay at 2026-09-20T17:00:00Z`, then `The tide, height above chart datum:`, then a `Golden Gate` line containing `m`. `The stream:` follows and, on this catalog, no place line follows it. Observed on 2026-10-14.

```
San Francisco Bay at 2026-09-20T17:00:00Z

The tide, height above chart datum:
  Golden Gate             1.35 m  low in 3h 57m

The stream:
```

- **Proof of the partial bay.** Save stdout under `$run/artifacts/verify/bay/at.out`. This is a pass of `bay-at` and `bay-partial` for Golden Gate's tide only.
- **The whole bay.** Not driven on 2026-10-14. The command would be `omatide fetch` with no `--region`, then `omatide bay --at 2026-09-20T17:00:00Z`. Unmet precondition, the default-region catalog. Do not mark `Point Bonita`, `Raccoon Strait`, or `Port Chicago` as passed from the transcript above.

## Gotchas

- A place whose station is missing from the catalog is omitted. An empty stream section means those places are absent.
- `--at` expects a UTC time like `2026-09-20T17:00:00Z`. `omatide bay --at yesterday` exits 1. Stderr is `omatide: at: expected a UTC time like 2026-09-20T17:00:00Z`.
- The Golden Gate height in the transcript is for that fixed instant. It does not move with the wall clock. The `in 3h 57m` text is part of that instant too.
- Do not copy the user's `~/.local/share/omatide/stations.json` in to make the full bay pass. Run `omatide fetch`.
