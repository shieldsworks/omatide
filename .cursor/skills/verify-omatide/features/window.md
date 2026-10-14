# The window

`./run.sh` opens the tide in its own Quickshell process. The window starts
`omatide run` from this clone's release binary when the socket is down.
If that process never answers, the window shows the last engine log line
instead of a blank chart.

## Sub-features

- `window-launch` `./run.sh` opens the window under Quickshell.
- `window-start` the window's start script runs this clone's release binary, and `omatide watch` prints `hello`.
- `window-error` a missing engine is named in the log, which is the notice text, and the chart is not driven.
- `window-path` `scripts/verify.sh` resolves the binary from this file's directory and from the plugin tree.

## How to get to it (user POV)

- Run `./run.sh` from the clone. It execs `quickshell -p ui/shell.qml`.
- The Omarchy bar loads `ui/TideBar.qml`. The panel loads `ui/Panel.qml`. Both talk to the same engine socket.
- Keys, once the window is painted. `n` toggles night. `[` and `]` step the bay an hour. `0` returns to now. `+` and `-` zoom. `q` or Escape closes.
- `quickshell ipc -p ui/shell.qml call omatide status` prints the window's engine state. `call omatide press n` is the `n` key.

## Driving it with the start script

Preconditions.

- The baseline from `README.md`, plus a release binary at `target/release/omatide`.
- Stop any engine already listening on `$run/omatide/tide.sock` before **Start the engine**.
- Quickshell and Hyprland are required for the painted window. This VM has neither. `window-launch`, the keys, the IPC call, and the painted notice were not driven. The other steps are the substitutes that were run. They are not a screenshot.

- **Open the window.** Run `./run.sh` inside this run's environment. An empty `WAYLAND_DISPLAY` is unreachable. A relative name is the socket under the current `XDG_RUNTIME_DIR` when that directory is absolute. An absolute path is that path. A real socket is passed through as an absolute path, after `$run/env`, so the engine still uses `$run`. A missing socket is written to `wayland.txt`. `./run.sh` is not given a stand-in path. Unreachable returns non-zero and does not run `./run.sh`. The probe still requires `XDG_RUNTIME_DIR` to be `$run`. The env file is loaded into an array. The process is started in the background. Still alive after three seconds is the pass, the same shape as `omatide_watch`. The fence records 124, saves the process command, then stops the pid. An exit before that deadline fails. That includes 0, a crash, and 127.

```sh
omatide_window_open() {
  local _opts _status display runtime socket probe run_status pid i
  local -a _env
  omatide_fence_save _opts
  _status=0
  wayland_abs=
  test -n "${run-}" || _status=1
  if test "$_status" -eq 0; then
    mkdir -p "$run/artifacts/verify/window" || _status=$?
  fi
  if test "$_status" -eq 0; then
    display=${WAYLAND_DISPLAY-}
    runtime=${XDG_RUNTIME_DIR-}
    socket=
    case "$display" in
      "")
        printf '%s\n' 'WAYLAND_DISPLAY is empty' 'not run: ./run.sh' \
          > "$run/artifacts/verify/window/wayland.txt" || _status=$?
        ;;
      /*)
        socket=$display
        ;;
      *)
        case "$runtime" in
          /*) socket=$runtime/$display ;;
          *)
            printf '%s\n' 'XDG_RUNTIME_DIR is not absolute' 'not run: ./run.sh' \
              > "$run/artifacts/verify/window/wayland.txt" || _status=$?
            ;;
        esac
        ;;
    esac
  fi
  if test "$_status" -eq 0 && test -n "${socket-}"; then
    if test -S "$socket"; then
      wayland_abs=$socket
      printf '%s\n' "$wayland_abs" > "$run/artifacts/verify/window/wayland.txt" || _status=$?
    else
      printf '%s\n' "wayland socket is not reachable: $socket" 'not run: ./run.sh' \
        > "$run/artifacts/verify/window/wayland.txt" || _status=$?
    fi
  fi
  if test "$_status" -eq 0; then
    test -n "${run-}" && omatide_env_load _env "$run/env" || _status=$?
  fi
  if test "$_status" -eq 0; then
    probe=$(env "${_env[@]}" sh -c 'printf %s "$XDG_RUNTIME_DIR"') || _status=$?
  fi
  if test "$_status" -eq 0; then
    test "$probe" = "$run" || _status=$?
  fi
  if test "$_status" -eq 0 && test -z "${wayland_abs-}"; then
    _status=1
  fi
  if test "$_status" -eq 0 && test -n "${wayland_abs-}"; then
    _env+=("WAYLAND_DISPLAY=$wayland_abs")
    env "${_env[@]}" ./run.sh \
      >"$run/artifacts/verify/window/run.out" \
      2>"$run/artifacts/verify/window/run.err" &
    pid=$!
    omatide_fence_pid "$pid"
    i=0
    while test "$i" -lt 30; do
      if ! kill -0 "$pid" 2>/dev/null; then
        break
      fi
      sleep 0.1 || true
      i=$((i + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
      ps -o args= -p "$pid" > "$run/artifacts/verify/window/run.cmd" || _status=$?
      printf '%s\n' 124 > "$run/artifacts/verify/window/run.status" || _status=$?
      omatide_kill "$pid"
      wait "$pid" || true
    else
      wait "$pid"
      run_status=$?
      printf '%s\n' "$run_status" > "$run/artifacts/verify/window/run.status" || _status=$?
      _status=1
    fi
  fi
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}
omatide_window_open
```

  Observed on 2026-10-14, with Quickshell absent and `WAYLAND_DISPLAY` empty, the fence returned 1. `wayland.txt` said `WAYLAND_DISPLAY is empty` and `not run: ./run.sh`. The painted window was not driven. The probe still required `XDG_RUNTIME_DIR` to be `$run`. A process that exits before three seconds fails, including exit 0, a crash, and 127 when `quickshell` is absent. A process still alive at that deadline is recorded as 124 and then stopped.
