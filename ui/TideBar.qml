import QtQuick
import Quickshell
import qs.Ui
import qs.Commons

// The tide at the boat in the bar, labeled Tide. The height above chart
// datum now, which way it is going, and the next turn are in the tooltip.
// Click for the stream as well, and the bay's narrows.
BarWidget {
    id: root
    moduleName: "org.omahoy.tide"

    // The shape the shell's summon, hide and popout switching expect.
    property bool opened: false
    property bool popoutSwitchClosing: false
    function open() {
        popoutSwitchClosing = false;
        opened = true;
    }
    function close() {
        opened = false;
    }
    function closeForPopoutSwitch() {
        popoutSwitchClosing = true;
        close();
    }

    readonly property var water: Tide.water
    readonly property var nextTurn: {
        const turns = Tide.turnsOf(root.water, "heightM");
        return turns.length ? turns[0] : null;
    }
    readonly property string label: "Tide"
    // A stream worth noticing before you leave the dock.
    readonly property bool strong: !!Tide.stream && Tide.stream.knots >= 2.0
    readonly property string detail: {
        if (Tide.incompatible) return "omatide speaks another protocol version";
        if (!Tide.connected || !Tide.state) return Tide.waited ? "omatide isn't running" : "Starting omatide…";
        if (!Tide.hasCatalog) return "No stations yet: run `omatide fetch`";
        if (!root.water) return "No tide yet";
        const height = root.water.heightM.toFixed(1) + (root.water.rising ? "↑" : "↓");
        const summary = root.nextTurn
            ? height + " " + (root.nextTurn.turn === "high" ? "HW" : "LW") + " " + Tide.clock(root.nextTurn.time)
            : height;
        if (root.strong) return summary + " — The stream is running 2 knots or more";
        return summary;
    }

    implicitWidth: button.implicitWidth
    implicitHeight: button.implicitHeight

    WidgetButton {
        id: button
        anchors.fill: parent
        bar: root.bar
        text: root.label
        foreground: root.strong ? Color.urgent : (root.bar ? root.bar.barForeground : Color.foreground)
        dimmed: !Tide.connected || !root.water
        tooltipText: root.detail
        onPressed: b => {
            if (b === Qt.LeftButton) {
                if (root.opened) root.close();
                else root.open();
            }
        }
    }

    KeyboardPanel {
        id: popup
        anchorItem: button
        bar: root.bar
        owner: root
        open: root.opened
        padding: 12
        borderSpec: Border.flat(root.strong ? Color.urgent : Color.accent, 2)
        // Fixed size: binding to the loaded list makes the popover jump as
        // it settles.
        contentWidth: 400
        contentHeight: 460
        focusTarget: content.item
        Loader {
            id: content
            anchors.fill: parent
            active: root.opened
            sourceComponent: Summary {
                onCloseRequested: root.close()
            }
        }
    }
}
