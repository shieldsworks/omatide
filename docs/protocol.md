# omatide protocol

Version 1. The tide engine (`omatide run`) is the server. The omatide bar
widget and window are clients.

Unlike the other Omahoy engines this one needs no network once its
stations are fetched. Harmonic constants are good for years, so a tide
that can be predicted at the dock can be predicted at sea, with the dish
off.

## Transport

- A Unix stream socket, `$XDG_RUNTIME_DIR/omatide/tide.sock` by default
  (`--socket` sets another). The directory is created with mode 0700.
- A lock file beside the socket (`tide.sock.lock`) keeps a second engine
  from starting on the same socket. A socket left behind by a crashed
  engine is replaced.
- Newline-delimited JSON, UTF-8, one object per line.
- Every engine message has `"type"` and `"v": 1`. A client that sees
  another `v` shows an error and stops using the data.
- Key order is not significant. Keys that don't apply are left out, never
  sent as `null`. Clients ignore message types and keys they don't know.
- Any number of clients may connect. `state` goes to every client;
  `curve`, `bay`, `stations` and `error` go only to the client that asked.
- A client that can't take a message within 2 seconds, or falls 64
  messages behind, is disconnected. It can reconnect.
- A line longer than 64 KiB from a client, or one that isn't a JSON
  object, is answered with an `error` and otherwise ignored.

## Units

Heights are meters above the chart datum, mean lower low water, which is
what a depth on a chart is measured from. Speeds are knots. Times are ISO
8601 in UTC. A stream's direction is degrees true.

## Engine messages

`hello` is sent once on connect, followed by `state`.

```json
{"type":"hello","v":1,"tide":"0.1.0"}
```

`state` is the complete current state, re-sent whenever it changes and at
least every 30 seconds so a countdown stays honest. Clients replace their
copy.

```json
{"type":"state","v":1,
 "time":"2026-09-20T17:12:00Z",
 "keel":"connected",
 "here":{"lat":37.86630,"lon":-122.31480,"at":"boat"},
 "stations":{"tide":122,"current":295},
 "tide":{"station":"9414816","name":"Berkeley","lat":37.86500,"lon":-122.30700,
         "distanceNm":0.38,"heightM":1.372,"rising":false,
         "turns":[{"turn":"low","time":"2026-09-20T20:47:19Z","heightM":1.103}]},
 "current":{"station":"SFB1218-1","name":"Emeryville Marina","lat":37.84332,
            "lon":-122.32537,"distanceNm":1.47,"depthM":1.5,
            "knots":0.11,"way":"ebb","setDeg":294,
            "turns":[{"turn":"slack","time":"2026-09-20T18:52:00Z","knots":0.0}]},
 "bay":[{"place":"Golden Gate","note":"the strongest stream in the bay, and the reference the rest follow",
         "station":"SFB1202-17","lat":37.82922,"lon":-122.46202,
         "knots":2.08,"way":"flood","setDeg":52,
         "next":{"turn":"flood","time":"2026-09-21T14:37:08Z","knots":2.17}}],
 "problems":["config.toml: unknown setting bogus"]}
```

- `keel` is `connected`, `lost`, `incompatible`, or `off` when omatide was
  started `--no-keel`.
- `here.at` is `boat` when omakeel has a fix and `home` otherwise.
- `tide` and `current` are the nearest predictable station of each kind,
  and are left out when there is none within 60 nautical miles.
- `follows` names the reference station, for a subordinate station.
- `way` is `flood`, `ebb` or `slack`. `setDeg` is left out where NOAA
  doesn't say which way the stream sets.
- `turns` runs three days ahead. A tide's turns are `high` and `low` with
  `heightM`; a stream's are `slack`, `flood` and `ebb` with `knots`, and
  `knots` is the size of the stream, never negative — `way` says which way
  it runs.
- `bay` is San Francisco Bay's named narrows at this moment, in the order
  you sail them: in from the sea, up to the delta, then down the east
  shore. It is empty outside the bay.

## Client requests

A request is a JSON object with a `type`. An `id` of any JSON value is
echoed on the answer, so several can be in flight at once.

`curve` asks for something to draw.

```json
{"type":"curve","id":7,"station":"9414290","time":"2026-09-20T12:00:00Z",
 "hours":30,"stepSeconds":360}
```

- `station` defaults to the nearest to `lat`/`lon`, or to where the boat
  is, of `kind` (`tide` or `current`, default `tide`).
- `time` defaults to now, `hours` to 24 (1 to 336), `stepSeconds` to 600
  (60 to 3600). The step is stretched rather than the window cut if the
  answer would carry more than 2016 points, so a fortnight is a fortnight,
  only coarser.

```json
{"type":"curve","v":1,"id":7,"station":"9414290","name":"SAN FRANCISCO (Golden Gate)",
 "kind":"tide","unit":"m","start":"2026-09-20T12:00:00Z","stepSeconds":360,
 "values":[1.372,1.362,1.350],
 "turns":[{"turn":"low","time":"2026-09-20T20:47:19Z","heightM":1.103}]}
```

`values` are bare numbers at `start` plus *n* × `stepSeconds`: a fortnight
of them is a tenth the size of the same points as objects, and the times
are implied. A current's values are signed — positive on the flood.

`bay` asks for the whole bay at one moment, which is what the window's
scrubber uses.

```json
{"type":"bay","id":8,"time":"2026-09-21T14:00:00Z"}
```

The answer carries `current` and `tide` arrays shaped like `state.bay`,
the tide one with `heightM` in place of `knots` and `way`.

`streams` asks for the stream at every station over an area, at one
moment: what a chart layer draws. omahelm's stream layer uses it, the way
its wind layer uses omawind's `field`.

```json
{"type":"streams","id":8,"south":37.7,"west":-122.6,"north":37.95,"east":-122.3,
 "time":"2026-09-21T22:00:00Z","depthM":3}
```

Every field is optional. A box keeps only the stations inside it; `lat`
and `lon` sort the answer nearest first and add `distanceNm` to each, and
`withinNm` drops what is further off than that. `time` defaults to now and
`depthM` to the engine's `depth` setting.

```json
{"type":"streams","v":1,"id":8,"time":"2026-09-21T22:00:00Z","depthM":3.0,
 "streams":[{"station":"SFB1212-9","name":"Raccoon Strait",
             "lat":37.87190,"lon":-122.44200,"depthM":5.8,
             "knots":0.62,"way":"ebb","setDeg":237}]}
```

One bin per station, the one nearest `depthM`, because a chart wants one
arrow in each channel rather than three stacked on top of each other. At
most 400 stations come back; `"more": true` says some were left out. The
whole bay is about 150, so that only bites on an area far larger than a
chart shows.

`stations` lists the catalog, for drawing a map.

```json
{"type":"stations","id":9,"kind":"current","lat":37.87,"lon":-122.44,"withinNm":3}
```

Every filter is optional. The answer is
`{"type":"stations","v":1,"stations":[…]}`, each with `station`, `name`,
`kind`, `lat`, `lon`, and `distanceNm`, `depthM` and `floodDeg` where they
apply. Only stations that can actually be predicted are listed.

`error` answers a request the engine can't serve.

```json
{"type":"error","v":1,"id":9,"message":"no station SFB9999-1"}
```
