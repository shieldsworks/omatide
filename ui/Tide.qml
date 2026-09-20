pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// The connection to the tide engine, `omatide run`, shared by the bar on
// every monitor and by the window. The protocol is docs/protocol.md:
// newline-delimited JSON, version 1. When the engine isn't running this
// starts it, detached.
//
// The engine needs no network once its stations are fetched, so unlike
// omawind there is nothing here about downloads: a tide that can be
// predicted at the dock can be predicted at sea.
QtObject {
    id: tide

    readonly property int version: 1
    readonly property string runtime: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/omatide/"
    readonly property string path: runtime + "tide.sock"
    // The checkout or plugin directory: this file is <repo>/ui/Tide.qml.
    readonly property string repo: decodeURIComponent(String(Qt.resolvedUrl("..")).replace(/^file:\/\//, "")).replace(/\/$/, "")
    readonly property string binary: Quickshell.env("OMATIDE_BIN") || repo + "/target/release/omatide"
    readonly property string log: runtime + "engine.log"

    // The latest `state`; null while disconnected.
    property var state: null
    property bool incompatible: false
    property bool waited: false
    property int attempts: 0
    property string lastLog: ""
    readonly property bool connected: socket !== null && socket.connected

    readonly property var here: state ? state.here : null
    readonly property var water: state && isPlace(state.tide) ? state.tide : null
    readonly property var stream: state && isPlace(state.current) ? state.current : null
    readonly property var bay: state && Array.isArray(state.bay) ? state.bay.filter(isArrow) : []
    readonly property var problems: state && Array.isArray(state.problems) ? state.problems : []
    readonly property bool hasCatalog: !!state && !!state.stations
        && (state.stations.tide > 0 || state.stations.current > 0)

    // Nothing from the engine is trusted to be the right shape: a broken
    // engine must not be able to break the window.
    function num(v, lo, hi) {
        return typeof v === "number" && isFinite(v) && v >= lo && v <= hi;
    }
    function isPlace(p) {
        return p !== null && typeof p === "object" && typeof p.station === "string"
            && num(p.lat, -90, 90) && num(p.lon, -180, 180) && Array.isArray(p.turns);
    }
    function isArrow(a) {
        return a !== null && typeof a === "object" && typeof a.place === "string"
            && num(a.lat, -90, 90) && num(a.lon, -180, 180) && num(a.knots, 0, 30);
    }
    // Turns that can be drawn, in time order.
    function turnsOf(place, key) {
        if (!place || !Array.isArray(place.turns)) return [];
        return place.turns.filter(t => t !== null && typeof t === "object"
            && typeof t.turn === "string" && typeof t.time === "string"
            && !isNaN(Date.parse(t.time))
            && (t[key] === undefined || num(t[key], -100, 100)));
    }

    function receive(line) {
        let m;
        try {
            m = JSON.parse(line);
        } catch (e) {
            return;
        }
        if (m === null || typeof m !== "object" || typeof m.v !== "number") return;
        if (tide.incompatible) return;
        if (m.v !== tide.version) {
            tide.incompatible = true;
            tide.state = null;
            tide.socket.connected = false;
            return;
        }
        if (m.type === "state" && m.here !== null && typeof m.here === "object") {
            tide.state = m;
        } else if (m.type === "curve" || m.type === "bay" || m.type === "stations"
                   || m.type === "error") {
            // Answers to a request, matched by the id that was sent.
            tide.answered(m);
        }
    }

    signal answered(var message)

    // Sends a request and calls back once, with the answer or with null
    // if nothing came within 10 seconds.
    property int lastId: 0
    function ask(request, then) {
        if (!socket || !socket.connected) {
            then(null);
            return;
        }
        const id = ++lastId;
        request.id = id;
        const guard = waiterFactory.createObject(tide, {wanted: id, then: then});
        socket.write(JSON.stringify(request) + "\n");
        socket.flush();
    }
    property Component waiterFactory: Component {
        QtObject {
            id: waiter
            property int wanted: 0
            property var then: null
            property Connections link: Connections {
                target: tide
                function onAnswered(m) {
                    if (m.id !== waiter.wanted) return;
                    waiter.finish(m.type === "error" ? null : m);
                }
            }
            property Timer giveUp: Timer {
                interval: 10000
                running: true
                onTriggered: waiter.finish(null)
            }
            function finish(m) {
                const call = waiter.then;
                waiter.then = null;
                waiter.destroy();
                if (call) call(m);
            }
        }
    }

    // A protocol time in local time: "14:00", or "Mon 14:00".
    function clock(iso, withDay) {
        const d = new Date(iso);
        return isNaN(d.getTime()) ? "" : Qt.formatDateTime(d, withDay ? "ddd HH:mm" : "HH:mm");
    }
    // "in 1h 45m", or "45m ago".
    function until(iso, now) {
        const d = Date.parse(iso);
        if (isNaN(d)) return "";
        const minutes = Math.round((d - now) / 60000);
        const size = Math.abs(minutes);
        const text = size < 60 ? size + "m"
            : Math.floor(size / 60) + "h " + String(size % 60).padStart(2, "0") + "m";
        return minutes >= 0 ? "in " + text : text + " ago";
    }
    // Sixteen points of the compass.
    function compass(deg) {
        const points = ["N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE",
                        "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW"];
        return points[Math.round((((deg % 360) + 360) % 360) / 22.5) % 16];
    }
    function rangeNm(lat1, lon1, lat2, lon2) {
        const r = Math.PI / 180;
        const a = Math.pow(Math.sin((lat2 - lat1) * r / 2), 2)
            + Math.cos(lat1 * r) * Math.cos(lat2 * r) * Math.pow(Math.sin((lon2 - lon1) * r / 2), 2);
        return 2 * 3440.065 * Math.asin(Math.min(1, Math.sqrt(a)));
    }

    // argv, never shell text built from paths. The engine keeps one copy
    // running through its lock file, so a second start is harmless.
    function start() {
        const script = 'mkdir -p -m 700 "$1" && b="$2"; [ -x "$b" ] || b=omatide; exec "$b" run 2>>"$3"';
        Quickshell.execDetached(["env", "-C", Quickshell.env("HOME") || "/", "sh", "-c", script,
                                 "omatide-start", tide.runtime, tide.binary, tide.log]);
    }

    property var socket: socketFactory.createObject(tide)
    property Component socketFactory: Component {
        Socket {
            path: tide.path
            connected: true
            parser: SplitParser {
                onRead: data => tide.receive(data)
            }
            onConnectedChanged: {
                if (connected) {
                    tide.attempts = 0;
                    tide.waited = false;
                } else {
                    tide.state = null;
                }
            }
        }
    }

    // A failed connect leaves Quickshell's socket allocated, and toggling
    // `connected` can't retry it, so each retry is a fresh Socket. The
    // engine is started on the first failure and again every 20 tries.
    property Timer reconnect: Timer {
        interval: tide.attempts < 10 ? 1000 : 3000
        repeat: true
        running: tide.socket !== null && !tide.socket.connected && !tide.incompatible
        onTriggered: {
            tide.attempts += 1;
            if (tide.attempts % 20 === 1) tide.start();
            if (tide.attempts >= 6) tide.waited = true;
            tide.logFile.reload();
            const previous = tide.socket;
            tide.socket = tide.socketFactory.createObject(tide);
            previous.destroy();
        }
    }

    property FileView logFile: FileView {
        path: tide.log
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            const lines = text().trim().split("\n");
            tide.lastLog = lines.length ? lines[lines.length - 1] : "";
        }
    }
}
