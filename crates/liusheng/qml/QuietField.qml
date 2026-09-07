pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic

TextField {
    id: field
    property bool search: false
    implicitHeight: 38
    leftPadding: search ? 36 : 12
    rightPadding: search && text.length ? 34 : 12
    color: Theme.text
    placeholderTextColor: Theme.muted
    selectionColor: Theme.accentWash
    selectedTextColor: Theme.text
    font.family: Theme.fontFamily
    font.pixelSize: Theme.labelSize
    selectByMouse: true
    Accessible.name: placeholderText
    background: Rectangle {
        radius: Theme.radius
        color: field.search ? Theme.subtle : Theme.surface
        border.width: field.activeFocus ? 2 : field.search ? 0 : 1
        border.color: field.activeFocus ? Theme.accent : Theme.line
    }
    Icon {
        name: "search"
        size: 16
        color: Theme.muted
        visible: field.search
        x: 12
        anchors.verticalCenter: parent.verticalCenter
    }
    QuietButton {
        visible: field.search && field.text.length > 0
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        compact: true
        glyph: "close"
        hint: qsTr("清除搜索")
        onClicked: {
            field.clear();
            field.forceActiveFocus();
        }
    }
}
