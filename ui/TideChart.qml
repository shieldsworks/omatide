import QtQuick

// The tide curve: height above chart datum against time, with the highs
// and lows marked, now where now is, and a line you can drag to read the
// height at any moment.
//
// The same chart draws a current, which is the other thing that rises and
// falls: for a stream the zero line is slack water, the flood is above it
// and the ebb below, and the two are shaded differently because getting
// them the wrong way round is the mistake that costs you the tide.
Item {
    id: chart

    // A `curve` answer from the engine.
    property var curve: null
    property var theme: null
    // Milliseconds, so the now line keeps up.
    property real now: Date.now()
    // Where the reader is pointing, in milliseconds, or 0 for nowhere.
    property real scrubAt: 0
    readonly property bool isCurrent: !!curve && curve.kind === "current"

    signal scrubbed(real at)

    readonly property color ink: theme ? theme.foreground : "#a9b1d6"
    readonly property color accent: theme ? theme.accent : "#7aa2f7"
    readonly property color warn: theme ? theme.red : "#f7768e"
    readonly property color paper: theme ? theme.background : "#1a1b26"
    readonly property int textSize: theme ? Math.max(9, theme.baseSize - 1) : 11

    // The window the curve covers, in milliseconds.
    readonly property real start: curve ? Date.parse(curve.start) : 0
    readonly property real step: curve ? curve.stepSeconds * 1000 : 600000
    readonly property var values: curve && Array.isArray(curve.values) ? curve.values : []
    readonly property real span: values.length > 1 ? (values.length - 1) * step : 0
    readonly property bool ready: values.length > 1 && isFinite(start) && start > 0

    // The value at a moment, interpolated between the points either side.
    function valueAt(when) {
        if (!ready) return NaN;
        const at = (when - start) / step;
        if (at < 0 || at > values.length - 1) return NaN;
        const i = Math.floor(at);
        const a = values[i];
        const b = values[Math.min(i + 1, values.length - 1)];
        return typeof a === "number" && typeof b === "number" ? a + (b - a) * (at - i) : NaN;
    }

    // Margins: room on the left for the scale and at the bottom for the
    // clock.
    readonly property real leftPad: 34
    readonly property real bottomPad: 20
    readonly property real topPad: 18

    // The range drawn, rounded out to whole units so the grid is tidy.
    readonly property var extent: {
        if (!ready) return {low: 0, high: 1};
        let low = Infinity, high = -Infinity;
        for (let i = 0; i < values.length; i++) {
            const v = values[i];
            if (typeof v !== "number" || !isFinite(v)) continue;
            low = Math.min(low, v);
            high = Math.max(high, v);
        }
        if (!isFinite(low)) return {low: 0, high: 1};
        // A stream is drawn about slack, so flood and ebb are comparable.
        if (isCurrent) {
            const most = Math.max(Math.abs(low), Math.abs(high), 0.5);
            const edge = Math.ceil(most * 2) / 2 + 0.25;
            return {low: -edge, high: edge};
        }
        // A tide always shows the chart datum, because that is what a
        // depth on the chart is measured from.
        low = Math.min(0, low);
        const pad = Math.max(0.25, (high - low) * 0.18);
        return {low: Math.floor((low - pad) * 2) / 2, high: Math.ceil((high + pad) * 2) / 2};
    }

    function xOf(when) {
        return leftPad + (when - start) / span * (width - leftPad - 4);
    }
    function yOf(v) {
        const e = extent;
        return height - bottomPad - (v - e.low) / (e.high - e.low) * (height - bottomPad - topPad);
    }
    function whenOf(x) {
        return start + (x - leftPad) / (width - leftPad - 4) * span;
    }

    onCurveChanged: face.requestPaint()
    onNowChanged: face.requestPaint()
    // As the bay's chart does: a theme change repaints, rather than waiting
    // for the next minute's now line to carry the new colors in.
    onInkChanged: face.requestPaint()
    onAccentChanged: face.requestPaint()
    onWarnChanged: face.requestPaint()
    onPaperChanged: face.requestPaint()
    onTextSizeChanged: face.requestPaint()
    onScrubAtChanged: face.requestPaint()
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
            if (!chart.ready) {
                g.fillStyle = chart.ink;
                g.globalAlpha = 0.5;
                g.font = chart.textSize + "px monospace";
                g.fillText(chart.curve ? "no curve" : "waiting for omatide…", chart.leftPad, height / 2);
                return;
            }
            chart.grid(g);
            chart.water(g);
            chart.marks(g);
            chart.lines(g);
        }
    }

    // Horizontal lines every half unit, and a day line at local midnight.
    function grid(g) {
        const e = extent;
        g.font = textSize + "px monospace";
        g.textBaseline = "middle";
        const stepUnit = (e.high - e.low) > 3 ? 1 : 0.5;
        for (let v = Math.ceil(e.low / stepUnit) * stepUnit; v <= e.high + 1e-9; v += stepUnit) {
            const y = yOf(v);
            const datum = Math.abs(v) < 1e-9;
            g.strokeStyle = ink;
            g.globalAlpha = datum ? 0.45 : 0.12;
            g.lineWidth = 1;
            g.beginPath();
            g.moveTo(leftPad, Math.round(y) + 0.5);
            g.lineTo(width - 4, Math.round(y) + 0.5);
            g.stroke();
            g.globalAlpha = datum ? 0.8 : 0.5;
            g.fillStyle = ink;
            g.textAlign = "right";
            g.fillText(v.toFixed(stepUnit < 1 ? 1 : 0), leftPad - 5, y);
        }
        // Midnight, and the day it starts.
        g.textBaseline = "alphabetic";
        g.textAlign = "left";
        const first = new Date(start);
        first.setHours(0, 0, 0, 0);
        for (let d = first.getTime(); d <= start + span; d += 86400000) {
            if (d < start) continue;
            const x = xOf(d);
            g.strokeStyle = ink;
            g.globalAlpha = 0.3;
            g.beginPath();
            g.moveTo(Math.round(x) + 0.5, topPad);
            g.lineTo(Math.round(x) + 0.5, height - bottomPad);
            g.stroke();
            g.globalAlpha = 0.6;
            g.fillStyle = ink;
            g.fillText(Qt.formatDateTime(new Date(d), "ddd d MMM"), x + 3, topPad - 6);
        }
        // The clock along the bottom, every three hours where it fits.
        const hours = span / 3600000;
        const every = hours <= 30 ? 3 : hours <= 80 ? 12 : 24;
        const firstHour = new Date(start);
        firstHour.setMinutes(0, 0, 0);
        g.globalAlpha = 0.55;
        g.textAlign = "center";
        for (let t = firstHour.getTime(); t <= start + span; t += 3600000) {
            const when = new Date(t);
            if (when.getHours() % every !== 0) continue;
            const x = xOf(t);
            if (x < leftPad + 10 || x > width - 14) continue;
            g.fillText(Qt.formatDateTime(when, "HH"), x, height - 5);
        }
        g.globalAlpha = 1;
    }

    // The curve, and the water (or the stream) under it.
    function water(g) {
        const zero = yOf(isCurrent ? 0 : extent.low);
        // A stream is shaded from slack, so a glance says flood or ebb.
        if (isCurrent) {
            paintBand(g, true);
            paintBand(g, false);
        } else {
            g.beginPath();
            g.moveTo(xOf(start), zero);
            for (let i = 0; i < values.length; i++) g.lineTo(xOf(start + i * step), yOf(values[i]));
            g.lineTo(xOf(start + span), zero);
            g.closePath();
            g.fillStyle = accent;
            g.globalAlpha = 0.22;
            g.fill();
        }
        g.beginPath();
        for (let i = 0; i < values.length; i++) {
            const x = xOf(start + i * step), y = yOf(values[i]);
            if (i === 0) g.moveTo(x, y);
            else g.lineTo(x, y);
        }
        g.globalAlpha = 1;
        g.strokeStyle = accent;
        g.lineWidth = 2;
        g.lineJoin = "round";
        g.stroke();
    }

    // One side of a stream: the flood above slack, or the ebb below it.
    function paintBand(g, flood) {
        const zero = yOf(0);
        g.beginPath();
        g.moveTo(xOf(start), zero);
        for (let i = 0; i < values.length; i++) {
            const v = values[i];
            const keep = flood ? Math.max(0, v) : Math.min(0, v);
            g.lineTo(xOf(start + i * step), yOf(keep));
        }
        g.lineTo(xOf(start + span), zero);
        g.closePath();
        g.fillStyle = flood ? accent : warn;
        g.globalAlpha = 0.22;
        g.fill();
    }

    // High and low water, or slack and maximum stream.
    //
    // Turns can fall minutes apart where a stream barely reverses, so a
    // label that would land on the one before it is left out. The dot
    // stays: the turn is still there, only its writing is crowded.
    function marks(g) {
        if (!curve || !Array.isArray(curve.turns)) return;
        g.font = textSize + "px monospace";
        let lastAbove = -1e9, lastBelow = -1e9;
        for (let i = 0; i < curve.turns.length; i++) {
            const t = curve.turns[i];
            if (!t || typeof t.time !== "string") continue;
            const when = Date.parse(t.time);
            if (isNaN(when) || when < start || when > start + span) continue;
            const value = isCurrent
                ? (t.turn === "slack" ? 0 : (t.turn === "ebb" ? -t.knots : t.knots))
                : t.heightM;
            if (typeof value !== "number" || !isFinite(value)) continue;
            const x = xOf(when), y = yOf(value);
            const up = t.turn === "high" || t.turn === "flood";
            g.fillStyle = ink;
            g.globalAlpha = 0.9;
            g.beginPath();
            g.arc(x, y, 2.5, 0, 2 * Math.PI);
            g.fill();
            // Labels sit outside the curve, so they never cover it, and
            // above or below by which way the turn went.
            const room = textSize * 4.2;
            if (up) {
                if (x - lastAbove < room) continue;
                lastAbove = x;
            } else {
                if (x - lastBelow < room) continue;
                lastBelow = x;
            }
            g.globalAlpha = 0.75;
            g.textAlign = x > width - 52 ? "right" : x < leftPad + 40 ? "left" : "center";
            const label = Qt.formatDateTime(new Date(when), "HH:mm");
            const size = isCurrent
                ? (t.turn === "slack" ? "slack" : t.knots.toFixed(1) + " kn")
                : t.heightM.toFixed(2) + " m";
            g.fillText(label, x, up ? y - 16 : y + 24);
            g.fillText(size, x, up ? y - 6 : y + 34);
        }
        g.globalAlpha = 1;
    }

    // Now, and wherever the reader is pointing.
    function lines(g) {
        g.lineWidth = 1;
        if (now >= start && now <= start + span) {
            const x = xOf(now);
            g.strokeStyle = warn;
            g.globalAlpha = 0.9;
            g.beginPath();
            g.moveTo(Math.round(x) + 0.5, topPad - 4);
            g.lineTo(Math.round(x) + 0.5, height - bottomPad);
            g.stroke();
            const v = valueAt(now);
            if (!isNaN(v)) {
                g.fillStyle = warn;
                g.beginPath();
                g.arc(x, yOf(v), 4, 0, 2 * Math.PI);
                g.fill();
            }
        }
        if (scrubAt > start && scrubAt < start + span) {
            const x = xOf(scrubAt);
            g.strokeStyle = ink;
            g.globalAlpha = 0.6;
            g.beginPath();
            g.moveTo(Math.round(x) + 0.5, topPad - 4);
            g.lineTo(Math.round(x) + 0.5, height - bottomPad);
            g.stroke();
        }
        g.globalAlpha = 1;
    }

    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        onPositionChanged: mouse => chart.point(mouse.x)
        onPressed: mouse => chart.point(mouse.x)
        onExited: {
            chart.scrubAt = 0;
            chart.scrubbed(0);
        }
    }

    function point(x) {
        if (!ready) return;
        const when = Math.max(start, Math.min(start + span, whenOf(x)));
        scrubAt = when;
        scrubbed(when);
    }
}
