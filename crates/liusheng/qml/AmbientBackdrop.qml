pragma ComponentBehavior: Bound
import QtQuick

Rectangle {
    id: backdrop
    required property var colors
    property bool animate: false
    property bool previousDark: colors.dark
    readonly property bool animating: drift.running
    property real midpoint: 0.46
    property color topColor: colors.background
    property color bottomColor: colors.edge
    color: topColor
    gradient: Gradient {
        GradientStop {
            position: 0
            color: backdrop.topColor
        }
        GradientStop {
            position: backdrop.midpoint
            color: backdrop.topColor
        }
        GradientStop {
            position: 1
            color: backdrop.bottomColor
        }
    }
    // Geometry-only gradients work on Metal, Wayland and software rendering.
    Behavior on topColor {
        enabled: backdrop.previousDark === backdrop.colors.dark && !Theme.reducedMotion
        ColorAnimation {
            duration: 650
        }
    }
    Behavior on bottomColor {
        enabled: backdrop.previousDark === backdrop.colors.dark && !Theme.reducedMotion
        ColorAnimation {
            duration: 650
        }
    }
    Component.onCompleted: previousDark = colors.dark
    Connections {
        target: backdrop.colors
        function onDarkChanged() {
            Qt.callLater(function () {
                backdrop.previousDark = backdrop.colors.dark;
            });
        }
    }
    Timer {
        id: drift
        interval: 100
        repeat: true
        property real phase: 0
        running: backdrop.animate && !Theme.reducedMotion && backdrop.visible
        onTriggered: {
            phase = (phase + Math.PI / 160) % (2 * Math.PI);
            backdrop.midpoint = 0.5 + 0.15 * Math.sin(phase);
        }
    }
}