- **Start the engine.** The script in `ui/Tide.qml` is `mkdir -p -m 700 "$1" && b="$2"; [ -x "$b" ] || b=omatide; exec "$b" run 2>>"$3"`. Tide's log is `$run/omatide/engine.log` (`runtime + "engine.log"`). The fence passes that path as `$3`. It starts the script in the background, checks the pid and `$run/omatide/tide.sock`, records a `timeout 2` watch, checks a second `omatide run` exits 1, then stops the pid. It does not wait in the foreground.

```sh
omatide_window_start() {
  local _opts _status script i watch_status second_status pid
  local -a _env
  omatide_fence_save _opts
  _status=0
  script='mkdir -p -m 700 "$1" && b="$2"; [ -x "$b" ] || b=omatide; exec "$b" run 2>>"$3"'
  test -n "${run-}" && test -n "${bin-}" && omatide_env_load _env "$run/env" || _status=1
  if test "$_status" -eq 0; then
    mkdir -p "$run/artifacts/verify/window" || _status=$?
  fi
  if test "$_status" -eq 0; then
    env "${_env[@]}" env -C "$run/home" sh -c "$script" omatide-start \
      "$run/omatide/" "$bin" "$run/omatide/engine.log" \
      >"$run/artifacts/verify/window/start.out" \
      2>"$run/artifacts/verify/window/start.err" &
    pid=$!
    omatide_fence_pid "$pid"
    echo "$pid" > "$run/window.pid"
    i=0
    while test "$i" -lt 25; do
      if test -S "$run/omatide/tide.sock" && kill -0 "$pid" 2>/dev/null; then
        break
      fi
      sleep 0.2 || true
      i=$((i + 1))
    done
    if ! test -S "$run/omatide/tide.sock" || ! kill -0 "$pid" 2>/dev/null; then
      _status=1
    fi
  fi
  if test "$_status" -eq 0; then
    ps -o args= -p "$pid" > "$run/artifacts/verify/window/start.cmd" || _status=$?
  fi
  if test "$_status" -eq 0; then
    timeout 2 env "${_env[@]}" "$bin" watch --socket "$run/omatide/tide.sock" \
      >"$run/artifacts/verify/window/watch.out" \
      2>"$run/artifacts/verify/window/watch.err"
    watch_status=$?
    printf '%s\n' "$watch_status" > "$run/artifacts/verify/window/watch.status"
    test "$watch_status" -eq 124 || _status=$watch_status
  fi
  if test "$_status" -eq 0; then
    grep -F '{"tide":"0.1.0","type":"hello","v":1}' \
      "$run/artifacts/verify/window/watch.out" || _status=$?
  fi
  if test "$_status" -eq 0; then
    grep -F '"keel":"connecting"' "$run/artifacts/verify/window/watch.out" || _status=$?
  fi
  if test "$_status" -eq 0; then
    env "${_env[@]}" "$bin" run \
      >"$run/artifacts/verify/window/second.out" \
      2>"$run/artifacts/verify/window/second.err"
    second_status=$?
    printf '%s\n' "$second_status" > "$run/artifacts/verify/window/second.status"
    test "$second_status" -eq 1 || _status=1
  fi
  if test "$_status" -eq 0; then
    grep -F "already running on $run/omatide/tide.sock" \
      "$run/artifacts/verify/window/second.err" || _status=$?
  fi
  if test -n "${pid-}"; then
    omatide_kill "$pid"
  fi
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}
omatide_window_start
```

  Observed on 2026-10-14 the process command was `/workspace/target/release/omatide run`. The log was empty. The watch printed `{"tide":"0.1.0","type":"hello","v":1}` and a `state` with `"keel":"connecting"`, `"at":"home"`, and `"station":"9414290"`. Exit 124. The socket was `$run/omatide/tide.sock`. A second `omatide run` exited 1 with `omatide: omatide is already running on $run/omatide/tide.sock`.
