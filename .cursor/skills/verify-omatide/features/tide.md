# The tide

`omatide tide` prints the height above chart datum now, and the highs and
lows to come, at a station or at the nearest station to a position.

## Sub-features

- `tide-station` prints Golden Gate for station `9414290`.
- `tide-home` with no argument uses the default home and the same station in this catalog.
- `tide-position` takes `latitude,longitude` and names the nearest tide station.
- `tide-missing` with an empty data directory tells the sailor to fetch.
- `tide-unknown` names a station that is not in the catalog and exits 1.

## How to get to it (user POV)

- Run `omatide tide` in a terminal. With no argument it uses home from `OMATIDE_CONFIG`, or the Berkeley Marina when that file is absent.
- Run `omatide tide 9414290`.
- Run `omatide tide 37.8063,-122.4659`.
- Run `omatide stations` to see the same station in the list.
- The window and the bar read the same station from the engine. Those paths are `window.md` and `engine.md`.

## Driving it with the tide command

Preconditions.

- The baseline from `README.md`. Doctor passes.
- `9414290` is in `$run/data/omatide/stations.json`.

- **Fetch landed.** Open the catalog file. Run `grep -F '"id": "9414290"' "$run/data/omatide/stations.json"`. The line is present. On 2026-10-14 the fetch log also contained `1 tide stations in the region`, `9414290 SAN FRANCISCO (Golden Gate)`, `2 stations, 2 of them predictable`, and `saved to` the catalog path. Exit 0.
- **One station.** Run `omatide tide 9414290`. Exit 0. Stdout begins with `9414290 — SAN FRANCISCO (Golden Gate)` and the next line contains `m above chart datum`. Later lines contain `high` and `low`. Observed on 2026-10-14, the meters and the times will differ.

```
9414290 — SAN FRANCISCO (Golden Gate)
  now  0.93 m above chart datum
  2026-10-14T10:19:20Z  high  1.37 m
  2026-10-14T14:44:18Z  low   0.96 m
  2026-10-14T20:46:13Z  high  1.74 m
  2026-10-15T03:44:38Z  low   0.03 m
```

- **Home.** Run `omatide tide`. Exit 0. The first line is the same station line. This catalog has one tide station, so home at the Berkeley Marina still resolves to Golden Gate.
- **A position.** Run `omatide tide 37.8063,-122.4659`. Exit 0. The first line is the same station line.
- **The list.** Run `omatide stations`. Exit 0. Stdout contains `9414290` and `SAN FRANCISCO (Golden Gate)`. Observed on 2026-10-14 it also contained `PCT0286-1` and `Fort Point, 0.5 nmi. east of`.
- **No catalog.** Use `$run/env.empty` so `OMATIDE_DATA_DIR` is `$run/empty` after the rest of the run's env. Exit 1. The last stderr line keeps the backticks.

```sh
set -euo pipefail
: "${run:?}"
mkdir -p "$run/artifacts/verify/tide"
set +e
env $(cat "$run/env.empty") "$bin" tide 9414290 \
  >"$run/artifacts/verify/tide/missing.out" \
  2>"$run/artifacts/verify/tide/missing.err"
status=$?
set -e
test "$status" -eq 1
last=$(tail -n 1 "$run/artifacts/verify/tide/missing.err")
test "$last" = 'Run `omatide fetch` first.'
grep -F 'No such file or directory (os error 2)' \
  "$run/artifacts/verify/tide/missing.err"
test -f "$run/data/omatide/stations.json"
```

  Observed on 2026-10-14 the first stderr line ended with `No such file or directory (os error 2)`.
- **Unknown station.** Run `omatide tide SFB9999`. Exit 1. Stderr is `omatide: no station SFB9999`.
- **Proof.** Save each command's stdout, stderr, and exit code under `$run/artifacts/verify/tide/`. The second look is the `9414290` line in `stations.json` and in `omatide stations`.

## Gotchas

- The height and the turn times change with the clock. Match the station line and the words `m above chart datum`, `high`, and `low`.
- `--days` accepts 1 to 14 and defaults to 2. It does not pin the clock. There is no `--at` on `tide`.
- `tide-station` needs the catalog. An empty `OMATIDE_DATA_DIR` is `tide-missing`.
- Do not point `OMATIDE_DATA_DIR` at the user's `~/.local/share/omatide`.
- This catalog's only tide station is about 8 nautical miles from the default home. A full bay catalog picks Berkeley instead. Do not treat Golden Gate as the home station once `omatide fetch` has run with no `--region`.
