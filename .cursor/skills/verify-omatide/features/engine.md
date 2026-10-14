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

- **Start.** `omatide_engine_start` runs `omatide run --socket "$run/tide.sock" --no-keel` in the background and records the pid in `$run/pid`. Ready when `$run/tide.sock` is a socket and `kill -0` on that pid succeeds. On 2026-10-14 the engine log was empty, because the catalog loaded.

```sh
omatide_engine_start() {
  local _opts _status i pid
  local -a _env
  omatide_fence_save _opts
  _status=0
  test -n "${run-}" && test -n "${bin-}" && omatide_env_load _env "$run/env" || _status=1
  if test "$_status" -eq 0; then
    mkdir -p "$run/artifacts/verify/engine" || _status=$?
  fi
  if test "$_status" -eq 0; then
    env "${_env[@]}" "$bin" run --socket "$run/tide.sock" --no-keel \
      >"$run/engine.log" 2>&1 &
    pid=$!
    omatide_fence_pid "$pid"
    echo "$pid" > "$run/pid"
    i=0
    while test "$i" -lt 25; do
      if test -S "$run/tide.sock" && kill -0 "$pid" 2>/dev/null; then
        break
      fi
      sleep 0.2 || true
      i=$((i + 1))
    done
    if ! test -S "$run/tide.sock" || ! kill -0 "$pid" 2>/dev/null; then
      _status=1
    fi
  fi
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}
omatide_engine_start
```

- **Watch.** `omatide_watch "$run/tide.sock" "$run/artifacts/verify/engine/watch.out" "$run/artifacts/verify/engine/watch.err"`. The status file is `watch.out.status`. Exit 124. The first line is `{"tide":"0.1.0","type":"hello","v":1}`. The next line has `"type":"state"`, `"keel":"off"`, `"at":"home"`, `"station":"9414290"`, and `"stations":{"current":1,"tide":1}`. Turns and the height move. `"bay":[]` on this catalog, because the named narrows are not in it.
- **One engine.** `omatide_env run --socket "$run/tide.sock" --no-keel` returns 1. Stderr is `omatide: omatide is already running on $run/tide.sock`. The first pid is still alive, and a second watch still prints `hello`. The shell stays open.
- **No catalog.** The baseline catalog stays in `$run/data/omatide`. This engine has its own socket, `$run/empty.sock`, and its own pid file, `$run/empty.pid`. Cleanup kills that pid. `$run/env.empty` points `OMATIDE_DATA_DIR` at `$run/empty`.

```sh
omatide_engine_empty() {
  local _opts _status i pid watch_status
  local -a _env
  omatide_fence_save _opts
  _status=0
  test -n "${run-}" && test -n "${bin-}" && omatide_env_load _env "$run/env.empty" || _status=1
  if test "$_status" -eq 0; then
    mkdir -p "$run/artifacts/verify/engine" || _status=$?
  fi
  if test "$_status" -eq 0; then
    env "${_env[@]}" "$bin" run --socket "$run/empty.sock" --no-keel \
      >"$run/empty.log" 2>&1 &
    pid=$!
    omatide_fence_pid "$pid"
    echo "$pid" > "$run/empty.pid"
    i=0
    while test "$i" -lt 25; do
      if test -S "$run/empty.sock" && kill -0 "$pid" 2>/dev/null; then
        break
      fi
      sleep 0.2 || true
      i=$((i + 1))
    done
    if ! test -S "$run/empty.sock" || ! kill -0 "$pid" 2>/dev/null; then
      _status=1
    fi
  fi
  if test "$_status" -eq 0; then
    timeout 2 env "${_env[@]}" "$bin" watch --socket "$run/empty.sock" \
      >"$run/artifacts/verify/engine/empty-watch.out" \
      2>"$run/artifacts/verify/engine/empty-watch.err"
    watch_status=$?
    test "$watch_status" -eq 124 || _status=$watch_status
  fi
  if test "$_status" -eq 0; then
    grep -F 'run `omatide fetch`' "$run/empty.log" || _status=$?
  fi
  if test "$_status" -eq 0; then
    grep -F '"stations":{"current":0,"tide":0}' \
      "$run/artifacts/verify/engine/empty-watch.out" || _status=$?
  fi
  if test "$_status" -eq 0 && grep -F '"station":"9414290"' \
      "$run/artifacts/verify/engine/empty-watch.out"; then
    printf '%s\n' 'empty catalog still named 9414290' >&2
    _status=1
  fi
  if test "$_status" -eq 0; then
    cp "$run/empty.log" "$run/artifacts/verify/engine/empty.log" || _status=$?
  fi
  if test "$_status" -eq 0; then
    cp "$run/empty.pid" "$run/artifacts/verify/engine/empty.pid" || _status=$?
  fi
  omatide_fence_finish "$_status" "$_opts"
  return "$_status"
}
omatide_engine_empty
```

  Stderr contains the backticks around the fetch hint. The `state` line has `"stations":{"current":0,"tide":0}`. It has no `"station":"9414290"`. Observed on 2026-10-14.
- **Proof.** Save the watch transcript, the second start's stderr, and `$run/pid` under `$run/artifacts/verify/engine/`. The empty-catalog watch transcript is already at `empty-watch.out`.

## Gotchas

- `timeout` exits 124 because it stopped the watch. The pid in `$run/pid` must still be alive. The empty-catalog pid in `$run/empty.pid` stays alive too, until cleanup.
- `--no-keel` is why this socket's `keel` is `off`. The window's start does not pass `--no-keel`. `docs/protocol.md` lists the settled values `connected`, `lost`, `incompatible`, and `off`. The engine also sends `connecting` from the moment it binds until omakeel answers, the link is lost, or the protocols disagree. `"keel":"connecting"` is a valid transient. It is the window start's passing value. See `window.md`.
- Without `--socket` the engine uses `$XDG_RUNTIME_DIR/omatide/tide.sock`. A relative `XDG_RUNTIME_DIR` is an error.
- Kill `$run/pid`, `$run/empty.pid`, and `$run/window.pid` when those files exist. A second start that exited 1 is already gone. Cleanup does this and then deletes `$run`.
