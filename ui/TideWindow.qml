import QtQuick
import Quickshell

// The tide in a window of its own: the height where the boat is, the
// stream at the nearest station, and the whole of San Francisco Bay at
// whatever moment you drag it to.
//
// Run standalone (ui/shell.qml) it owns its process; as the shell's panel
// the shell opens and hides it. The bar's popover stays the quick look.
Item {
    id: app

    // Set by the Omarchy shell when loaded as a panel.
    property var shell: null
    property var manifest: null
    property bool standalone: true
    property bool opened: standalone

    function open(payload) {
        opened = true;
        Qt.callLater(() => surface.forceActiveFocus());
    }
    function close() {
        opened = false;
    }
    function dismiss() {
        if (standalone) Qt.quit();
        else if (shell) shell.hide("org.omahoy.tide");
        else opened = false;
    }

    property Theme theme: Theme {}

    Connections {
        target: Quickshell
        function onLastWindowClosed() { if (app.standalone) Qt.quit(); }
    }

    // Now, kept fresh so the countdowns and the now line keep up.
    property real now: Date.now()
    Timer {
        interval: 15000
        repeat: true
        running: app.opened
        onTriggered: {
            app.now = Date.now();
            if (!app.scrubbing) app.at = app.now;
        }
    }

    // The moment the bay is drawn at. Starts at now and follows it until
    // the reader drags it somewhere else.
    property real at: Date.now()
    property bool scrubbing: false
    // Hours either side of now the scrubber covers.
    readonly property int scrubSpan: 12

    readonly property var water: Tide.water
    readonly property var stream: Tide.stream
    readonly property string where: {
        if (!Tide.here) return "";
        return Tide.here.at === "boat" ? "the boat" : "home";
    }

    // The curves on show, fetched from the engine and refetched when the
    // station changes or the day rolls over.
    property var tideCurve: null
    property var currentCurve: null
    property var bayNow: null
    property string tideAsked: ""
    property string currentAsked: ""

    function refresh() {
        if (!Tide.connected) return;
        const from = Qt.formatDateTime(new Date(app.now - 6 * 3600000), "yyyy-MM-ddTHH:00:00Z");
        if (water) {
            const want = water.station + from;
            if (want !== tideAsked) {
                tideAsked = want;
                Tide.ask({type: "curve", station: water.station, hours: 30,
                          time: isoHourAgo(6), stepSeconds: 360},
                         m => { if (m) app.tideCurve = m; });
            }
        }
        if (stream) {
            const want = stream.station + from;
            if (want !== currentAsked) {
                currentAsked = want;
                Tide.ask({type: "curve", station: stream.station, hours: 30,
                          time: isoHourAgo(6), stepSeconds: 360},
                         m => { if (m) app.currentCurve = m; });
            }
        }
    }
    function isoHourAgo(hours) {
        const d = new Date(app.now - hours * 3600000);
        d.setMinutes(0, 0, 0);
        return d.toISOString().replace(/\.\d+Z$/, "Z");
    }

    // The bay at the scrubbed moment. Asking on every pixel of a drag
    // would flood the socket, so a timer collects the moves.
    Timer {
        id: bayAsk
        interval: 120
        onTriggered: {
            const iso = new Date(app.at).toISOString().replace(/\.\d+Z$/, "Z");
            Tide.ask({type: "bay", time: iso}, m => {
                if (m && Array.isArray(m.current)) app.bayNow = m;
            });
        }
    }
    onAtChanged: bayAsk.restart()
    onOpenedChanged: if (opened) { refresh(); bayAsk.restart(); }

    Connections {
        target: Tide
        function onStateChanged() { app.refresh(); }
        function onConnectedChanged() {
            app.tideAsked = "";
            app.currentAsked = "";
            app.refresh();
            bayAsk.restart();
        }
    }

    // Live when not scrubbing, otherwise whatever was fetched.
    readonly property var bayArrows: {
        if (!scrubbing && Math.abs(at - now) < 60000) return Tide.bay;
        return bayNow && Array.isArray(bayNow.current) ? bayNow.current.filter(Tide.isArrow) : [];
    }

    FloatingWindow {
        id: win
        visible: app.opened
        title: "Omatide"
        implicitWidth: Number(Quickshell.env("OMATIDE_WIDTH")) || 1040
        implicitHeight: Number(Quickshell.env("OMATIDE_HEIGHT")) || 780
        color: app.theme.background
        onVisibleChanged: {
            if (!visible && app.opened) app.dismiss();
            else if (visible) Qt.callLater(() => surface.forceActiveFocus());
        }

        Item {
            id: surface
            anchors.fill: parent
            anchors.margins: 12
            focus: true
            Keys.onPressed: e => {
                if (e.key === Qt.Key_Escape || e.key === Qt.Key_Q) app.dismiss();
                else if (e.key === Qt.Key_N) app.theme.night = !app.theme.night;
                else if (e.key === Qt.Key_BracketLeft) app.stepBay(-1);
                else if (e.key === Qt.Key_BracketRight) app.stepBay(1);
                else if (e.key === Qt.Key_0 || e.key === Qt.Key_Home) app.followNow();
                else return;
                e.accepted = true;
            }

            component Line: Text {
                color: app.theme.foreground
                font.family: "monospace"
                font.pixelSize: app.theme.baseSize
                elide: Text.ElideRight
            }

            // ------------------------------------------------ the top
            Row {
                id: head
                anchors.left: parent.left
                anchors.right: parent.right
                spacing: 10

                Line {
                    text: "OMATIDE"
                    font.bold: true
                    color: app.theme.accent
                }
                Line {
                    width: head.width - 240
                    text: {
                        if (Tide.incompatible) return "omatide speaks another protocol version";
                        if (!Tide.connected) return Tide.waited
                            ? (Tide.lastLog || "omatide isn't running") : "Starting omatide…";
                        if (!Tide.hasCatalog) return "No stations yet — run `omatide fetch`";
                        if (!app.water) return "No tide station within reach";
                        return "The tide at " + app.where + ", from " + app.water.name
                            + " " + app.water.distanceNm.toFixed(1) + " nm off";
                    }
                    opacity: 0.8
                }
                Item { width: 1; height: 1 }
                Rectangle {
                    width: night.implicitWidth + 12
                    height: night.implicitHeight + 6
                    color: "transparent"
                    border.color: app.theme.foreground
                    border.width: 1
                    opacity: app.theme.night ? 1 : 0.4
                    radius: 3
                    Line { id: night; anchors.centerIn: parent; text: "NIGHT" }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: app.theme.night = !app.theme.night
                    }
                }
            }

            // ------------------------------------------- the big number
            Row {
                id: readout
                anchors.top: head.bottom
                anchors.topMargin: 10
                anchors.left: parent.left
                anchors.right: parent.right
                spacing: 28

                Column {
                    spacing: 2
                    Line {
                        text: app.water ? app.water.heightM.toFixed(2) + " m" : "–"
                        font.pixelSize: app.theme.baseSize * 2.4
                        font.bold: true
                    }
                    Line {
                        opacity: 0.75
                        text: {
                            if (!app.water) return "";
                            const turns = Tide.turnsOf(app.water, "heightM");
                            const next = turns.length ? turns[0] : null;
                            const way = app.water.rising ? "rising" : "falling";
                            if (!next) return way;
                            return way + " · " + next.turn + " water "
                                + Tide.clock(next.time) + " " + Tide.until(next.time, app.now);
                        }
                    }
                    Line {
                        opacity: 0.55
                        text: app.water ? "above chart datum (MLLW)" : ""
                    }
                }
                Column {
                    spacing: 2
                    Line {
                        text: app.stream
                            ? app.stream.knots.toFixed(1) + " kn " + app.stream.way
                            : "–"
                        font.pixelSize: app.theme.baseSize * 2.4
                        font.bold: true
                        color: !app.stream ? app.theme.foreground
                            : app.stream.way === "ebb" ? app.theme.red : app.theme.accent
                    }
                    Line {
                        opacity: 0.75
                        text: {
                            if (!app.stream) return "";
                            const turns = Tide.turnsOf(app.stream, "knots");
                            const next = turns.length ? turns[0] : null;
                            const set = typeof app.stream.setDeg === "number"
                                ? "setting " + Tide.compass(app.stream.setDeg) + " "
                                  + String(Math.round(app.stream.setDeg)).padStart(3, "0") + "°T"
                                : "";
                            if (!next) return set;
                            return set + " · " + next.turn + " " + Tide.clock(next.time)
                                + " " + Tide.until(next.time, app.now);
                        }
                    }
                    Line {
                        opacity: 0.55
                        text: app.stream
                            ? app.stream.name + (typeof app.stream.depthM === "number"
                                ? " at " + app.stream.depthM.toFixed(1) + " m" : "")
                            : ""
                    }
                }
            }

            // ---------------------------------------------- the curves
            Row {
                id: charts
                anchors.top: readout.bottom
                anchors.topMargin: 12
                anchors.left: parent.left
                anchors.right: parent.right
                height: (parent.height - readout.height - head.height - 60) * 0.42
                spacing: 12

                TideChart {
                    width: (charts.width - 12) / 2
                    height: charts.height
                    curve: app.tideCurve
                    theme: app.theme
                    now: app.now
                }
                TideChart {
                    width: (charts.width - 12) / 2
                    height: charts.height
                    curve: app.currentCurve
                    theme: app.theme
                    now: app.now
                }
            }

            // ------------------------------------------------- the bay
            Line {
                id: bayLabel
                anchors.top: charts.bottom
                anchors.topMargin: 8
                anchors.left: parent.left
                anchors.right: parent.right
                text: {
                    const p = map.pointedAt;
                    if (p) return p.place + " — " + p.note;
                    const when = Math.abs(app.at - app.now) < 60000
                        ? "now" : Tide.clock(new Date(app.at).toISOString(), true);
                    return "The stream through San Francisco Bay, " + when
                        + "   ·   drag the bar, [ and ] step an hour, 0 back to now";
                }
                opacity: map.pointedAt ? 0.95 : 0.6
            }

            BayChart {
                id: map
                anchors.top: bayLabel.bottom
                anchors.topMargin: 4
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: scrub.top
                anchors.bottomMargin: 8
                arrows: app.bayArrows
                theme: app.theme
            }

            // --------------------------------------------- the scrubber
            Item {
                id: scrub
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 26

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width
                    height: 4
                    radius: 2
                    color: app.theme.foreground
                    opacity: 0.2
                }
                // Now.
                Rectangle {
                    x: scrub.width / 2 - 1
                    y: 3
                    width: 2
                    height: 14
                    color: app.theme.red
                    opacity: 0.8
                }
                Rectangle {
                    id: handle
                    x: Math.max(0, Math.min(scrub.width - width,
                        scrub.width / 2 + (app.at - app.now) / (app.scrubSpan * 3600000)
                        * (scrub.width / 2) - width / 2))
                    y: 1
                    width: 10
                    height: 18
                    radius: 3
                    color: app.theme.accent
                }
                Text {
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    color: app.theme.foreground
                    opacity: 0.6
                    font.family: "monospace"
                    font.pixelSize: Math.max(9, app.theme.baseSize - 2)
                    text: "−" + app.scrubSpan + " h … +" + app.scrubSpan + " h"
                }
                MouseArea {
                    anchors.fill: parent
                    onPressed: mouse => app.scrubTo(mouse.x, scrub.width)
                    onPositionChanged: mouse => {
                        if (pressed) app.scrubTo(mouse.x, scrub.width);
                    }
                    onReleased: app.scrubbing = Math.abs(app.at - app.now) > 60000
                }
            }
        }
    }

    function scrubTo(x, wide) {
        const fraction = Math.max(-1, Math.min(1, (x - wide / 2) / (wide / 2)));
        app.scrubbing = true;
        app.at = app.now + fraction * app.scrubSpan * 3600000;
    }
    function stepBay(hours) {
        app.scrubbing = true;
        const limit = app.scrubSpan * 3600000;
        app.at = app.now + Math.max(-limit, Math.min(limit, app.at - app.now + hours * 3600000));
    }
    function followNow() {
        app.scrubbing = false;
        app.at = app.now;
    }
}
