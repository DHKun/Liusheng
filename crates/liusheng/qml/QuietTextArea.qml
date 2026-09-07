pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic

TextArea {
    id: field
    padding: 12
    color: Theme.text
    placeholderTextColor: Theme.muted
    selectionColor: Theme.accentWash
    selectedTextColor: Theme.text
    font.family: Theme.fontFamily
    font.pixelSize: Theme.captionSize
    selectByMouse: true
    wrapMode: Text.Wrap
    background: Rectangle {
        radius: Theme.radius
        color: Theme.background
        border.color: field.activeFocus ? Theme.accent : Theme.line
        border.width: field.activeFocus ? 2 : 1
    }
}
