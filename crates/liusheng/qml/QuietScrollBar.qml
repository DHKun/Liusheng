pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic

ScrollBar {
    id: bar
    implicitWidth: 8
    policy: ScrollBar.AsNeeded
    padding: 2
    minimumSize: 0.08
    contentItem: Rectangle {
        visible: bar.size < 0.999
        implicitWidth: 4
        implicitHeight: 40
        radius: 2
        color: Theme.muted
        opacity: bar.pressed ? 0.8 : bar.active || bar.hovered ? 0.55 : 0.25
        Behavior on opacity {
            NumberAnimation {
                duration: Theme.medium
            }
        }
    }
}
