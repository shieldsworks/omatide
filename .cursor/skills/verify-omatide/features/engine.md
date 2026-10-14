# The engine

`omatide run` serves the tide and the stream on a Unix socket. `omatide
watch` prints each line as it arrives. The lines are `docs/protocol.md`.

## Sub-features

- `engine-hello` a client receives `hello`, then `state`.
- `engine-home` with no keel, `state` says the position is home.
- `engine-single` a second `omatide run` on the same socket exits 1.
- `engine-empty` with no catalog, `state` carries the fetch problem and no tide station.

## How to get to it (user POV)

- The window starts `omatide run` when the socket is down. That path is `window.md`.
- Run `omatide run --socket PATH --no-keel` in a terminal.
- Run `omatide watch --socket PATH` in another terminal.

## Driving it with the socket

Preconditions.

- The baseline from `README.md`. Doctor passes.
- No engine is already bound to `$run/tide.sock`.

- **Start.** The drive runs `omatide run --socket "$run/tide.sock" --no-keel` in the background. Ready when `$run/tide.sock` is a socket and `kill -0` on that pid succeeds. On 2026-10-14 the engine log was empty, because the catalog loaded. The EXIT trap stops that process group.


- **Watch.** The drive runs `timeout 2 omatide watch --socket "$run/tide.sock"`. Exit 124. The first line is `{"tide":"0.1.0","type":"hello","v":1}`. The next line has `"type":"state"`, `"keel":"off"`, `"at":"home"`, `"station":"9414290"`, and `"stations":{"current":1,"tide":1}`. Turns and the height move. `"bay":[]` on this catalog, because the named narrows are not in it.
- **One engine.** A second `omatide run --socket "$run/tide.sock" --no-keel` returns 1. Stderr is `omatide: omatide is already running on $run/tide.sock`. The first pid is still alive, and a second watch still prints `hello`.
- **No catalog.** The baseline catalog stays in `$run/data/omatide`. This engine has its own socket, `$run/empty.sock`. The EXIT trap stops that process group. `$run/env.empty` points `OMATIDE_DATA_DIR` at `$run/empty`.


  Stderr contains the backticks around the fetch hint. The `state` line has `"stations":{"current":0,"tide":0}`. It has no `"station":"9414290"`. Observed on 2026-10-14.
- **Proof.** Save the watch transcript, the second start's stderr, and `$run/pid` under `$run/artifacts/verify/engine/`. The empty-catalog watch transcript is already at `empty-watch.out`.

## Gotchas

- `timeout` exits 124 because it stopped the watch. The pid in `$run/pid` must still be alive. The empty-catalog pid in `$run/empty.pid` stays alive too, until cleanup.
- `--no-keel` is why this socket's `keel` is `off`. The window's start does not pass `--no-keel`. `docs/protocol.md` lists the settled values `connected`, `lost`, `incompatible`, and `off`. The engine also sends `connecting` from the moment it binds until omakeel answers, the link is lost, or the protocols disagree. `"keel":"connecting"` is a valid transient. It is the window start's passing value. See `window.md`.
- Without `--socket` the engine uses `$XDG_RUNTIME_DIR/omatide/tide.sock`. A relative `XDG_RUNTIME_DIR` is an error.
- The EXIT trap stops the engine process groups and then deletes `$run`. A second start that exited 1 is already gone. Do not `pkill` by name.
