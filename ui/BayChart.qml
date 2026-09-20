import QtQuick
import "coast.js" as Coast

// The stream everywhere in San Francisco Bay at one moment.
//
// This is the old tidal current chart, drawn from NOAA's own survey: an
// arrow at each of the bay's narrows, pointing the way the stream sets
// and as long as it is strong, over the shore of the bay itself.
//
// The shore is a schematic and not a chart: simplified to about ninety
// meters, with no soundings and no marks. omahelm is the thing you
// navigate by; this is the thing you plan on.
//
// Scroll to zoom and drag to pan, because a dozen of the bay's places
// sit inside two miles off Angel Island and the whole bay can't show
// them all at once. Double-click goes back to the whole bay.
//
// What it is for is the thing a list of numbers can't show — that the
// Gate can be flooding while Carquinez still ebbs, and that Raccoon
// Strait turns before either.
Item {
    id: bay

    // Rows shaped like the protocol's `bay` array.
    property var arrows: []
    property var theme: null
    property string highlight: ""

    signal picked(string station)

    readonly property color ink: theme ? theme.foreground : "#a9b1d6"
    readonly property color accent: theme ? theme.accent : "#7aa2f7"
    readonly property color warn: theme ? theme.red : "#f7768e"
    readonly property int textSize: theme ? Math.max(9, theme.baseSize - 2) : 10

    readonly property real pad: 30
    // The strongest stream on show sets the scale, so a slack bay doesn't
    // draw twenty invisible arrows.
    readonly property real fastest: {
        let most = 0.5;
        for (let i = 0; i < arrows.length; i++) most = Math.max(most, arrows[i].knots);
        return most;
    }
    // The bay, from the bar outside the Gate to the head of the south
    // bay and up to Suisun. Fixed, not taken from the arrows: framing on
    // the arrows cut the Gate off at the western-most one, and made the
    // whole map shift about as stations came and went with the tide.
    readonly property var frame: ({s: 37.45, n: 38.12, w: -122.58, e: -122.02})
    // Wider still if a place ever falls outside it.
    readonly property var bounds: {
        let s = frame.s, n = frame.n, w = frame.w, e = frame.e;
        for (let i = 0; i < arrows.length; i++) {
            s = Math.min(s, arrows[i].lat);
            n = Math.max(n, arrows[i].lat);
            w = Math.min(w, arrows[i].lon);
            e = Math.max(e, arrows[i].lon);
        }
        return {s: s, n: n, w: w, e: e};
    }
    // A degree of longitude is shorter than a degree of latitude, by the
    // cosine of the latitude. Without that the bay comes out stretched.
    readonly property real shrink: bounds ? Math.cos((bounds.s + bounds.n) / 2 * Math.PI / 180) : 1
    readonly property real scale: {
        if (!bounds) return 1;
        const wide = Math.max(1e-6, (bounds.e - bounds.w) * shrink);
        const tall = Math.max(1e-6, bounds.n - bounds.s);
        return Math.min((width - 2 * pad) / wide, (height - 2 * pad) / tall);
    }
    // Zoom, and where the middle of the view sits. 1 is the whole bay;
    // the narrows off Angel Island want about 6.
    property real zoom: 1
    readonly property real maxZoom: 12
    // The point the view is centered on, in degrees. Kept in degrees and
    // not pixels so a resize doesn't slide the bay sideways. `placed`
    // says whether it has been moved: a longitude of 0 is Greenwich, not
    // "unset", so it can't stand in for one.
    property real centerLat: 0
    property real centerLon: 0
    property bool placed: false
    readonly property bool zoomed: zoom > 1.001

    // Un-zoomed, the bay sits in the middle of whatever room there is.
    function middleLat() { return placed ? centerLat : (bounds ? (bounds.s + bounds.n) / 2 : 0); }
    function middleLon() { return placed ? centerLon : (bounds ? (bounds.w + bounds.e) / 2 : 0); }

    function home() {
        zoom = 1;
        placed = false;
    }
    // Back to the whole bay whenever the places change out from under it,
    // so a view can't be left pointing at nothing.
    onBoundsChanged: if (!arrows.length) home()

    function xOf(lon) {
        return bounds ? width / 2 + (lon - middleLon()) * shrink * scale * zoom : 0;
    }
    function yOf(lat) {
        return bounds ? height / 2 - (lat - middleLat()) * scale * zoom : 0;
    }
    // And back again, for zooming about the pointer.
    function lonAt(x) {
        return bounds ? middleLon() + (x - width / 2) / (shrink * scale * zoom) : 0;
    }
    function latAt(y) {
        return bounds ? middleLat() - (y - height / 2) / (scale * zoom) : 0;
    }

    // Keeps the view over the bay, so it can never be dragged off into
    // the ocean.
    //
    // What may be panned is what the canvas can't already see. The
    // window is often far wider than the bay is at this scale, and then
    // there is nothing to pan sideways on at all — clamping to the
    // frame's own width instead of the view's pinned the middle and
    // stopped zoom moving where it was told.
    function settle() {
        // Nothing to hold in place until the view has been moved.
        if (!bounds || !placed || scale <= 0) return;
        const seesLon = (width / 2) / (shrink * scale * zoom);
        const seesLat = (height / 2) / (scale * zoom);
        const halfLon = (bounds.e - bounds.w) / 2;
        const halfLat = (bounds.n - bounds.s) / 2;
        const lon = middleLon(), lat = middleLat();
        centerLon = seesLon >= halfLon
            ? (bounds.w + bounds.e) / 2
            : Math.max(bounds.w + seesLon, Math.min(bounds.e - seesLon, lon));
        centerLat = seesLat >= halfLat
            ? (bounds.s + bounds.n) / 2
            : Math.max(bounds.s + seesLat, Math.min(bounds.n - seesLat, lat));
    }

    // Zooms about a point on screen, so whatever is under the pointer
    // stays under it.
    function zoomAt(factor, x, y) {
        const next = Math.max(1, Math.min(maxZoom, zoom * factor));
        if (next === zoom) return;
        if (next === 1) {
            home();
            return;
        }
        // Where the pointer is now, read at the zoom we are leaving.
        // Reading it after the change would give the middle back every
        // time, and the view would never move.
        const lon = lonAt(x), lat = latAt(y);
        zoom = next;
        // Solve for the middle that leaves (lon, lat) where it was.
        placed = true;
        centerLon = lon - (x - width / 2) / (shrink * scale * zoom);
        centerLat = lat + (y - height / 2) / (scale * zoom);
        settle();
    }

    onArrowsChanged: face.requestPaint()
    onHighlightChanged: face.requestPaint()
    onZoomChanged: face.requestPaint()
    onCenterLatChanged: face.requestPaint()
    onCenterLonChanged: face.requestPaint()
    onPlacedChanged: face.requestPaint()
    onWidthChanged: { settle(); face.requestPaint(); }
    onHeightChanged: { settle(); face.requestPaint(); }

    Canvas {
        id: face
        anchors.fill: parent
        renderStrategy: Canvas.Cooperative

        onPaint: {
            const g = getContext("2d");
            g.reset();
            g.clearRect(0, 0, width, height);
            if (!bay.arrows.length) {
                g.fillStyle = bay.ink;
                g.globalAlpha = 0.5;
                g.font = bay.textSize + "px monospace";
                g.fillText("waiting for the bay…", 12, height / 2);
                return;
            }
            bay.drawCoast(g);
            g.font = bay.textSize + "px monospace";
            // The bay's narrows crowd together off Angel Island, so the
            // arrows all go down first and the names afterwards, and a
            // name that would land on one already written is left out.
            // The strongest stream wins the room, and whatever the
            // pointer is on always gets its name.
            const order = bay.arrows.slice().sort((a, b) => b.knots - a.knots);
            for (let i = 0; i < order.length; i++) bay.draw(g, order[i]);
            bay.taken = [];
            const pointed = order.filter(a => a.station === bay.highlight);
            for (let i = 0; i < pointed.length; i++) bay.label(g, pointed[i], true);
            for (let i = 0; i < order.length; i++) {
                if (order[i].station !== bay.highlight) bay.label(g, order[i], false);
            }
            bay.legend(g);
        }
    }

    // Rectangles a name has already been written into, this repaint.
    property var taken: []

    // The bay's shore, behind everything. Faint on purpose: it is there
    // to say which water an arrow is in, not to be read itself.
    function drawCoast(g) {
        if (!bounds) return;
        g.strokeStyle = ink;
        g.globalAlpha = 0.38;
        g.lineWidth = 1;
        g.lineJoin = "round";
        g.lineCap = "round";
        const lines = Coast.LINES;
        for (let i = 0; i < lines.length; i++) {
            const line = lines[i];
            g.beginPath();
            // Each line is longitude, latitude, longitude, latitude...
            for (let j = 0; j + 1 < line.length; j += 2) {
                const x = xOf(line[j]), y = yOf(line[j + 1]);
                if (j === 0) g.moveTo(x, y);
                else g.lineTo(x, y);
            }
            g.stroke();
        }
        g.globalAlpha = 1;
    }

    function draw(g, a) {
        const x = xOf(a.lon), y = yOf(a.lat);
        const flood = a.way === "flood";
        const slack = a.way === "slack" || a.knots < 0.05;
        const on = highlight === a.station;
        // Longest arrow is 27 px; the square root keeps a weak stream
        // visible without letting a strong one swamp the chart. Shorter
        // than it could be, because the bay's narrows crowd together off
        // Angel Island and long arrows there tangle into a thicket.
        const length = 7 + 20 * Math.sqrt(Math.min(1, a.knots / fastest));
        g.strokeStyle = slack ? ink : (flood ? accent : warn);
        g.fillStyle = g.strokeStyle;
        g.globalAlpha = on ? 1 : 0.85;
        if (slack || typeof a.setDeg !== "number") {
            // Slack: a ring, not an arrow, because there is no way to it.
            g.lineWidth = 1.5;
            g.globalAlpha = 0.7;
            g.beginPath();
            g.arc(x, y, 4, 0, 2 * Math.PI);
            g.stroke();
        } else {
            // Compass degrees: north is up the screen.
            const r = a.setDeg * Math.PI / 180;
            const dx = Math.sin(r), dy = -Math.cos(r);
            const tipX = x + dx * length, tipY = y + dy * length;
            g.lineWidth = on ? 3 : 2;
            g.beginPath();
            g.moveTo(x - dx * 3, y - dy * 3);
            g.lineTo(tipX, tipY);
            g.stroke();
            // The head.
            const wing = 5;
            g.beginPath();
            g.moveTo(tipX, tipY);
            g.lineTo(tipX - dx * wing * 1.8 + dy * wing, tipY - dy * wing * 1.8 - dx * wing);
            g.lineTo(tipX - dx * wing * 1.8 - dy * wing, tipY - dy * wing * 1.8 + dx * wing);
            g.closePath();
            g.fill();
        }
        g.globalAlpha = 1;
    }

    // The name and the speed, on whichever side has room, unless another
    // name is there already.
    function label(g, a, always) {
        const x = xOf(a.lon), y = yOf(a.lat);
        const slack = a.way === "slack" || a.knots < 0.05;
        const speed = slack ? "slack" : a.knots.toFixed(1) + " kn";
        const wide = Math.max(a.place.length, speed.length) * textSize * 0.62;
        const tall = textSize * 2.4;
        // On the side the stream is coming from, so a name never lies
        // under its own arrow; away from the middle of the chart when
        // there is no arrow to avoid.
        const right = typeof a.setDeg === "number" && !slack
            ? Math.sin(a.setDeg * Math.PI / 180) < 0
            : x <= width / 2;
        const side = right ? 7 : -7 - wide;
        const box = {x: x + side, y: y - textSize - 4, w: wide, h: tall};
        if (!always && (overlaps(box) || box.x < 0 || box.x + box.w > width)) return;
        taken.push(box);
        g.textAlign = right ? "left" : "right";
        const anchor = right ? x + 7 : x - 7;
        g.globalAlpha = always ? 1 : 0.72;
        g.fillStyle = ink;
        g.fillText(a.place, anchor, y - 4);
        g.globalAlpha = always ? 1 : 0.85;
        g.fillStyle = slack ? ink : (a.way === "flood" ? accent : warn);
        g.fillText(speed, anchor, y + 8);
        g.globalAlpha = 1;
    }

    function overlaps(box) {
        for (let i = 0; i < taken.length; i++) {
            const t = taken[i];
            if (box.x < t.x + t.w && t.x < box.x + box.w
                && box.y < t.y + t.h && t.y < box.y + box.h) return true;
        }
        return false;
    }

    function legend(g) {
        g.textAlign = "left";
        g.globalAlpha = 0.7;
        g.fillStyle = accent;
        g.fillText("▲ flood", 8, height - 20);
        g.fillStyle = warn;
        g.fillText("▼ ebb", 8, height - 8);
        g.fillStyle = ink;
        g.textAlign = "right";
        g.fillText("longest arrow " + fastest.toFixed(1) + " kn", width - 8, height - 8);
        if (zoomed) {
            g.fillText("×" + zoom.toFixed(1) + "  double-click for the whole bay",
                       width - 8, height - 20);
        }
        g.globalAlpha = 1;
    }

    MouseArea {
        id: input
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        // Where a press started, and whether it has become a drag. A
        // press that never moves is a click on a place; one that moves is
        // a pan, and then it mustn't also pick.
        property point from: Qt.point(0, 0)
        property real fromLat: 0
        property real fromLon: 0
        property bool dragging: false

        onPressed: mouse => {
            from = Qt.point(mouse.x, mouse.y);
            fromLat = bay.latAt(mouse.y);
            fromLon = bay.lonAt(mouse.x);
            dragging = false;
        }
        onPositionChanged: mouse => {
            if (!pressed) {
                bay.highlight = bay.nearest(mouse.x, mouse.y);
                return;
            }
            if (!dragging && Math.hypot(mouse.x - from.x, mouse.y - from.y) < 4) return;
            dragging = true;
            if (!bay.zoomed) return;
            // Put the place the press started on back under the pointer.
            bay.placed = true;
            bay.centerLon = fromLon - (mouse.x - bay.width / 2) / (bay.shrink * bay.scale * bay.zoom);
            bay.centerLat = fromLat + (mouse.y - bay.height / 2) / (bay.scale * bay.zoom);
            bay.settle();
        }
        onReleased: {
            if (!dragging && bay.highlight) bay.picked(bay.highlight);
        }
        onExited: bay.highlight = ""
        onWheel: wheel => {
            const steps = wheel.angleDelta.y / 120;
            if (!steps) return;
            bay.zoomAt(Math.pow(1.25, steps), wheel.x, wheel.y);
        }
        onDoubleClicked: bay.home()
        cursorShape: bay.zoomed ? (pressed && dragging ? Qt.ClosedHandCursor : Qt.OpenHandCursor)
                                : Qt.ArrowCursor
    }

    // The place under the pointer, within 24 pixels.
    function nearest(x, y) {
        let best = "", closest = 24;
        for (let i = 0; i < arrows.length; i++) {
            const a = arrows[i];
            const d = Math.hypot(xOf(a.lon) - x, yOf(a.lat) - y);
            if (d < closest) {
                closest = d;
                best = a.station;
            }
        }
        return best;
    }

    // What the highlighted place is called, and what to know about it.
    readonly property var pointedAt: {
        for (let i = 0; i < arrows.length; i++) {
            if (arrows[i].station === highlight) return arrows[i];
        }
        return null;
    }
}
