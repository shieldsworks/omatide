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

Run the drive in a bash you start for that purpose. Do not source
`drive.sh` and do not paste it into the shell you are already using.

```sh
bash --noprofile --norc .cursor/skills/verify-omatide/drive.sh
```

That process is the whole drive. It sets `set -euo pipefail` and `set -m`.
On EXIT, INT, and TERM it signals the engine and the window process group
with TERM, then KILL, including an engine Tide started with `execDetached`.
It copies the artifacts out, and only then deletes the run directory. The
shell that started it is not the shell those options and traps belong to.

With no arguments the drive runs doctor, tide, current, bay, engine, and
window. Name the features you want.

```sh
bash --noprofile --norc .cursor/skills/verify-omatide/drive.sh doctor tide current bay engine
bash --noprofile --norc .cursor/skills/verify-omatide/drive.sh window
```

An unknown feature exits 2. The drive prints `evidence: /tmp/omatide-verify-evidence.XXXXXX`
as it exits. That directory remains. The run directory does not.

`bash --noprofile --norc` inherits the environment of the shell that started
it, including `PATH`. When `mise` is on `PATH`, the drive builds with
`mise exec -- cargo build --release --locked`. Otherwise it installs Rust
1.98.1 and builds with `rustup run 1.98.1`. `mise.toml` names Rust `1.98`,
and that channel is 1.98.1 (`48a229cea 2026-09-01`). Rust 1.98.0
(`88d9e12ae 2026-08-18`) is a different compiler. Both arms require
`rustc --version` to contain `1.98.1`. The drive does not prepend a mise
directory of its own. Put `mise` on `PATH`, or leave it off, before you
start the child.

The drive records the build start, runs `touch src/main.rs`, deletes
`target/release/omatide`, and builds. The touch forces Cargo to relink
this package, so a stub left on the old deps inode is not what gets
published. The new binary's mtime has to be at least the build start, and
`omatide --version` has to print `omatide 0.1.0`.

Do not export `HOME`, `XDG_RUNTIME_DIR`, `XDG_DATA_HOME`,
`XDG_CONFIG_HOME`, `OMATIDE_DATA_DIR`, or `OMATIDE_CONFIG` from the shell
that starts the drive. The drive writes those into `$run/env` and passes
the lines to `env` as an array. A path has to be absolute, with no glob
character. A dangling symlink is rejected. `realpath -m` has to keep every
path under `$run`. The binary does not run unless that file passes, so it
cannot fall through to `~/.local/share/omatide`.

Every run gets its own directory. Do not read or write
`~/.local/share/omatide` or `~/.config/omatide`.

Ready when `$run/fetch.log` contains `2 stations, 2 of them predictable`
and `saved to $run/data/omatide/stations.json`, and that file exists. The
box holds NOAA station `9414290`, San Francisco at the Golden Gate, and
one current bin, `PCT0286`. Fetch needs the network once. After that
fetch, the commands below need no network.

`omatide fetch` with no `--region` uses the default bay and takes a couple
of minutes. This proof did not run that fetch. `features/bay.md` says
what the small box leaves out.

## Doctor

Doctor runs inside the drive after fetch. The version line is
`omatide 0.1.0`. The catalog path is under `$run`. The env file has 7
lines. The last line of `$run/env.empty` is `OMATIDE_DATA_DIR=$run/empty`.

## Drive

Follow `features/`. Start at `features/README.md`.

- A sailor's terminal is `omatide tide`, `omatide current`, `omatide bay`,
  `omatide stations`, `omatide watch`, and `omatide fetch`.
- The engine is `omatide run`, then `omatide watch` on the same socket.
  `docs/protocol.md` is the line format. `keel` also takes the transient
  value `connecting` before omakeel answers. See `features/engine.md`.
- The window is `./run.sh`, which execs `quickshell -p ui/shell.qml`. The
  drive passes `$run/env` and, when the compositor socket is real, that
  absolute socket. Still alive after three seconds is the pass, recorded
  as 124. The drive then stops the process group and requires the tree it
  started to be gone and `$run/omatide/tide.sock` to be free. An exit
  before that deadline fails the drive. An empty or unreachable
  `WAYLAND_DISPLAY` does not run `./run.sh`. The evidence says
  `window-open: not-driven`. Tide reads `$run/omatide/engine.log`. A
  missing log fails the drive. Quickshell on a Hyprland session is
  required to paint the window. When it is not on `PATH`, do not skip
  `features/window.md`. The drive still runs the start script, the path
  check, and the missing engine.

`scripts/verify.sh` resolves `target/release/omatide` from the QML file's
directory and from the plugin directory, then checks that
`ui/TideWindow.qml` names `Tide.lastLog`. The pass text is
`All engine-path checks passed.` A transcript that contains `Skipped.`
fails the drive.

## Evidence

The evidence directory is created before the build. The EXIT trap copies
`$run/artifacts`, `$run/fetch.log`, `$run/data/omatide/stations.json`,
`$run/engine.log`, and `$run/omatide/engine.log` into that directory
before it deletes `$run`.

- `record` saves the command, stdout, stderr, and the exit code under
  `verify/<id>/`.
- For the socket, the watch transcript is the proof. Exit 124 means the
  watch was still running when `timeout` stopped it.
- The NOAA download is the real `omatide fetch`. Do not hand-write
  `stations.json`.
- If a path cannot be reached, the drive writes the command and the
  missing precondition. Do not mark that path passed.

## Cleanup

The trap is the cleanup. It runs on a normal exit, on Ctrl-C, and on
TERM. It signals each process group the drive started (`kill -- -pgid`),
waits, then sends KILL. It copies the artifacts, then deletes the run
directory. It does not `pkill` by name.

When omatide's commands or the lines it prints change, run
`/maintain-verification-skill` over this directory.
