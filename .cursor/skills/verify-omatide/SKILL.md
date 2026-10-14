---
name: verify-omatide
description: "Drives omatide the way a sailor does, through the tide, current, and bay commands, the engine socket, and the Quickshell window when it can run. Use before calling a behavior change done, when reviewing a change, or when asked to verify omatide."
---

# Verify omatide

`scripts/verify.sh` proves the window can find this checkout and start its
engine binary. This skill proves the program works. It runs the real
binary, drives a feature from the sailor's side, and keeps evidence. Run
both. The agent that built a change does not sign off on this run.

## Launch

Run from the repository root. Find that root with
`git rev-parse --show-toplevel`. This proof ran in `/workspace`. A clone
at `~/Work/omahoy/omatide` is the same root when that directory is the
clone. Do not hard-code either path.

Every run gets its own directory. Do not read or write
`~/.local/share/omatide` or `~/.config/omatide`.

```sh
mise install
cargo build --release --locked
run=$(mktemp -d /tmp/omatide-verify.XXXXXX)
mkdir -p "$run/home" "$run/artifacts/verify"
bin=$(realpath ./target/release/omatide)
cat > "$run/env" <<EOF
HOME=$run/home
XDG_RUNTIME_DIR=$run
XDG_DATA_HOME=$run/data
XDG_CONFIG_HOME=$run/config
OMATIDE_DATA_DIR=$run/data/omatide
OMATIDE_CONFIG=$run/config.toml
TZ=UTC
EOF
env $(cat "$run/env") "$bin" fetch --region 37.80,-122.48,37.82,-122.45 \
  >"$run/fetch.log" 2>&1
```

Ready when `$run/fetch.log` contains `2 stations, 2 of them predictable`
and `saved to $run/data/omatide/stations.json`, and that file exists. The
box holds NOAA station `9414290`, San Francisco at the Golden Gate, and
one current bin, `PCT0286`. Fetch needs the network once. After that
fetch, the commands below need no network.

`omatide fetch` with no `--region` uses the default bay and takes a couple
of minutes. This proof did not run that fetch. `features/bay.md` says
what the small box leaves out.

This cloud VM had no `mise` binary, and its default `rustc` was 1.83.0.
Edition 2024 needs Rust 1.85. `Cargo.toml` sets `rust-version` to 1.89.
`mise.toml` pins 1.98. The proof built with
`rustup run 1.98.0 cargo build --release --locked`. A machine that has
run `mise install` skips that rustup step.

The engine, for the socket features, is a second step. See
`features/engine.md`. The window's own start is `features/window.md`.

## Doctor

```sh
realpath ./target/release/omatide
env $(cat "$run/env") "$bin" --version
data=$(sed -n 's/^OMATIDE_DATA_DIR=//p' "$run/env")
test -f "$data/stations.json" && echo "catalog $data/stations.json"
grep -c . "$run/env"
```

The version line is `omatide 0.1.0`. The binary path is inside this
clone. The catalog path is under `$run`. The env file has 7 lines, and
every path in it is under `$run`. Never drive the user's catalog, config,
or `$XDG_RUNTIME_DIR/omatide/tide.sock`.

Observed on 2026-10-14, with this clone at `/workspace`.

```
/workspace/target/release/omatide
omatide 0.1.0
catalog /tmp/omatide-verify.mGvGWP/data/omatide/stations.json
7
```

The temp directory differs every launch.

## Drive

Follow `features/`. Start at `features/README.md`. A proof that drives one
convenient entry point is incomplete when the map lists others.

Write `omatide` below as `env $(cat "$run/env") "$bin"`.

- A sailor's terminal is `omatide tide`, `omatide current`, `omatide bay`,
  `omatide stations`, `omatide watch`, and `omatide fetch`.
- The engine is `omatide run`, then `omatide watch` on the same socket.
  `docs/protocol.md` is the line format.
- The window is `./run.sh`, which execs `quickshell -p ui/shell.qml`. The
  window starts `omatide run` itself when the socket is down. Quickshell
  on a Hyprland session is required to paint it. When `quickshell` is not
  on `PATH`, do not skip `features/window.md`. Drive that file and say
  the painted window was not driven.

`timeout` ends `omatide watch` with exit 124 while the engine is still
running. Record that exit and judge the output file.

`scripts/verify.sh` checks the QML path from this file's directory and
the plugin directory, then checks that the window shows `Tide.lastLog`.
On 2026-10-14, before the Qt6 packages were installed, it printed
`Qt6 QML dev packages not installed. Skipped.` and exited 0. A skip
leaves the path check unproved. The pass text is
`All engine-path checks passed.` After the packages named in
`.github/workflows/ci.yml` were installed, this VM printed that line.

## Evidence

Write under `$run/artifacts/verify/<feature>/`. Cleanup copies
`$run/artifacts/verify` to `${EVIDENCE_DIR:-/tmp/omatide-evidence}`.

- Save the command, stdout, stderr, and the exit code.
- For the socket, save the lines. The second look is another `omatide
  watch`, or `omatide stations` for the same station id.
- For fetch, the second look is `stations.json` on disk. Confirm the
  station id is in the file, not only in the fetch log.
- The NOAA download is the real `omatide fetch`. Do not hand-write
  `stations.json`.
- If a path cannot be reached, write the command and the missing
  precondition. Do not mark that path passed.

## Cleanup

```sh
if test -f "$run/pid"; then
  kill "$(cat "$run/pid")" 2>/dev/null || true
fi
if test -f "$run/window.pid"; then
  kill "$(cat "$run/window.pid")" 2>/dev/null || true
fi
mkdir -p "${EVIDENCE_DIR:-/tmp/omatide-evidence}"
cp -a "$run/artifacts/verify/." "${EVIDENCE_DIR:-/tmp/omatide-evidence}/"
```

Kill only the pids this run recorded. Do not use `pkill` on the name
`omatide`. Evidence stays under `${EVIDENCE_DIR:-/tmp/omatide-evidence}`.
After cleanup, `test -f` the copied proof and confirm
`git status --porcelain` in the clone is what it was before the run.

On the 2026-10-14 run the copied tide transcript was still at
`/tmp/omatide-evidence/tide/station.out` after the run directory was
removed.

## Helpers

This skill adds no script. The binary, `scripts/verify.sh`, and
`omatide watch` are the helpers.

```sh
./scripts/verify.sh
env $(cat "$run/env") "$bin" watch --socket "$run/tide.sock"
```

When the commands or the lines they print change, run
`/maintain-verification-skill` over this directory.
