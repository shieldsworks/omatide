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

Paste one fence at a time into the interactive shell you already have
open. Run `set +e` first. `omatide_launch` returns 1 without building
when errexit is on, and tells you to run `set +e`. A failing fence
returns 1. With errexit still on, that return exits this shell.

A fence is a function. `omatide_fence_save` records `$-` in that shell,
then captures `set +o`. A command substitution reports errexit as off, so
the saved text is corrected from `$-` before the body runs. The letter
`e` is checked one character at a time, so errtrace's `E` does not count.
The body turns nounset and pipefail on, and turns errexit off. With
errexit on, a failed command exits this interactive shell.
`omatide_fence_finish` evals the saved options, so errexit, nounset, and
pipefail come back as they were. The function returns the status. It does
not call `exit`.

`${run:?}` returns from the function before `omatide_fence_finish` runs,
and the strict options stay on. Do not use it. `omatide_env_ok` is the
guard. It requires `$run`, a readable non-empty env file, and one
assignment each for `HOME`, `XDG_RUNTIME_DIR`, `XDG_DATA_HOME`,
`XDG_CONFIG_HOME`, `OMATIDE_DATA_DIR`, `OMATIDE_CONFIG`, and `TZ`. Each
path is absolute, has no glob character, and `readlink -f` keeps it under
`$run`. `omatide_env_exec` reads the file into an array and passes that
array to `env`, so the shell does not split or glob the lines. When the
guard fails, the binary does not run, so it cannot fall through to
`~/.local/share/omatide`.

A fence that puts a process in the background calls `omatide_fence_pid`,
which appends the pid to `OMATIDE_FENCE_PIDS`. On a non-zero status,
`omatide_fence_finish` kills every pid in that list. Cleanup kills the
pid files after that, then unsets `run` and `bin`.

Build with `mise exec` when `mise` is on `PATH`. Otherwise install Rust
1.98.1 and build with `rustup run 1.98.1`. `mise.toml` names Rust `1.98`,
and that channel is 1.98.1 (`48a229cea 2026-09-01`). Rust 1.98.0
(`88d9e12ae 2026-08-18`) is a different compiler. Both arms require
`rustc --version` to contain `1.98.1`. Record the build start, `touch
src/main.rs`, then delete `target/release/omatide` and build. The touch
forces Cargo to relink this package, so a stub left on the old deps inode
is not what gets published. The new binary's mtime has to be at least the
build start, and `omatide --version` has to print `omatide 0.1.0`. A
failed launch unsets `run` when that path is not a directory.

`cargo` on `PATH` is the wrong compiler on a machine whose default
`rustc` is older. This cloud VM had no `mise` binary, and its default
`rustc` was 1.83.0. Edition 2024 needs Rust 1.85. `Cargo.toml` sets
`rust-version` to 1.89. The proof on this VM took the `rustup run` arm.

