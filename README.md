# Omatide

Tides and currents for [Omahoy](https://github.com/shieldsworks/omahoy),
offline, with a model of San Francisco Bay.

**Status: early.** It predicts, and it has a window. There are no routes
through the bay yet, and no sun or moon.

A tide is not fetched, it is predicted. NOAA measured the harmonic
constants at each station over years, and from them the height and the
stream follow by arithmetic for as far ahead as you like. Omatide
downloads those constants once and then works at sea, with the dish off,
for years.

## What it does

- **The tide where the boat is.** The height above chart datum now, which
  way it is going, and the highs and lows to come, from the nearest of
  NOAA's stations to the position omakeel reports.
- **The stream where the boat is.** Set and drift now, and the slacks and
  maximums to come.
- **San Francisco Bay, all of it.** The bay is not one tide. High water at
  the Golden Gate reaches Port Chicago an hour and three quarters later
  and a third of a meter lower, and the stream that is slack off Alcatraz
  is still ebbing through Raccoon Strait. Omatide carries every station
  NOAA has in the bay — 122 tide stations and 295 current bins — and draws
  the bay's narrows as a current chart you can drag through time.
- **A bar widget**, with the height, which way it is going, and the next
  turn.

## How right it is

Omatide's predictions are checked against NOAA's own published tables,
over seven years spread across the moon's 18.6-year nodal cycle:

| | worst error |
|---|---|
| tide heights | 1.3 mm |
| high and low water | to the minute |
| stream speed | 0.2 cm/s, four thousandths of a knot |
| slack and maximum stream | to the minute |

That is NOAA's own rounding. `cargo test` checks it; the fixtures are in
`tests/fixtures/noaa/` and `scripts/reference.py` rebuilds them.

The arithmetic is Schureman's, written from scratch — no tide library.
Three things in it are worth knowing, because none is in the textbook:

- Schureman's epoch is 1900 January 1 at **0h**, not the Julian Day
  2415020.0 it is usually written as. Half a day is 6.6° of lunar motion,
  and getting it wrong costs 24 cm.
- **NOAA holds node factors constant for a whole calendar year**, worked
  out at mid-year. Varying them properly with time is a better model of
  the ocean and a worse match to the tables everyone else reads.
- M1, L2, 2MK3, M3 and S1 do not take the node factors the textbook gives
  them. What NOAA uses is in `src/constituent.rs`, each marked, each
  checked across the nodal cycle.

## Installing

```sh
mise install          # the Rust toolchain
cargo build --release
./target/release/omatide fetch   # NOAA's stations, once, a few minutes
./run.sh                         # the window
```

`fetch` writes about 380 KB to `~/.local/share/omatide/stations.json` and
is the only thing that needs the network.

To put the bar widget in the Omarchy shell, `scripts/link-plugin.sh`
points `~/.config/omarchy/plugins/org.omahoy.tide` at this checkout.

## Commands

```
omatide run       serve the tide and the stream to Omahoy apps
omatide watch     print the running engine's state as it changes
omatide fetch     download every station in the region from NOAA, once
omatide stations  list the stations omatide has
omatide tide      the tide at a station, or the nearest one to a position
omatide current   the stream at a station, or the nearest one
omatide bay       the stream everywhere in San Francisco Bay at one moment
```

## Settings

`~/.config/omatide/config.toml`:

```toml
region = 36.8, -123.8, 38.8, -121.6   # the waters to carry stations for
home = "37.8663, -122.3148"           # where the tide is, with no GPS
depth = 3                             # meters: which current bin to read
```

A current station is measured in bins down the water column, and they are
not the same stream — near the bottom it runs slower. A boat feels the top
of it, so `depth` is shallow by default.

## In the window

| | |
|---|---|
| `n` | Night Watch: red on black, for this window |
| `[` `]` | step the bay an hour back or on |
| `0` | back to now |
| `+` `-` | zoom the bay in and out |
| `q`, Escape | close |

Drag the bar under the bay to watch it turn. Hover a chart to read the
height or the speed at that moment. On the bay: scroll or `+` and `-` to
zoom, drag to pan, double-click for the whole bay again.

## Where the numbers come from

NOAA CO-OPS, `api.tidesandcurrents.noaa.gov`: the metadata service for
stations, harmonic constants, datums and subordinate offsets. A
subordinate station — most of the places a sailor names, Red Rock and
Point Blunt and Sausalito among them — has no constants of its own, only
offsets against one that has, so omatide warps the reference station's own
curve onto the subordinate's turns rather than replacing it with a plain
cosine. That keeps the shallow-water lopsidedness that is everywhere in
the bay.

`docs/protocol.md` is the socket protocol.

## License

MIT
