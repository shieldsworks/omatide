#!/usr/bin/env python3
"""Builds ui/coast.js: the outline of San Francisco Bay, for the bay chart.

The chart draws NOAA's current stations as arrows. Without a shore behind
them the eye has nothing to hang them on, so this traces the bay from
OpenStreetMap's coastline and simplifies it down to a few kilobytes.

It is deliberately a schematic and not a chart. There are no soundings,
no marks and no detail below about a hundred meters — omahelm is the
thing you navigate by. Keeping it coarse is the point, not a shortcut.

    python3 scripts/coastline.py

Coastline is OpenStreetMap, ODbL. Overpass is often busy, so the raw
answer is cached in .cache/ and the script can be re-run without
fetching again.
"""
import json
import math
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OVERPASS = "https://overpass-api.de/api/interpreter"

# The bay, its approaches and up to Suisun: wider than the places the
# chart names, so the outline never stops short of an arrow.
SOUTH, WEST, NORTH, EAST = 37.40, -122.75, 38.25, -121.90

# How far a point may be moved when simplifying, in degrees of latitude.
# About 90 meters, which is under a pixel at the size the bay is drawn.
TOLERANCE = 0.0008
# A fragment shorter than this is a rock or a pier, not a shore.
SHORTEST = 0.004


def fetch():
    """Coastline ways, cached because Overpass is often busy."""
    cache = ROOT / ".cache" / "coastline.json"
    if cache.exists():
        try:
            elements = json.loads(cache.read_text())["elements"]
            if elements:
                print(f"using {cache}")
                return elements
        except (ValueError, KeyError):
            cache.unlink()  # a bad download; fetch it again
    query = (f'[out:json][timeout:180];way["natural"="coastline"]'
             f'({SOUTH},{WEST},{NORTH},{EAST});out geom;')
    req = urllib.request.Request(
        OVERPASS, data=urllib.parse.urlencode({"data": query}).encode(),
        headers={"User-Agent": "omatide (github.com/shieldsworks/omatide)"})
    for attempt in range(1, 4):
        try:
            with urllib.request.urlopen(req, timeout=300) as r:
                body = r.read()
            break
        except urllib.error.HTTPError as e:
            if e.code not in (429, 504) or attempt == 3:
                raise
            print(f"Overpass busy ({e.code}), retrying in {20 * attempt} s", file=sys.stderr)
            time.sleep(20 * attempt)
    elements = json.loads(body)["elements"]
    if not elements:
        sys.exit("Overpass returned no coastline; nothing cached, try again later")
    cache.parent.mkdir(exist_ok=True)
    cache.write_bytes(body)
    print(f"fetched {len(elements)} ways")
    return elements


def stitch(ways):
    """Joins ways end to end into as few chains as possible.

    OpenStreetMap cuts the shore into hundreds of short ways that share
    their end nodes exactly. Drawn as they come they are hundreds of
    separate strokes; joined up they are a handful of long ones, which
    both draws better and simplifies better, because a corner between two
    ways is no longer a corner that has to be kept.
    """
    ends = {}
    chains = []
    for points in ways:
        if len(points) < 2:
            continue
        chains.append(list(points))
    changed = True
    while changed:
        changed = False
        ends = {}
        for i, chain in enumerate(chains):
            if chain is None:
                continue
            ends.setdefault(chain[-1], []).append(i)
        for i, chain in enumerate(chains):
            if chain is None:
                continue
            # Something ends where this one starts: put it in front.
            for j in ends.get(chain[0], []):
                if j == i or chains[j] is None:
                    continue
                chains[i] = chains[j][:-1] + chain
                chains[j] = None
                changed = True
                break
    return [c for c in chains if c is not None]


def clip(points):
    """Runs of the way that lie inside the box, as separate lines."""
    out, run = [], []
    for lat, lon in points:
        if SOUTH <= lat <= NORTH and WEST <= lon <= EAST:
            run.append((lat, lon))
        else:
            # Keep the point that leaves, so the line reaches the edge.
            if run:
                run.append((lat, lon))
                out.append(run)
                run = []
    if run:
        out.append(run)
    return [r for r in out if len(r) > 1]


def simplify(points, tolerance):
    """Douglas-Peucker: drop every point no further than `tolerance` from
    the line its neighbours make."""
    if len(points) < 3:
        return points
    # Longitude is shorter than latitude by the cosine, so distances are
    # measured on a grid that accounts for it.
    shrink = math.cos(math.radians((SOUTH + NORTH) / 2))

    def keep(lo, hi, marked):
        if hi <= lo + 1:
            return
        (y0, x0), (y1, x1) = points[lo], points[hi]
        dy, dx = y1 - y0, (x1 - x0) * shrink
        length = math.hypot(dy, dx)
        worst, at = 0.0, lo
        for i in range(lo + 1, hi):
            y, x = points[i]
            if length == 0:
                d = math.hypot(y - y0, (x - x0) * shrink)
            else:
                d = abs(dy * ((x - x0) * shrink) - dx * (y - y0)) / length
            if d > worst:
                worst, at = d, i
        if worst > tolerance:
            marked.add(at)
            keep(lo, at, marked)
            keep(at, hi, marked)

    marked = {0, len(points) - 1}
    sys.setrecursionlimit(10000)
    keep(0, len(points) - 1, marked)
    return [points[i] for i in sorted(marked)]


def length_of(points):
    shrink = math.cos(math.radians((SOUTH + NORTH) / 2))
    return sum(math.hypot(points[i + 1][0] - points[i][0],
                          (points[i + 1][1] - points[i][1]) * shrink)
               for i in range(len(points) - 1))


def main():
    ways = []
    for way in fetch():
        geometry = way.get("geometry")
        if not geometry:
            continue
        ways.append([(p["lat"], p["lon"]) for p in geometry])
    chains = stitch(ways)
    print(f"{len(ways)} ways stitched into {len(chains)} chains")

    lines = []
    for chain in chains:
        for run in clip(chain):
            thin = simplify(run, TOLERANCE)
            if len(thin) > 1 and length_of(thin) >= SHORTEST:
                lines.append(thin)
    lines.sort(key=length_of, reverse=True)
    points = sum(len(line) for line in lines)

    out = ROOT / "ui" / "coast.js"
    with out.open("w") as f:
        f.write(".pragma library\n\n")
        f.write("// San Francisco Bay's shore, for the bay chart's backdrop.\n")
        f.write("// Built by scripts/coastline.py from OpenStreetMap's coastline,\n")
        f.write("// ODbL. Simplified to about 90 meters: a schematic, not a chart.\n")
        f.write("//\n")
        f.write(f"// {len(lines)} lines, {points} points, "
                f"{SOUTH}..{NORTH} N, {WEST}..{EAST} W.\n\n")
        f.write("// Each line is longitude, latitude, longitude, latitude...\n")
        f.write("var LINES = [\n")
        for line in lines:
            flat = ",".join(f"{lon:.4f},{lat:.4f}" for lat, lon in line)
            f.write(f"[{flat}],\n")
        f.write("];\n")
    print(f"{out}: {len(lines)} lines, {points} points, {out.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
