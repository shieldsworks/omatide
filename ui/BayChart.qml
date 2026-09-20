import QtQuick

// The stream everywhere in San Francisco Bay at one moment.
//
// This is the old tidal current chart, drawn from NOAA's own survey: an
// arrow at each of the bay's narrows, pointing the way the stream sets
// and as long as it is strong. The bay's shape comes out of where the
// stations are, because the stations are in the channels.
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
    readonly property var bounds: {
        if (!arrows.length) return null;
        let s = 90, n = -90, w = 180, e = -180;
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
    readonly property real offsetX: bounds
        ? (width - (bounds.e - bounds.w) * shrink * scale) / 2 : 0
    readonly property real offsetY: bounds
        ? (height - (bounds.n - bounds.s) * scale) / 2 : 0

    function xOf(lon) {
        return bounds ? offsetX + (lon - bounds.w) * shrink * scale : 0;
    }
    function yOf(lat) {
        return bounds ? offsetY + (bounds.n - lat) * scale : 0;
    }

    onArrowsChanged: face.requestPaint()
    onHighlightChanged: face.requestPaint()
    onWidthChanged: face.requestPaint()
    onHeightChanged: face.requestPaint()

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

    function draw(g, a) {
        const x = xOf(a.lon), y = yOf(a.lat);
        const flood = a.way === "flood";
        const slack = a.way === "slack" || a.knots < 0.05;
        const on = highlight === a.station;
        // Longest arrow is 34 px; the square root keeps a weak stream
        // visible without letting a strong one swamp the chart.
        const length = 8 + 26 * Math.sqrt(Math.min(1, a.knots / fastest));
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
        g.globalAlpha = 1;
    }

    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        onPositionChanged: mouse => bay.highlight = bay.nearest(mouse.x, mouse.y)
        onExited: bay.highlight = ""
        onClicked: {
            if (bay.highlight) bay.picked(bay.highlight);
        }
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
