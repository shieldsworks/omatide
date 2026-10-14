# AGENTS.md

Omatide predicts tides and currents for Omahoy from NOAA's harmonic
constants, offline, with a model of San Francisco Bay. A Rust engine
serves state over a Unix socket; a Quickshell (QML) window and bar
widget read it.

## Layout

- `src/` the engine and CLI. `predict.rs`, `constituent.rs`, `astro.rs`
  are the arithmetic; `engine.rs` is the socket server.
- `tests/predict.rs` checks predictions against NOAA tables in
  `tests/fixtures/`. `tests/engine.rs` drives a real engine over its socket.
- `ui/` the QML window and widget.
- `docs/protocol.md` the socket protocol.
- `scripts/` fixture rebuilders and `verify.sh`, the engine-path check.

## Commands

Toolchain: `mise install` (Rust 1.98, rustfmt, clippy).

- Build: `cargo build --locked`
- Lint: `cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings`
- Test: `cargo test --locked`
- Release build: `cargo build --release --locked`
- Engine path: `./scripts/verify.sh` (needs Qt6 dev packages; skips without them)

## Safe test command

Run tests as:

    setsid -w cargo test --locked

or `mise test-agent`. The engine tests start the engine in-process, and it
listens for SIGINT and SIGTERM; `http.rs` spawns and kills curl. `setsid`
keeps the tests in their own session so no signal reaches your shell.
Run anything that starts the engine or the window the same way.

## CI

`.github/workflows/ci.yml` runs on x86_64 and aarch64: `mise lint`,
`mise test`, the release build, then `scripts/verify.sh`. All must pass.

## Rules

- Keep diffs small and on topic.
- No comments that restate the code. Write comments only for what the
  code can't say.
- Match the prose style already here: plain, short, no filler.
- Don't touch `manifest.json`, `scripts/link-plugin.sh`, version numbers,
  or anything else for release or the marketplace without asking.
- Run lint and the safe test command before you say you're done.