- **Path from this file.** Run `./scripts/verify.sh` and require the pass line. Exit 0 with `Skipped.` is a failed step.

```sh
omatide_window_path() {
  local _opts _status
  omatide_fence_save _opts
  _status=0
  test -n "${run-}" || _status=1
  if test "$_status" -eq 0; then
    mkdir -p "$run/artifacts/verify/window" || _status=$?
  fi
  if test "$_status" -eq 0; then
    ./scripts/verify.sh >"$run/artifacts/verify/window/verify.out" 2>&1 || _status=$?
  fi
  if test "$_status" -eq 0; then
    grep -F 'All engine-path checks passed.' \
      "$run/artifacts/verify/window/verify.out" || _status=$?
  fi
  if test "$_status" -eq 0 && grep -F 'Skipped.' "$run/artifacts/verify/window/verify.out"; then
    printf '%s\n' 'scripts/verify.sh skipped' >&2
    _status=1
  fi
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}
omatide_window_path
```

  Observed on 2026-10-14, after the Qt6 QML packages in `.github/workflows/ci.yml` were installed, both cases printed `ok`, then `window surfaces Tide.lastLog`, then `All engine-path checks passed.` The `repo` and `started` paths were under the temp checkout and under `plugins/org.omahoy.tide`. Exit 0.
- **Missing engine.** Run the same start script with `PATH=/usr/bin:/bin` and a binary path that is not executable. The log argument is `$run/omatide/engine.log`, the file Tide reads. Observed exit 127 and log `omatide-start: 1: exec: omatide: not found`. That line is what the window stores as `Tide.lastLog`. A missing log fails the fence. After six failed connects the notice text is that line. The rectangle that draws it was not driven.

```sh
omatide_window_missing() {
  local _opts _status script miss
  local -a _env
  omatide_fence_save _opts
  _status=0
  script='mkdir -p -m 700 "$1" && b="$2"; [ -x "$b" ] || b=omatide; exec "$b" run 2>>"$3"'
  test -n "${run-}" && omatide_env_load _env "$run/env" || _status=1
  if test "$_status" -eq 0; then
    mkdir -p "$run/artifacts/verify/window" || _status=$?
  fi
  if test "$_status" -eq 0; then
    env "${_env[@]}" env -C "$run/home" env PATH=/usr/bin:/bin \
      sh -c "$script" omatide-start "$run/omatide/" "$run/not-a-binary" \
      "$run/omatide/engine.log"
    miss=$?
    printf '%s\n' "$miss" > "$run/artifacts/verify/window/missing.status"
    test "$miss" -eq 127 || _status=1
  fi
  if test "$_status" -eq 0; then
    test -f "$run/omatide/engine.log" || _status=1
  fi
  if test "$_status" -eq 0; then
    grep -F 'exec: omatide: not found' \
      "$run/omatide/engine.log" || _status=$?
  fi
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}
omatide_window_missing
```
- **One default socket.** `omatide_window_start` runs a second `omatide run` while its engine is still up, then stops that pid. Observed exit 1 and stderr `omatide: omatide is already running on $run/omatide/tide.sock`.
- **Proof.** Save the `./run.sh` stderr, the start-script pid command, the watch transcript, the missing-engine log, and the `scripts/verify.sh` transcript under `$run/artifacts/verify/window/`. Do not label them as a window screenshot.

## Gotchas

- `scripts/verify.sh` prints `Qt6 QML dev packages not installed. Skipped.` and exits 0 when `pkg-config` cannot see Qt6. The path fence greps for `All engine-path checks passed.` and returns 1 when the transcript contains `Skipped.` This VM installed the CI packages so the probe could load. A Hyprland machine with Quickshell paints the window without those packages. The path fence still needs the packages, because it compiles the probe.
- The start script does not pass `--socket` or `--no-keel`. It inherits `XDG_RUNTIME_DIR` and execs `omatide run`. With this run's env, that socket is `$run/omatide/tide.sock`. `keel` stays `connecting` while omakeel's socket is absent. `engine.md` says why that transient is valid.
- The binary the script runs is `OMATIDE_BIN` when that is set, otherwise the release binary next to the QML file. `./run.sh` does not export `OMATIDE_BIN`. The path check is `scripts/verify.sh`.
- Kill the pid you recorded. A second `omatide run` on `$run/omatide/tide.sock` exits 1 and is not the one to kill.
- `quickshell` is not an Ubuntu package. `./run.sh` exits 127 before the deadline when it is absent. That is a failed open, recorded as a non-pass. It is not a product bug in the chart.