```sh
omatide_kill() {
  local pid="$1" i
  test -n "${pid-}" || return 0
  kill "$pid" 2>/dev/null || true
  i=0
  while test "$i" -lt 20 && kill -0 "$pid" 2>/dev/null; do
    sleep 0.1 || true
    i=$((i + 1))
  done
  if kill -0 "$pid" 2>/dev/null; then
    kill -9 "$pid" 2>/dev/null || true
  fi
  return 0
}

omatide_fence_finish() {
  local status="$1"
  local opts="$2"
  if test "$status" -ne 0; then
    local pid
    for pid in ${OMATIDE_FENCE_PIDS-}; do
      omatide_kill "$pid"
    done
  fi
  unset OMATIDE_FENCE_PIDS
  eval "$opts"
}

omatide_dash_errexit() {
  local rest="${1-$-}" ch
  while test -n "$rest"; do
    ch=${rest%"${rest#?}"}
    rest=${rest#?}
    if test "$ch" = e; then
      return 0
    fi
  done
  return 1
}

omatide_fence_save() {
  local -n _slot=$1
  local _dash=$- _raw
  _raw=$(set +o)
  if omatide_dash_errexit "$_dash"; then
    _raw=${_raw/set +o errexit/set -o errexit}
  fi
  _slot=$_raw
  set -euo pipefail
  set +e
}

omatide_fence_pid() {
  local pid="$1"
  test -n "${pid-}" || return 1
  case ${OMATIDE_FENCE_PIDS-} in
    "") OMATIDE_FENCE_PIDS=$pid ;;
    *) OMATIDE_FENCE_PIDS="$OMATIDE_FENCE_PIDS $pid" ;;
  esac
}

omatide_env_ok() {
  local file="$1" key line val n
  test -n "${run-}" || return 1
  test -n "${file-}" || return 1
  test -r "$file" || return 1
  test -s "$file" || return 1
  while IFS= read -r line || test -n "$line"; do
    case "$line" in
      HOME=*|XDG_RUNTIME_DIR=*|XDG_DATA_HOME=*|XDG_CONFIG_HOME=*|OMATIDE_DATA_DIR=*|OMATIDE_CONFIG=*|TZ=*) ;;
      *) return 1 ;;
    esac
  done < "$file"
  for key in HOME XDG_RUNTIME_DIR XDG_DATA_HOME XDG_CONFIG_HOME OMATIDE_DATA_DIR OMATIDE_CONFIG TZ; do
    n=$(grep -c "^${key}=" "$file") || n=0
    test "$n" -eq 1 || return 1
    val=$(sed -n "s/^${key}=//p" "$file") || return 1
    test -n "$val" || return 1
    case "$val" in
      *[[:space:]]*|*\**|*\?*|*\[*|*\]*) return 1 ;;
    esac
    case "$key" in
      TZ) ;;
      *)
        omatide_path_under_run "$val" || return 1
        ;;
    esac
  done
  return 0
}

omatide_path_under_run() {
  local val="$1" cur rest resolved run_real
  test -n "${run-}" || return 1
  test -d "$run" || return 1
  case "$val" in
    /*) ;;
    *) return 1 ;;
  esac
  case "$val" in
    */..|*/../*|../*) return 1 ;;
  esac
  run_real=$(readlink -f "$run") || return 1
  cur=$val
  rest=
  while test ! -e "$cur" && test "$cur" != /; do
    rest=$(basename "$cur")${rest:+/$rest}
    cur=$(dirname "$cur")
  done
  resolved=$(readlink -f "$cur") || return 1
  if test -n "$rest"; then
    resolved=$resolved/$rest
  fi
  case "$resolved" in
    "$run_real"|"$run_real"/*) return 0 ;;
    *) return 1 ;;
  esac
}

omatide_env_load() {
  local -n _out=$1
  local file=$2 line
  omatide_env_ok "$file" || return 1
  _out=()
  while IFS= read -r line || test -n "${line-}"; do
    _out+=("$line")
  done < "$file"
}

omatide_env_exec() {
  local file=$1
  shift
  local -a _args
  omatide_env_load _args "$file" || return 1
  env "${_args[@]}" "$@"
}

omatide_launch() {
  local _opts _status compiler pin fresh ver mtime
  pin=1.98.1
  if omatide_dash_errexit; then
    printf '%s\n' 'errexit is on. Run set +e before this fence.' >&2
    return 1
  fi
  omatide_fence_save _opts
  _status=0
  compiler=
  fresh=
  if test -n "${run-}" && ! test -d "$run"; then
    unset run
  fi
  cd "$(git rev-parse --show-toplevel)" || _status=$?
  if test "$_status" -eq 0; then
    build_start=$(date +%s) || _status=$?
  fi
  if test "$_status" -eq 0; then
    touch src/main.rs || _status=$?
  fi
  if test "$_status" -eq 0; then
    rm -f target/release/omatide || _status=$?
  fi
  if test "$_status" -eq 0; then
    if command -v mise >/dev/null 2>&1; then
      mise install || _status=$?
      if test "$_status" -eq 0; then
        mise exec -- cargo build --release --locked || _status=$?
      fi
      if test "$_status" -eq 0; then
        compiler=$(mise exec -- rustc --version) || _status=$?
      fi
    else
      rustup toolchain install "$pin" || _status=$?
      if test "$_status" -eq 0; then
        rustup run "$pin" cargo build --release --locked || _status=$?
      fi
      if test "$_status" -eq 0; then
        compiler=$(rustup run "$pin" rustc --version) || _status=$?
      fi
    fi
  fi
  if test "$_status" -eq 0; then
    case "$compiler" in
      *"$pin"*) ;;
      *) printf '%s\n' "compiler is $compiler, want $pin" >&2; _status=1 ;;
    esac
  fi
  if test "$_status" -eq 0; then
    bin=$(realpath ./target/release/omatide) || _status=$?
  fi
  if test "$_status" -eq 0; then
    ver=$(env -i "$bin" --version) || _status=$?
  fi
  if test "$_status" -eq 0; then
    test "$ver" = "omatide 0.1.0" || _status=1
  fi
  if test "$_status" -eq 0; then
    mtime=$(stat -c %Y "$bin") || _status=$?
  fi
  if test "$_status" -eq 0; then
    if test "$mtime" -ge "$build_start"; then
      fresh="mtime $mtime"
    else
      printf '%s\n' "release binary mtime $mtime is older than build start $build_start" >&2
      _status=1
    fi
  fi
  if test "$_status" -eq 0; then
    run=$(mktemp -d /tmp/omatide-verify.XXXXXX) || _status=$?
  fi
  if test "$_status" -eq 0; then
    mkdir -p "$run/home" "$run/artifacts/verify/launch" "$run/empty" || _status=$?
  fi
  if test "$_status" -eq 0; then
    printf '%s\n' "$build_start" > "$run/artifacts/verify/launch/build_start" || _status=$?
  fi
  if test "$_status" -eq 0; then
    stat -c %Y "$bin" > "$run/artifacts/verify/launch/bin_mtime" || _status=$?
  fi
  if test "$_status" -eq 0; then
    printf '%s\n' "$fresh" > "$run/artifacts/verify/launch/fresh.txt" || _status=$?
  fi
  if test "$_status" -eq 0; then
    printf '%s\n' "$compiler" > "$run/artifacts/verify/launch/rustc.txt" || _status=$?
  fi
  if test "$_status" -eq 0; then
    cat > "$run/env" <<EOF
HOME=$run/home
XDG_RUNTIME_DIR=$run
XDG_DATA_HOME=$run/data
XDG_CONFIG_HOME=$run/config
OMATIDE_DATA_DIR=$run/data/omatide
OMATIDE_CONFIG=$run/config.toml
TZ=UTC
EOF
    _status=$?
  fi
  if test "$_status" -eq 0; then
    grep -v '^OMATIDE_DATA_DIR=' "$run/env" > "$run/env.empty" || _status=$?
  fi
  if test "$_status" -eq 0; then
    printf 'OMATIDE_DATA_DIR=%s\n' "$run/empty" >> "$run/env.empty" || _status=$?
  fi
  if test "$_status" -eq 0; then
    test -n "${run-}" && test -n "${bin-}" \
      && omatide_env_exec "$run/env" "$bin" fetch --region 37.80,-122.48,37.82,-122.45 \
      >"$run/fetch.log" 2>&1 || _status=$?
  fi
  if test "$_status" -eq 0; then
    test -f "$run/data/omatide/stations.json" || _status=$?
  fi
  if test "$_status" -eq 0; then
    test ! -e "$run/empty/stations.json" || _status=$?
  fi
  if test "$_status" -eq 0; then
    grep -F '2 stations, 2 of them predictable' "$run/fetch.log" || _status=$?
  fi
  if test "$_status" -eq 0; then
    grep -F "saved to $run/data/omatide/stations.json" "$run/fetch.log" || _status=$?
  fi
  omatide_fence_finish "$_status" "$_opts"
  if test "$_status" -ne 0 && test -n "${run-}" && ! test -d "$run"; then
    unset run
  fi
  return "$_status"
}
omatide_launch
```

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
omatide_doctor() {
  local _opts _status data env_lines
  omatide_fence_save _opts
  _status=0
  test -n "${run-}" && test -n "${bin-}" && realpath ./target/release/omatide || _status=$?
  if test "$_status" -eq 0; then
    test -n "${run-}" && omatide_env_exec "$run/env" "$bin" --version || _status=$?
  fi
  if test "$_status" -eq 0; then
    data=$(sed -n 's/^OMATIDE_DATA_DIR=//p' "$run/env") || _status=$?
  fi
  if test "$_status" -eq 0; then
    test -f "$data/stations.json" || _status=$?
  fi
  if test "$_status" -eq 0; then
    printf '%s\n' "catalog $data/stations.json"
    env_lines=$(grep -c . "$run/env") || _status=$?
  fi
  if test "$_status" -eq 0; then
    printf '%s\n' "$env_lines"
    test "$env_lines" -eq 7 || _status=$?
  fi
  if test "$_status" -eq 0; then
    test "$(tail -n 1 "$run/env.empty")" = "OMATIDE_DATA_DIR=$run/empty" || _status=$?
  fi
  if test "$_status" -eq 0; then
    test ! -e "$run/empty/stations.json" || _status=$?
  fi
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}
omatide_doctor
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

