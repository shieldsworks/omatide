---
name: verify-omatide
description: "Drives omatide the way a sailor does, through the tide, current, and bay commands, the engine socket, and the Quickshell window when it can run. Use before calling a behavior change done, when reviewing a change, or when asked to verify omatide."
---

# Verify omatide

`scripts/verify.sh` checks that the path math in `ui/Tide.qml` resolves
`target/release/omatide` for a checkout and for the plugin tree, and that
`ui/TideWindow.qml` names `Tide.lastLog`. The probe starts a stub binary.
This skill runs the real binary, drives a feature from the sailor's side,
and keeps evidence. Run both. The agent that built a change does not sign
off on this run.

## Launch

Run from the repository root. Find that root with
`git rev-parse --show-toplevel`. This proof ran in `/workspace`. A clone
at `~/Work/omahoy/omatide` is the same root when that directory is the
clone. Do not hard-code either path.

Every run gets its own directory. Do not read or write
`~/.local/share/omatide` or `~/.config/omatide`.

The shell is `set -euo pipefail`. A failed command, an unset variable, or
a failed pipe stops the run. Build with `mise exec` when `mise` is on
`PATH`. Otherwise install Rust 1.98.0 and build with `rustup run`. Delete
`target/release/omatide` before the build. The new binary's mtime has to
be at least the build start, which ties it to this build.

```sh
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
rm -f target/release/omatide
build_start=$(date +%s)
if command -v mise >/dev/null 2>&1; then
  mise install
  mise exec -- cargo build --release --locked
else
  rustup toolchain install 1.98.0
  rustup run 1.98.0 cargo build --release --locked
fi
bin=$(realpath ./target/release/omatide)
test "$(stat -c %Y "$bin")" -ge "$build_start"
run=$(mktemp -d /tmp/omatide-verify.XXXXXX)
mkdir -p "$run/home" "$run/artifacts/verify/launch" "$run/empty"
printf '%s\n' "$build_start" > "$run/artifacts/verify/launch/build_start"
stat -c %Y "$bin" > "$run/artifacts/verify/launch/bin_mtime"
cat > "$run/env" <<EOF
HOME=$run/home
XDG_RUNTIME_DIR=$run
XDG_DATA_HOME=$run/data
XDG_CONFIG_HOME=$run/config
OMATIDE_DATA_DIR=$run/data/omatide
OMATIDE_CONFIG=$run/config.toml
TZ=UTC
EOF
grep -v '^OMATIDE_DATA_DIR=' "$run/env" > "$run/env.empty"
printf 'OMATIDE_DATA_DIR=%s\n' "$run/empty" >> "$run/env.empty"
: "${run:?}"
env $(cat "$run/env") "$bin" fetch --region 37.80,-122.48,37.82,-122.45 \
  >"$run/fetch.log" 2>&1
test -f "$run/data/omatide/stations.json"
test ! -e "$run/empty/stations.json"
grep -F '2 stations, 2 of them predictable' "$run/fetch.log"
grep -F "saved to $run/data/omatide/stations.json" "$run/fetch.log"
```

`mise.toml` pins Rust 1.98. The `rustup` branch installs that toolchain.
`cargo` on `PATH` is the wrong compiler on a machine whose default
`rustc` is older. This cloud VM had no `mise` binary, and its default
`rustc` was 1.83.0. Edition 2024 needs Rust 1.85. `Cargo.toml` sets
`rust-version` to 1.89. The proof on this VM took the `rustup run` branch.

Ready when `$run/fetch.log` contains `2 stations, 2 of them predictable`
and `saved to $run/data/omatide/stations.json`, and that file exists. The
box holds NOAA station `9414290`, San Francisco at the Golden Gate, and
one current bin, `PCT0286`. Fetch needs the network once. After that
fetch, the commands below need no network.

`omatide fetch` with no `--region` uses the default bay and takes a couple
of minutes. This proof did not run that fetch. `features/bay.md` says
what the small box leaves out.

`$run/env.empty` is the no-catalog environment. It copies `$run/env` and
puts `OMATIDE_DATA_DIR=$run/empty` last, so that assignment wins. `$run/empty`
stays empty. The baseline catalog stays at `$run/data/omatide`.

The engine, for the socket features, is a second step. See
`features/engine.md`. The window's own start is `features/window.md`.

## Doctor

