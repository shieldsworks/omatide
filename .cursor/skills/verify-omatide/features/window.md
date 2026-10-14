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

- **Open the window.** Run `./run.sh` inside this run's environment. Resolve `WAYLAND_DISPLAY` to an absolute path first. Then apply `$run/env`, and pass that absolute path after the file so it wins. Quickshell keeps the compositor socket. The engine binds `$run/omatide/tide.sock`.

```sh
set -euo pipefail
: "${run:?}"
case ${WAYLAND_DISPLAY:-} in
  /*) wayland_abs=$WAYLAND_DISPLAY ;;
  "") wayland_abs=/omatide-verify-no-wayland ;;
  *) wayland_abs=${XDG_RUNTIME_DIR:?}/$WAYLAND_DISPLAY ;;
esac
case $wayland_abs in
  /*) ;;
  *) printf '%s\n' 'WAYLAND_DISPLAY is not absolute' >&2; exit 1 ;;
esac
probe=$(env $(cat "$run/env") WAYLAND_DISPLAY="$wayland_abs" \
  sh -c 'printf %s "$XDG_RUNTIME_DIR"')
test "$probe" = "$run"
mkdir -p "$run/artifacts/verify/window"
set +e
env $(cat "$run/env") WAYLAND_DISPLAY="$wayland_abs" ./run.sh \
  >"$run/artifacts/verify/window/run.out" \
  2>"$run/artifacts/verify/window/run.err"
run_status=$?
set -e
printf '%s\n' "$run_status" >"$run/artifacts/verify/window/run.status"
if ! command -v quickshell >/dev/null 2>&1; then
  test "$run_status" -eq 127
fi
```

  Observed on 2026-10-14, exit 127 and stderr `./run.sh: line 7: exec: quickshell: not found`. The painted window was not driven. The probe still required `XDG_RUNTIME_DIR` to be `$run`.
- **Start the engine.** The script in `ui/Tide.qml` is `mkdir -p -m 700 "$1" && b="$2"; [ -x "$b" ] || b=omatide; exec "$b" run 2>>"$3"`. Set `bin=$(realpath ./target/release/omatide)`. Run `env $(cat "$run/env") env -C "$run/home" sh -c "$script" omatide-start "$run/omatide/" "$bin" "$run/window-engine.log"` and record that pid in `$run/window.pid`. Observed the process command `/workspace/target/release/omatide run`. The log was empty. Then `timeout 2 omatide watch` with the baseline env printed `{"tide":"0.1.0","type":"hello","v":1}` and a `state` with `"keel":"connecting"`, `"at":"home"`, and `"station":"9414290"`. Exit 124. The socket was `$run/omatide/tide.sock`.
- **Path from this file.** Run `./scripts/verify.sh` and require the pass line. Exit 0 with `Skipped.` is a failed step.

```sh
set -euo pipefail
: "${run:?}"
mkdir -p "$run/artifacts/verify/window"
./scripts/verify.sh >"$run/artifacts/verify/window/verify.out" 2>&1
grep -F 'All engine-path checks passed.' "$run/artifacts/verify/window/verify.out"
if grep -F 'Skipped.' "$run/artifacts/verify/window/verify.out"; then
  printf '%s\n' 'scripts/verify.sh skipped' >&2
  exit 1
fi
```

  Observed on 2026-10-14, after the Qt6 QML packages in `.github/workflows/ci.yml` were installed, both cases printed `ok`, then `window surfaces Tide.lastLog`, then `All engine-path checks passed.` The `repo` and `started` paths were under the temp checkout and under `plugins/org.omahoy.tide`. Exit 0.
- **Missing engine.** Run the same start script with `PATH=/usr/bin:/bin` and a binary path that is not executable. Observed exit 127 and log `omatide-start: 1: exec: omatide: not found`. That line is what the window stores as `Tide.lastLog`. After six failed connects the notice text is that line. The rectangle that draws it was not driven.
- **One default socket.** Run `omatide run` again with the baseline env while the start-script engine is still up. Observed exit 1 and stderr `omatide: omatide is already running on $run/omatide/tide.sock`.
- **Proof.** Save the `./run.sh` stderr, the start-script pid command, the watch transcript, the missing-engine log, and the `scripts/verify.sh` transcript under `$run/artifacts/verify/window/`. Do not label them as a window screenshot.

## Gotchas

- `scripts/verify.sh` prints `Qt6 QML dev packages not installed. Skipped.` and exits 0 when `pkg-config` cannot see Qt6. The path fence greps for `All engine-path checks passed.` and exits 1 when the transcript contains `Skipped.` This VM installed the CI packages so the probe could load. A Hyprland machine with Quickshell paints the window without those packages. The path fence still needs the packages, because it compiles the probe.
- The start script does not pass `--socket` or `--no-keel`. It inherits `XDG_RUNTIME_DIR` and execs `omatide run`. With this run's env, that socket is `$run/omatide/tide.sock`. `keel` stays `connecting` while omakeel's socket is absent. `engine.md` says why that transient is valid.
- The binary the script runs is `OMATIDE_BIN` when that is set, otherwise the release binary next to the QML file. `./run.sh` does not export `OMATIDE_BIN`. The path check is `scripts/verify.sh`.
- Kill the pid you recorded. A second `omatide run` on `$run/omatide/tide.sock` exits 1 and is not the one to kill.
- `quickshell` is not an Ubuntu package. Do not treat exit 127 as a product failure.