## Helpers

`omatide_fence_finish` is in the launch fence. Define these next.
`omatide_stop` and `omatide_watch` have to exist before cleanup.
Cleanup calls `omatide_stop`.

```sh
omatide_env() {
  local _opts _status
  omatide_fence_save _opts
  _status=0
  test -n "${run-}" && test -n "${bin-}" \
    && omatide_env_exec "$run/env" "$bin" "$@" || _status=$?
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}

omatide_env_empty() {
  local _opts _status
  omatide_fence_save _opts
  _status=0
  test -n "${run-}" && test -n "${bin-}" \
    && omatide_env_exec "$run/env.empty" "$bin" "$@" || _status=$?
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}

omatide_stop() {
  local pidfile pid
  test -n "${run-}" || return 1
  for pidfile in "$run/pid" "$run/empty.pid" "$run/window.pid"; do
    if test -f "$pidfile"; then
      pid=$(cat "$pidfile") || true
      if test -n "${pid-}"; then
        omatide_kill "$pid"
      fi
    fi
  done
  return 0
}

omatide_watch() {
  local _opts _status socket out err watch_status
  local -a _env
  socket=${1-}
  out=${2-}
  err=${3-}
  omatide_fence_save _opts
  _status=0
  test -n "${run-}" && test -n "${bin-}" && test -n "${socket-}" && test -n "${out-}" && test -n "${err-}" \
    && omatide_env_load _env "$run/env" || _status=1
  if test "$_status" -eq 0; then
    timeout 2 env "${_env[@]}" "$bin" watch --socket "$socket" >"$out" 2>"$err"
    watch_status=$?
    printf '%s\n' "$watch_status" > "$out.status"
    test "$watch_status" -eq 124 || _status=$watch_status
  fi
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}
```

