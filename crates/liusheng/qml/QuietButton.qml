pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Button {
    id: control
    property string glyph: ""
    property bool primary: false
    property bool selected: false
    property bool compact: false
    property bool destructive: false
    property string hint: text
    readonly property color foreground: !enabled ? Theme.muted : primary ? Theme.background : destructive ? Theme.danger : selected ? Theme.accent : Theme.text
    implicitHeight: compact ? Theme.compactButton : Theme.buttonHeight
    implicitWidth: text.length ? implicitContentWidth + 28 : implicitHeight
    Layout.fillWidth: false
    Layout.fillHeight: false
    padding: 8
    spacing: 8
    font.family: Theme.fontFamily
    font.pixelSize: Theme.labelSize
    focusPolicy: Qt.StrongFocus
    hoverEnabled: true
    opacity: enabled ? 1 : 0.45
    Accessible.name: hint
    ToolTip.visible: hovered && hint.length > 0 && text.length === 0
    ToolTip.text: hint
    ToolTip.delay: 650
    ToolTip.toolTip.popupType: Popup.Item
    contentItem: RowLayout {
        spacing: control.spacing
        Icon {
            name: control.glyph
            color: control.foreground
            size: 18
            visible: control.glyph.length > 0
            Layout.alignment: Qt.AlignHCenter
        }
        Text {
            text: control.text
            visible: text.length > 0
            color: control.foreground
            font: control.font
            elide: Text.ElideRight
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
        }
    }
    background: Rectangle {
        radius: Theme.radius
        color: control.primary ? Theme.text : control.down ? Theme.line : control.selected ? Theme.accentWash : control.hovered ? Theme.subtle : "transparent"
        border.width: control.visualFocus ? 2 : 0
        border.color: Theme.accent
        Behavior on color {
            ColorAnimation {
                duration: Theme.fast
            }
        }
    }
}
