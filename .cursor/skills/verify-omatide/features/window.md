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

- **Open the window.** The drive runs `./run.sh` inside this run's environment. An empty `WAYLAND_DISPLAY` is unreachable. A relative name is the socket under the current `XDG_RUNTIME_DIR` when that directory is absolute. An absolute path is that path. A real socket is passed through as an absolute path, after `$run/env`, so the engine still uses `$run`. A missing socket is written to `wayland.txt`. `./run.sh` is not given a stand-in path. Unreachable does not run `./run.sh` and is recorded as `window-open: not-driven`. The probe still requires `XDG_RUNTIME_DIR` to be `$run`. The env file is loaded into an array. The process is started in its own process group. Still alive after three seconds is the pass, recorded as 124. The drive saves the process command, stops the group, and requires every pid it started to be gone and `$run/omatide/tide.sock` to be free. An exit before that deadline fails the drive. That includes 0, a crash, and 127.


  Observed on 2026-10-14, with Quickshell absent and `WAYLAND_DISPLAY` empty, the drive printed `window-open: not-driven`. `wayland.txt` said `WAYLAND_DISPLAY is empty` and `not run: ./run.sh`. The painted window was not driven. The probe still required `XDG_RUNTIME_DIR` to be `$run`. A process that exits before three seconds fails the drive, including exit 0, a crash, and 127 when `quickshell` is absent. A process still alive at that deadline is recorded as 124, then the process group is stopped.
- **Start the engine.** The script in `ui/Tide.qml` is `mkdir -p -m 700 "$1" && b="$2"; [ -x "$b" ] || b=omatide; exec "$b" run 2>>"$3"`. Tide's log is `$run/omatide/engine.log` (`runtime + "engine.log"`). The drive passes that path as `$3`. It starts the script in its own process group, checks the pid and `$run/omatide/tide.sock`, records a `timeout 2` watch, checks a second `omatide run` exits 1, then stops the group. After the stop, none of those pids remain and `$run/omatide/tide.sock` is not a socket. It does not wait in the foreground.


  Observed on 2026-10-14 the process command was `/workspace/target/release/omatide run`. The log was empty. The watch printed `{"tide":"0.1.0","type":"hello","v":1}` and a `state` with `"keel":"connecting"`, `"at":"home"`, and `"station":"9414290"`. Exit 124. The socket was `$run/omatide/tide.sock`. A second `omatide run` exited 1 with `omatide: omatide is already running on $run/omatide/tide.sock`.
- **Path from this file.** Run `./scripts/verify.sh` and require the pass line. Exit 0 with `Skipped.` is a failed step.


  Observed on 2026-10-14, after the Qt6 QML packages in `.github/workflows/ci.yml` were installed, both cases printed `ok`, then `window surfaces Tide.lastLog`, then `All engine-path checks passed.` The `repo` and `started` paths were under the temp checkout and under `plugins/org.omahoy.tide`. Exit 0.
- **Missing engine.** The drive runs the same start script with `PATH=/usr/bin:/bin` and a binary path that is not executable. The log argument is `$run/omatide/engine.log`, the file Tide reads. Observed exit 127 and log `omatide-start: 1: exec: omatide: not found`. That line is what the window stores as `Tide.lastLog`. A missing log fails the drive. After six failed connects the notice text is that line. The rectangle that draws it was not driven.

- **One default socket.** The drive runs a second `omatide run` while its engine is still up, then stops that process group. Observed exit 1 and stderr `omatide: omatide is already running on $run/omatide/tide.sock`.
- **Proof.** Save the `./run.sh` stderr, the start-script pid command, the watch transcript, the missing-engine log, and the `scripts/verify.sh` transcript under `$run/artifacts/verify/window/`. Do not label them as a window screenshot.

## Gotchas

- `scripts/verify.sh` prints `Qt6 QML dev packages not installed. Skipped.` and exits 0 when `pkg-config` cannot see Qt6. The drive requires `All engine-path checks passed.` and fails when the transcript contains `Skipped.` This VM installed the CI packages so the probe could load. A Hyprland machine with Quickshell paints the window without those packages. The path check still needs the packages, because it compiles the probe.
- The start script does not pass `--socket` or `--no-keel`. It inherits `XDG_RUNTIME_DIR` and execs `omatide run`. With this run's env, that socket is `$run/omatide/tide.sock`. `keel` stays `connecting` while omakeel's socket is absent. `engine.md` says why that transient is valid.
- The binary the script runs is `OMATIDE_BIN` when that is set, otherwise the release binary next to the QML file. `./run.sh` does not export `OMATIDE_BIN`. The path check is `scripts/verify.sh`.
- The drive stops the process group it started, then checks that those pids are gone and `$run/omatide/tide.sock` is not a socket. A second `omatide run` on that socket exits 1 and is not a process to leave running.
- `quickshell` is not an Ubuntu package. When the socket is real and `quickshell` is absent, `./run.sh` exits 127 before the deadline and the drive fails. That is not a product bug in the chart. An empty `WAYLAND_DISPLAY` does not run `./run.sh`.
