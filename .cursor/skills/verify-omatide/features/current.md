# The stream

`omatide current` prints the speed and the set now, and the slacks and
maximums to come, at a current station or at the nearest one.

## Sub-features

- `current-home` prints the only current bin in the baseline catalog.
- `current-station` prints that same bin when named.
- `current-missing` with an empty data directory tells the sailor to fetch.

## How to get to it (user POV)

- Run `omatide current` in a terminal.
- Run `omatide current PCT0286-1`.
- Run `omatide stations current` to list current stations only.

## Driving it with the current command

Preconditions.

- The baseline from `README.md`. Doctor passes.

- **Home.** Run `omatide current`. Exit 0. The first line is `PCT0286-1 — Fort Point, 0.5 nmi. east of`. The next line contains `kn`. Later lines contain `slack`. Observed on 2026-10-14.

```
PCT0286-1 — Fort Point, 0.5 nmi. east of
  now  0.52 kn setting 99°
  2026-10-14T11:41:45Z  slack 0.00 kn
  2026-10-14T13:05:26Z  ebb   0.68 kn
```

- **Named bin.** Run `omatide current PCT0286-1`. Exit 0. The first line matches the home run.
- **The list.** Run `omatide stations current`. Exit 0. Stdout contains `PCT0286-1` and does not contain `9414290`.
- **No catalog.** Use `$run/env.empty`. Exit 1. The last stderr line keeps the backticks.

```sh
omatide_current_missing() {
  local _opts _status last
  omatide_fence_save _opts
  _status=0
  test -n "${run-}" && test -n "${bin-}" && omatide_env_ok "$run/env.empty" || _status=1
  if test "$_status" -eq 0; then
    mkdir -p "$run/artifacts/verify/current" || _status=$?
  fi
  if test "$_status" -eq 0; then
    env $(cat "$run/env.empty") "$bin" current \
      >"$run/artifacts/verify/current/missing.out" \
      2>"$run/artifacts/verify/current/missing.err"
    test "$?" -eq 1 || _status=1
  fi
  if test "$_status" -eq 0; then
    last=$(tail -n 1 "$run/artifacts/verify/current/missing.err") || _status=$?
  fi
  if test "$_status" -eq 0; then
    test "$last" = 'Run `omatide fetch` first.' || _status=$?
  fi
  if test "$_status" -eq 0; then
    test -f "$run/data/omatide/stations.json" || _status=$?
  fi
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}
omatide_current_missing
```
- **Proof.** Save stdout, stderr, and the exit code under `$run/artifacts/verify/current/`. The second look is `"id": "PCT0286"` in `stations.json`.

## Gotchas

- Speeds and times move. Match the station line, `kn`, and `slack`.
- The baseline bin is 16.8 m down. The default `depth` is 3. With one bin in the catalog, that bin is the one the command prints.
- `PCT0286` without the bin is `no station PCT0286`. The key includes `-1`.
- A full bay catalog picks a different nearest current station for the Berkeley home. Do not require `PCT0286-1` after `omatide fetch` with no `--region`.
