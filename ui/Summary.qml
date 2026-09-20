import QtQuick
import qs.Commons

// The quick look: the tide and the stream where the boat is, the turns to
// come, and how the bay's narrows are running. j and k scroll; Escape
// closes.
Item {
    id: root
    focus: true

    signal closeRequested
    Keys.onPressed: e => {
        if (e.key === Qt.Key_Escape) root.closeRequested();
        else if (e.key === Qt.Key_J || e.key === Qt.Key_Down) root.scroll(1);
        else if (e.key === Qt.Key_K || e.key === Qt.Key_Up) root.scroll(-1);
        else return;
        e.accepted = true;
    }
    function scroll(rows) {
        const most = Math.max(0, list.contentHeight - list.height);
        list.contentY = Math.max(0, Math.min(most, list.contentY + rows * 3 * 24));
    }

    readonly property color ink: Color.foreground
    // Secondary lines are the text color, faded. The theme's muted color
    // is too dark to read on the popover.
    readonly property real faint: 0.65
    readonly property var water: Tide.water
    readonly property var stream: Tide.stream

    property real now: Date.now()
    Timer { interval: 20000; repeat: true; running: true; onTriggered: root.now = Date.now() }

    component Line: Text {
        width: parent ? parent.width : 0
        color: root.ink
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
    }

    Column {
        id: top
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 4

        Line {
            text: {
                if (Tide.incompatible) return "omatide speaks another protocol version";
                if (!Tide.connected) return Tide.waited ? "omatide isn't running" : "Starting omatide…";
                if (!Tide.hasCatalog) return "No stations yet";
                if (!Tide.here) return "Tide";
                return Tide.here.at === "boat" ? "The tide at the boat" : "The tide at home";
            }
            font.pixelSize: Style.font.body
            font.bold: true
        }
        Line {
            visible: text !== ""
            text: !Tide.connected && Tide.waited && Tide.lastLog ? Tide.lastLog
                : !Tide.hasCatalog && Tide.connected ? "Run `omatide fetch` to download NOAA's stations"
                : ""
            opacity: root.faint
            wrapMode: Text.WordWrap
            maximumLineCount: 3
        }
        Line {
            visible: !!root.water
            text: root.water
                ? root.water.heightM.toFixed(2) + " m " + (root.water.rising ? "rising" : "falling")
                : ""
            font.pixelSize: Style.font.body + 6
            font.bold: true
        }
        Line {
            visible: !!root.water
            text: root.water
                ? root.water.name + " · " + root.water.distanceNm.toFixed(1) + " nm off"
                  + (root.water.follows ? " · follows " + root.water.follows : "")
                : ""
            opacity: root.faint
        }
        Line {
            visible: !!root.stream
            text: root.stream
                ? "Stream " + root.stream.knots.toFixed(1) + " kn " + root.stream.way
                  + (typeof root.stream.setDeg === "number"
                     ? " " + Tide.compass(root.stream.setDeg)
                       + " " + String(Math.round(root.stream.setDeg)).padStart(3, "0") + "°T" : "")
                : ""
            color: root.stream && root.stream.way === "ebb" ? Color.urgent : root.ink
            font.pixelSize: Style.font.body
        }
        Line {
            visible: !!root.stream
            text: root.stream ? root.stream.name : ""
            opacity: root.faint
        }
        Item { width: 1; height: 4 }
    }

    ListView {
        id: list
        anchors.top: top.bottom
        anchors.topMargin: 6
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        clip: true
        spacing: 2
        boundsBehavior: Flickable.StopAtBounds

        model: {
            const rows = [];
            const turns = Tide.turnsOf(root.water, "heightM").slice(0, 6);
            for (let i = 0; i < turns.length; i++) {
                rows.push({head: i === 0 ? "High and low water" : "",
                           left: (turns[i].turn === "high" ? "HW " : "LW ")
                                 + Tide.clock(turns[i].time, true),
                           right: turns[i].heightM.toFixed(2) + " m",
                           note: Tide.until(turns[i].time, root.now), urgent: false});
            }
            const slacks = Tide.turnsOf(root.stream, "knots").slice(0, 6);
            for (let i = 0; i < slacks.length; i++) {
                rows.push({head: i === 0 ? "Slack and maximum stream" : "",
                           left: slacks[i].turn.padEnd(6, " ") + Tide.clock(slacks[i].time, true),
                           right: slacks[i].turn === "slack" ? "" : slacks[i].knots.toFixed(1) + " kn",
                           note: Tide.until(slacks[i].time, root.now),
                           urgent: slacks[i].turn === "ebb"});
            }
            const bay = Tide.bay;
            for (let i = 0; i < bay.length; i++) {
                rows.push({head: i === 0 ? "Through the bay" : "",
                           left: bay[i].place,
                           right: bay[i].way === "slack" ? "slack"
                               : bay[i].knots.toFixed(1) + " kn " + bay[i].way,
                           note: "", urgent: bay[i].way === "ebb"});
            }
            return rows;
        }

        delegate: Column {
            required property var modelData
            width: list.width
            spacing: 2
            Item { width: 1; height: modelData.head ? 8 : 0 }
            Text {
                visible: !!parent.parent.modelData.head
                width: list.width
                text: parent.parent.modelData.head
                color: root.ink
                opacity: root.faint
                font.family: Style.font.family
                font.pixelSize: Style.font.caption
                font.bold: true
            }
            Row {
                width: list.width
                spacing: 8
                Text {
                    width: list.width * 0.42
                    text: parent.parent.modelData.left
                    color: root.ink
                    font.family: Style.font.family
                    font.pixelSize: Style.font.caption
                    elide: Text.ElideRight
                }
                Text {
                    width: list.width * 0.28
                    text: parent.parent.modelData.right
                    color: parent.parent.modelData.urgent ? Color.urgent : root.ink
                    font.family: Style.font.family
                    font.pixelSize: Style.font.caption
                    horizontalAlignment: Text.AlignRight
                }
                Text {
                    text: parent.parent.modelData.note
                    color: root.ink
                    opacity: root.faint
                    font.family: Style.font.family
                    font.pixelSize: Style.font.caption
                }
            }
        }
    }
}
