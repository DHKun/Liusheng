import QtQuick
import QtQuick.Controls

Button {
    id: control

    property color accentColor: "#d9a15f"
    property color foregroundColor: "#12181b"
    property color focusColor: "#e8edf0"
    property color disabledColor: "#829198"

    focusPolicy: Qt.StrongFocus
    implicitWidth: 104
    implicitHeight: 38
    topPadding: 0
    bottomPadding: 0
    leftPadding: 16
    rightPadding: 16

    contentItem: Text {
        text: control.text
        color: control.foregroundColor
        font.family: "Noto Sans CJK SC"
        font.pixelSize: 12
        font.weight: Font.DemiBold
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }

    background: Rectangle {
        radius: 19
        color: control.enabled
               ? control.hovered ? Qt.lighter(control.accentColor, 1.08) : control.accentColor
               : control.disabledColor
        border.width: control.activeFocus ? 2 : 0
        border.color: control.focusColor

        Behavior on color {
            ColorAnimation { duration: 160; easing.type: Easing.OutCubic }
        }
    }
}