`timeout` ends `omatide watch` with exit 124 while the engine is still
running. `omatide_watch` records that exit in `$out.status` and returns
0 when the exit is 124. Judge the output file.

This skill adds no script. The binary, `scripts/verify.sh`, and
`omatide watch` are the helpers. The window probe that greps for the pass
line is the fence in `features/window.md`.

When the commands or the lines they print change, run
`/maintain-verification-skill` over this directory.

## Drive

Follow `features/`. Start at `features/README.md`. A proof that drives one
convenient entry point is incomplete when the map lists others.

Write `omatide` below as `omatide_env`. A no-catalog command is
`omatide_env_empty`. Both refuse to run the binary when `$run` or `$bin`
is empty.

```sh
omatide_env tide
```

`$run/env` sets `OMATIDE_DATA_DIR`. An export before `env` loses, because
`env` applies the file afterward.

- A sailor's terminal is `omatide tide`, `omatide current`, `omatide bay`,
  `omatide stations`, `omatide watch`, and `omatide fetch`.
- The engine is `omatide run`, then `omatide watch` on the same socket.
  `docs/protocol.md` is the line format. `keel` also takes the transient
  value `connecting` before omakeel answers. See `features/engine.md`.
- The window is `./run.sh`, which execs `quickshell -p ui/shell.qml`. Run
  it with `$run/env` and a real compositor socket, as `features/window.md`
  says, so the engine socket stays under `$run`. Still alive after three
  seconds is the pass, recorded as 124, then the process is stopped. An
  exit before that deadline fails. The window starts `omatide run` itself
  when the socket is down, and Tide reads `$run/omatide/engine.log`. A
  missing log fails that fence. Quickshell on a Hyprland session is
  required to paint it. When `quickshell` is not on `PATH`, do not skip
  `features/window.md`. Drive that file and say the painted window was
  not driven.

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
to `${EVIDENCE_DIR:-/tmp/omatide-evidence}/$(basename "$run")`. It also
copies `$run/fetch.log`, `$run/data/omatide/stations.json`,
`$run/engine.log`, and `$run/omatide/engine.log` into that evidence
directory when those files exist. Tide reads the last of those. Then it
deletes `$run` and unsets
`run` and `bin`. Each run has its own evidence directory.

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

