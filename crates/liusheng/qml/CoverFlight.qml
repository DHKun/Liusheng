pragma ComponentBehavior: Bound
import QtQuick

// A single in-window image carries the cover between the compact player and
// listening view. No native popup, texture readback or duplicate model is used.
Item {
    id: flight
    property url source
    property string title: ""
    readonly property bool running: movement.running
    property real targetX: 0
    property real targetY: 0
    property real targetSize: 0
    property bool allowed: true
    visible: running
    z: 35
    function cancel() {
        movement.stop();
    }
    function fly(fromItem, toItem) {
        const previous = running ? {
            x: x,
            y: y,
            size: width
        } : null;
        cancel();
        if (!allowed || Theme.reducedMotion || !source.toString().length || !fromItem || !toItem || fromItem.width <= 0 || toItem.width <= 0)
            return;
        const from = fromItem.mapToItem(parent, 0, 0), to = toItem.mapToItem(parent, 0, 0);
        x = previous ? previous.x : from.x;
        y = previous ? previous.y : from.y;
        width = previous ? previous.size : fromItem.width;
        height = width;
        targetX = to.x;
        targetY = to.y;
        targetSize = toItem.width;
        movement.start();
    }
    onSourceChanged: cancel()
    onAllowedChanged: {
        if (!allowed)
            cancel();
    }
    Connections {
        target: Theme
        function onReducedMotionChanged() {
            if (Theme.reducedMotion)
                flight.cancel();
        }
    }
    CoverArt {
        anchors.fill: parent
        source: flight.source
        title: flight.title
        resolution: 768
    }
    ParallelAnimation {
        id: movement
        NumberAnimation {
            target: flight
            property: "x"
            to: flight.targetX
            duration: Theme.spatial
            easing.type: Easing.InOutCubic
        }
        NumberAnimation {
            target: flight
            property: "y"
            to: flight.targetY
            duration: Theme.spatial
            easing.type: Easing.InOutCubic
        }
        NumberAnimation {
            target: flight
            property: "width"
            to: flight.targetSize
            duration: Theme.spatial
            easing.type: Easing.InOutCubic
        }
        NumberAnimation {
            target: flight
            property: "height"
            to: flight.targetSize
            duration: Theme.spatial
            easing.type: Easing.InOutCubic
        }
    }
}
