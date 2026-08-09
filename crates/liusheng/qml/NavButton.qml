import QtQuick
import QtQuick.Controls

Button {
    id: control

    property color accentColor: "#d9a15f"
    property color foregroundColor: "#e8edf0"
    property color hoverColor: "#1e292f"
    property bool selected: false

    implicitWidth: 176
    implicitHeight: 42
    leftPadding: 16
    rightPadding: 16
    focusPolicy: Qt.StrongFocus

    contentItem: Text {
        text: control.text
        color: control.selected ? control.accentColor : control.foregroundColor
        font.family: "Noto Sans CJK SC"
        font.pixelSize: 14
        font.weight: control.selected ? Font.DemiBold : Font.Medium
        verticalAlignment: Text.AlignVCenter
    }

    background: Rectangle {
        color: control.selected
               ? Qt.rgba(control.accentColor.r, control.accentColor.g, control.accentColor.b, 0.12)
               : control.hovered ? control.hoverColor : "transparent"
        radius: 10
        border.width: control.activeFocus ? 1 : 0
        border.color: control.accentColor

        Rectangle {
            visible: control.selected
            width: 3
            height: 18
            radius: 2
            color: control.accentColor
            anchors.left: parent.left
            anchors.leftMargin: 5
            anchors.verticalCenter: parent.verticalCenter
        }

        Behavior on color {
            ColorAnimation { duration: 180; easing.type: Easing.OutCubic }
        }
    }
}
