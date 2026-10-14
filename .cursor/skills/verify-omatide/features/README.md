# omatide verification map

This directory is the source for verifying omatide's user-facing behavior.
Read this index, then use the matching feature file as the recipe. A change
that adds or alters a user path updates its feature file in the same PR.

## Baseline preconditions

- A release build of this clone, launched per `../SKILL.md`, with its own
  `$run` directory. `$bin` is `realpath ./target/release/omatide`.
- `omatide fetch --region 37.80,-122.48,37.82,-122.45` has written
  `$run/data/omatide/stations.json`. That box is Golden Gate tide station
  `9414290` and current bin `PCT0286`.
- Doctor passes and names this clone's binary and this run's catalog.
- Never drive a catalog, config, or socket this run did not create.

The repository root is `git rev-parse --show-toplevel`. This proof used
`/workspace`. Use the clone you are in.

## Driving conventions

- Start every recipe from the baseline unless its preconditions say otherwise.
- Commands are literal. Keep quoted names and flags unchanged.
- Every `omatide` command below means `omatide_env` from `../SKILL.md`.
  `$run` and `$bin` are already set. A no-catalog command is
  `omatide_env_empty`. `omatide_env_ok` has to accept the env file before
  the binary runs. The empty directory's `OMATIDE_DATA_DIR` is the
  last assignment in that file. An export before `env` loses to whichever
  file `env` reads.

```sh
omatide_env tide
```
- Prefer stable handles. Station ids, the phrases in the terminal, and
  message types from `docs/protocol.md` are the handles. Heights, speeds,
  and timestamps move. Match the tokens a recipe names, not the whole line.
  `keel` may be `connecting` while omakeel has not answered. That value is
  in `engine.md`.
- Cleanup copies proof into the per-run evidence directory and then
  deletes `$run`. Leave the evidence directory in place.

## Proof and skip reporting

- CLI proof is the command, stdout, stderr, and the exit code.
- Socket proof is the reply lines.
- Mutation proof is a second read of `stations.json`, or a second
  `omatide stations` that still lists the station.
- Record the feature id with every artifact.
- An unreachable path is reported with the command tried and the unmet
  precondition. It is never reported as verified through a different path.
  A painted window needs Quickshell on Hyprland. On a machine without
  them, report the window paint unreached.

## Feature entry contract

Each feature file starts with an H1 and one paragraph, then these four H2s,
in order.

1. `Sub-features`
2. `How to get to it (user POV)`
3. `Driving it with the tide command`, or the matching command for that feature.
4. `Gotchas`

## Features

- [The tide](./tide.md) is the height now and the highs and lows to come.
- [The stream](./current.md) is the set and drift now, and the slacks to come.
- [The bay](./bay.md) is the tide and the stream at the named narrows.
- [The engine](./engine.md) is `omatide run` and `omatide watch`.
- [The window](./window.md) is `./run.sh`. The window starts its own engine, and it shows the engine log when that start fails.
