pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic

Slider {
    id: slider
    implicitHeight: 28
    implicitWidth: 160
    leftPadding: 6
    rightPadding: 6
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus
    background: Rectangle {
        x: slider.leftPadding
        y: (slider.height - height) / 2
        width: slider.availableWidth
        height: 3
        radius: 1.5
        color: Theme.line
        Rectangle {
            width: slider.visualPosition * parent.width
            height: parent.height
            radius: 1.5
            color: slider.enabled ? Theme.text : Theme.muted
        }
    }
    handle: Rectangle {
        x: slider.leftPadding + slider.visualPosition * (slider.availableWidth - width)
        y: (slider.height - height) / 2
        width: 10
        height: 10
        radius: 5
        color: Theme.text
        opacity: slider.enabled && (slider.hovered || slider.pressed || slider.visualFocus) ? 1 : 0
        border.width: slider.visualFocus ? 2 : 0
        border.color: Theme.accent
        Behavior on opacity {
            NumberAnimation {
                duration: Theme.fast
            }
        }
    }
}
