pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic

MenuItem {
    id: item
    property bool destructive: false
    implicitHeight: 36
    font.family: Theme.fontFamily
    font.pixelSize: Theme.labelSize
    leftPadding: checkable ? 30 : 12
    rightPadding: 12
    opacity: enabled ? 1 : 0.4
    contentItem: Text {
        text: item.text
        color: item.destructive ? Theme.danger : Theme.text
        font: item.font
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
    }
    indicator: Icon {
        x: 8
        anchors.verticalCenter: parent.verticalCenter
        size: 16
        name: "check"
        color: Theme.accent
        visible: item.checked
    }
    background: Rectangle {
        radius: 5
        color: item.highlighted ? Theme.subtle : "transparent"
        border.width: item.visualFocus ? 1 : 0
        border.color: Theme.accent
    }
}
