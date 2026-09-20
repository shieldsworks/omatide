#!/usr/bin/env python3
"""Rebuilds tests/fixtures/noaa/ from NOAA CO-OPS.

These are the tables omatide is checked against: its own predictions must
land on them. The years are spread across the moon's 18.6-year nodal
cycle, because that is what the node factors correct for and a bug there
hides in any one year.
"""
import json, os, urllib.request, datetime as dt

API = "https://api.tidesandcurrents.noaa.gov/api/prod/datagetter"
HERE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tests", "fixtures", "noaa")
# One four-day window a year, in a different month each time.
WINDOWS = [(2019, 1), (2021, 3), (2023, 5), (2026, 9), (2028, 11), (2031, 2), (2035, 7)]


def fetch(**args):
    args.setdefault("application", "omatide")
    args.setdefault("time_zone", "gmt")
    args.setdefault("units", "metric")
    args.setdefault("format", "json")
    url = API + "?" + "&".join(f"{k}={v}" for k, v in args.items())
    with urllib.request.urlopen(url, timeout=120) as r:
        body = json.load(r)
    if "error" in body:
        raise SystemExit(f"{url}: {body['error']['message']}")
    return body


def iso(t):
    return t.replace(" ", "T") + ":00Z"


def write(name, header, rows):
    path = os.path.join(HERE, name)
    with open(path, "w") as f:
        f.write(f"# {header}\n")
        f.write("# Rebuild with scripts/reference.py.\n")
        for row in rows:
            f.write(row + "\n")
    print(f"{path}: {len(rows)} rows")


def heights():
    rows = []
    for year, month in WINDOWS:
        start = dt.date(year, month, 12)
        body = fetch(product="predictions", station=9414290, datum="MLLW",
                     begin_date=start.strftime("%Y%m%d"),
                     end_date=(start + dt.timedelta(days=3)).strftime("%Y%m%d"),
                     interval="h")
        rows += [f"{iso(p['t'])} {p['v']}" for p in body["predictions"]]
    write("9414290-heights.txt",
          "San Francisco (Golden Gate) 9414290: predicted height above MLLW, meters.", rows)


def hilo(station, name):
    rows = []
    for year, month in WINDOWS:
        start = dt.date(year, month, 12)
        body = fetch(product="predictions", station=station, datum="MLLW",
                     begin_date=start.strftime("%Y%m%d"),
                     end_date=(start + dt.timedelta(days=3)).strftime("%Y%m%d"),
                     interval="hilo")
        rows += [f"{iso(p['t'])} {p['v']} {p['type']}" for p in body["predictions"]]
    write(f"{station}-hilo.txt", f"{name} {station}: predicted high and low water, MLLW, meters.", rows)


def currents(station, bin_, name):
    rows = []
    for year, month in WINDOWS:
        start = dt.date(year, month, 12)
        body = fetch(product="currents_predictions", station=station, bin=bin_,
                     begin_date=start.strftime("%Y%m%d"),
                     end_date=(start + dt.timedelta(days=1)).strftime("%Y%m%d"),
                     interval=60)
        rows += [f"{iso(p['Time'])} {p['Velocity_Major']}"
                 for p in body["current_predictions"]["cp"]]
    write(f"{station}-{bin_}-currents.txt",
          f"{name} {station} bin {bin_}: predicted speed along the channel, cm/s.", rows)


def maxslack(station, bin_, name):
    rows = []
    for year, month in WINDOWS:
        start = dt.date(year, month, 12)
        body = fetch(product="currents_predictions", station=station, bin=bin_,
                     begin_date=start.strftime("%Y%m%d"),
                     end_date=(start + dt.timedelta(days=3)).strftime("%Y%m%d"),
                     interval="MAX_SLACK")
        rows += [f"{iso(p['Time'])} {p['Velocity_Major']} {p['Type']}"
                 for p in body["current_predictions"]["cp"]]
    write(f"{station}-{bin_}-maxslack.txt",
          f"{name} {station} bin {bin_}: predicted slack and maximum stream, cm/s.", rows)


if __name__ == "__main__":
    heights()
    hilo(9414290, "San Francisco (Golden Gate)")
    hilo(9414806, "Sausalito")
    currents("SFB1212", 9, "Raccoon Strait")
    maxslack("SFB1212", 9, "Raccoon Strait")
    maxslack("SFB1202", 17, "Golden Gate Bridge, 0.88 nm NE of")
    maxslack("PCT0291", 1, "Alcatraz Island, 0.2 mile west of")