Call `omatide_cleanup` after the feature files. It uses `omatide_stop`
from Helpers. Pasting it earlier deletes `$run` while the watch helper
still needs that directory. After it returns, `run` and `bin` are unset.

```sh
omatide_cleanup() {
  local _opts _status
  omatide_fence_save _opts
  _status=0
  test -n "${run-}" || _status=1
  if test "$_status" -eq 0; then
    evidence=${EVIDENCE_DIR:-/tmp/omatide-evidence}/$(basename "$run")
    mkdir -p "$evidence" || _status=$?
  fi
  if test "$_status" -eq 0 && test -d "$run/artifacts/verify"; then
    cp -a "$run/artifacts/verify/." "$evidence/" || _status=$?
  fi
  if test "$_status" -eq 0 && test -f "$run/fetch.log"; then
    mkdir -p "$evidence/launch" || _status=$?
    if test "$_status" -eq 0; then
      cp -a "$run/fetch.log" "$evidence/launch/fetch.log" || _status=$?
    fi
  fi
  if test "$_status" -eq 0 && test -f "$run/data/omatide/stations.json"; then
    mkdir -p "$evidence/launch" || _status=$?
    if test "$_status" -eq 0; then
      cp -a "$run/data/omatide/stations.json" "$evidence/launch/stations.json" || _status=$?
    fi
  fi
  if test "$_status" -eq 0 && test -f "$run/engine.log"; then
    mkdir -p "$evidence/engine" || _status=$?
    if test "$_status" -eq 0; then
      cp -a "$run/engine.log" "$evidence/engine/engine.log" || _status=$?
    fi
  fi
  if test "$_status" -eq 0 && test -f "$run/omatide/engine.log"; then
    mkdir -p "$evidence/window" || _status=$?
    if test "$_status" -eq 0; then
      cp -a "$run/omatide/engine.log" "$evidence/window/engine.log" || _status=$?
    fi
  fi
  if test "$_status" -eq 0; then
    omatide_stop || _status=$?
  fi
  if test "$_status" -eq 0; then
    rm -rf "$run" || _status=$?
  fi
  if test "$_status" -eq 0; then
    test ! -d "$run" || _status=$?
  fi
  if test "$_status" -eq 0; then
    unset run bin
  fi
  if test "$_status" -eq 0; then
    test -d "$evidence" || _status=$?
  fi
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}
```

Call `omatide_cleanup` after the feature files.

Kill only the pids this run recorded. That includes the empty-catalog
engine in `$run/empty.pid`. Do not use `pkill` on the name `omatide`.
Evidence stays in the per-run directory. `$run` is gone. A second cleanup
finds no `$run` artifacts to copy and leaves the evidence directory in
place. After cleanup, `test -f` the copied proof and confirm
`git status --porcelain` in the clone is what it was before the run.
