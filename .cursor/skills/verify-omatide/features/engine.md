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

- **Start.** Run `omatide run --socket "$run/tide.sock" --no-keel` in the background. Record the pid in `$run/pid`. Ready when `$run/tide.sock` is a socket and `kill -0` on that pid succeeds. On 2026-10-14 the engine log was empty, because the catalog loaded.
- **Watch.** Run `timeout 2 omatide watch --socket "$run/tide.sock"`. Exit 124. The first line is `{"tide":"0.1.0","type":"hello","v":1}`. The next line has `"type":"state"`, `"keel":"off"`, `"at":"home"`, `"station":"9414290"`, and `"stations":{"current":1,"tide":1}`. Turns and the height move. `"bay":[]` on this catalog, because the named narrows are not in it.
- **One engine.** Run `omatide run --socket "$run/tide.sock" --no-keel` again. Exit 1. Stderr is `omatide: omatide is already running on $run/tide.sock`. The first pid is still alive, and a second watch still prints `hello`.
- **No catalog.** Use a second directory so the baseline catalog stays. Point `OMATIDE_DATA_DIR` at an empty directory, start `omatide run --socket` with `--no-keel`, and watch once. Stderr contains run `omatide fetch`. The `state` line has `"stations":{"current":0,"tide":0}` and no `"station":"9414290"`. Observed on 2026-10-14.
- **Proof.** Save the watch transcript, the second start's stderr, and `$run/pid` under `$run/artifacts/verify/engine/`.

## Gotchas

- `timeout` exits 124 because it stopped the watch. The pid in `$run/pid` must still be alive.
- `--no-keel` is why `keel` is `off`. The window's start does not pass `--no-keel`, so its `state` says `connecting` until omakeel answers. See `window.md`.
- Without `--socket` the engine uses `$XDG_RUNTIME_DIR/omatide/tide.sock`. A relative `XDG_RUNTIME_DIR` is an error.
- Kill `$run/pid` only. A second start that exited 1 is already gone.
