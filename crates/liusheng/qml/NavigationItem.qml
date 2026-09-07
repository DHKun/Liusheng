pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

QuietButton {
    id: nav
    property bool iconOnly: false
    implicitHeight: 40
    leftPadding: 12
    rightPadding: 12
    Layout.fillWidth: true
    contentItem: RowLayout {
        spacing: 12
        Icon {
            objectName: "navigationGlyph"
            name: nav.glyph
            size: 20
            color: nav.selected ? Theme.accent : Theme.secondary
            Layout.alignment: Qt.AlignHCenter
        }
        Text {
            objectName: "navigationLabel"
            visible: !nav.iconOnly
            text: nav.text
            color: nav.selected ? Theme.text : Theme.secondary
            font.family: Theme.fontFamily
            font.pixelSize: Theme.labelSize
            font.weight: nav.selected ? Font.Medium : Font.Normal
            Layout.fillWidth: true
            elide: Text.ElideRight
        }
    }
    ToolTip.visible: hovered && nav.iconOnly
}
