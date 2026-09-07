pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic

CheckBox {
    id: box
    implicitHeight: 38
    spacing: 12
    font.family: Theme.fontFamily
    font.pixelSize: Theme.labelSize
    indicator: Rectangle {
        x: 0
        anchors.verticalCenter: parent.verticalCenter
        width: 18
        height: 18
        radius: 4
        color: box.checked ? Theme.text : Theme.surface
        border.color: box.visualFocus ? Theme.accent : Theme.line
        border.width: box.visualFocus ? 2 : 1
        Icon {
            anchors.centerIn: parent
            size: 13
            name: "check"
            color: Theme.background
            visible: box.checked
        }
    }
    contentItem: Text {
        leftPadding: 30
        text: box.text
        color: Theme.text
        font: box.font
        wrapMode: Text.Wrap
        verticalAlignment: Text.AlignVCenter
    }
}