```sh
set -euo pipefail
: "${run:?}"
realpath ./target/release/omatide
env $(cat "$run/env") "$bin" --version
data=$(sed -n 's/^OMATIDE_DATA_DIR=//p' "$run/env")
test -f "$data/stations.json"
echo "catalog $data/stations.json"
env_lines=$(grep -c . "$run/env")
printf '%s\n' "$env_lines"
test "$env_lines" -eq 7
test "$(tail -n 1 "$run/env.empty")" = "OMATIDE_DATA_DIR=$run/empty"
test ! -e "$run/empty/stations.json"
```

The version line is `omatide 0.1.0`. The binary path is inside this
clone. The catalog path is under `$run`. The env file has 7 lines, and
every path in it is under `$run`. The last line of `$run/env.empty` is
the empty catalog. Never drive the user's catalog, config, or
`$XDG_RUNTIME_DIR/omatide/tide.sock` outside this run.

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

Write `omatide` below as these two lines. `: "${run:?}"` stops the shell
when `$run` is unset.

```sh
: "${run:?}"
env $(cat "$run/env") "$bin"
```

`$run/env` sets `OMATIDE_DATA_DIR`. An export before `env` loses, because
`env` applies the file afterward. A no-catalog command uses
`$run/env.empty` in place of `$run/env`.

- A sailor's terminal is `omatide tide`, `omatide current`, `omatide bay`,
  `omatide stations`, `omatide watch`, and `omatide fetch`.
- The engine is `omatide run`, then `omatide watch` on the same socket.
  `docs/protocol.md` is the line format. `keel` also takes the transient
  value `connecting` before omakeel answers. See `features/engine.md`.
- The window is `./run.sh`, which execs `quickshell -p ui/shell.qml`. Run
  it with `$run/env` and an absolute `WAYLAND_DISPLAY`, as
  `features/window.md` says, so the engine socket stays under `$run`.
  The window starts `omatide run` itself when the socket is down.
  Quickshell on a Hyprland session is required to paint it. When
  `quickshell` is not on `PATH`, do not skip `features/window.md`. Drive
  that file and say the painted window was not driven.

`timeout` ends `omatide watch` with exit 124 while the engine is still
running. Record that exit and judge the output file.

`scripts/verify.sh` resolves `target/release/omatide` from the QML file's
directory and from the plugin directory, then checks that
`ui/TideWindow.qml` names `Tide.lastLog`. On 2026-10-14, before the Qt6
packages were installed, it printed `Qt6 QML dev packages not installed.
Skipped.` and exited 0. A skip leaves the path check unproved. The pass
text is `All engine-path checks passed.` The fence in `features/window.md`
greps for that line, so a skip fails the step. After the packages named
in `.github/workflows/ci.yml` were installed, this VM printed that line.

## Evidence

Write under `$run/artifacts/verify/<feature>/`. Cleanup copies that tree
to `${EVIDENCE_DIR:-/tmp/omatide-evidence}/$(basename "$run")` and then
deletes `$run`. Each run has its own evidence directory.

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
set -euo pipefail
: "${run:?}"
evidence=${EVIDENCE_DIR:-/tmp/omatide-evidence}/$(basename "$run")
if test -d "$run/artifacts/verify"; then
  mkdir -p "$evidence"
  cp -a "$run/artifacts/verify/." "$evidence/"
fi
for pidfile in "$run/pid" "$run/empty.pid" "$run/window.pid"; do
  if test -f "$pidfile"; then
    kill "$(cat "$pidfile")" 2>/dev/null || true
  fi
done
rm -rf "$run"
test ! -d "$run"
test -d "$evidence"
```

Kill only the pids this run recorded. That includes the empty-catalog
engine in `$run/empty.pid`. Do not use `pkill` on the name `omatide`.
Evidence stays in the per-run directory. `$run` is gone. A second cleanup
finds no `$run` artifacts to copy and leaves the evidence directory in
place. After cleanup, `test -f` the copied proof and confirm
`git status --porcelain` in the clone is what it was before the run.

## Helpers

This skill adds no script. The binary, `scripts/verify.sh`, and
`omatide watch` are the helpers. The window probe that greps for the pass
line is the fence in `features/window.md`.

```sh
set -euo pipefail
: "${run:?}"
set +e
timeout 2 env $(cat "$run/env") "$bin" watch --socket "$run/tide.sock"
watch_status=$?
set -e
test "$watch_status" -eq 124
```

When the commands or the lines they print change, run
`/maintain-verification-skill` over this directory.
