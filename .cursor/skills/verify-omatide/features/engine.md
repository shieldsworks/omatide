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
- **No catalog.** The baseline catalog stays in `$run/data/omatide`. This engine has its own socket, `$run/empty.sock`, and its own pid file, `$run/empty.pid`. Cleanup kills that pid. `$run/env.empty` points `OMATIDE_DATA_DIR` at `$run/empty`.

```sh
set -euo pipefail
: "${run:?}"
mkdir -p "$run/artifacts/verify/engine"
env $(cat "$run/env.empty") "$bin" run --socket "$run/empty.sock" --no-keel \
  >"$run/empty.log" 2>&1 &
echo $! > "$run/empty.pid"
for _ in $(seq 1 25); do
  test -S "$run/empty.sock" && break
  sleep 0.2
done
test -S "$run/empty.sock"
kill -0 "$(cat "$run/empty.pid")"
set +e
timeout 2 env $(cat "$run/env.empty") "$bin" watch --socket "$run/empty.sock" \
  >"$run/artifacts/verify/engine/empty-watch.out" \
  2>"$run/artifacts/verify/engine/empty-watch.err"
watch_status=$?
set -e
test "$watch_status" -eq 124
grep -F 'run `omatide fetch`' "$run/empty.log"
grep -F '"stations":{"current":0,"tide":0}' \
  "$run/artifacts/verify/engine/empty-watch.out"
if grep -F '"station":"9414290"' "$run/artifacts/verify/engine/empty-watch.out"; then
  printf '%s\n' 'empty catalog still named 9414290' >&2
  exit 1
fi
cp "$run/empty.log" "$run/artifacts/verify/engine/empty.log"
cp "$run/empty.pid" "$run/artifacts/verify/engine/empty.pid"
```

  Stderr contains the backticks around the fetch hint. The `state` line has `"stations":{"current":0,"tide":0}`. It has no `"station":"9414290"`. Observed on 2026-10-14.
- **Proof.** Save the watch transcript, the second start's stderr, and `$run/pid` under `$run/artifacts/verify/engine/`. The empty-catalog watch transcript is already at `empty-watch.out`.

## Gotchas

- `timeout` exits 124 because it stopped the watch. The pid in `$run/pid` must still be alive. The empty-catalog pid in `$run/empty.pid` stays alive too, until cleanup.
- `--no-keel` is why this socket's `keel` is `off`. The window's start does not pass `--no-keel`. `docs/protocol.md` lists the settled values `connected`, `lost`, `incompatible`, and `off`. The engine also sends `connecting` from the moment it binds until omakeel answers, the link is lost, or the protocols disagree. `"keel":"connecting"` is a valid transient. It is the window start's passing value. See `window.md`.
- Without `--socket` the engine uses `$XDG_RUNTIME_DIR/omatide/tide.sock`. A relative `XDG_RUNTIME_DIR` is an error.
- Kill `$run/pid`, `$run/empty.pid`, and `$run/window.pid` when those files exist. A second start that exited 1 is already gone. Cleanup does this and then deletes `$run`.
