pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic

ComboBox {
    id: combo
    implicitHeight: 38
    implicitWidth: 170
    leftPadding: 12
    rightPadding: 32
    font.family: Theme.fontFamily
    font.pixelSize: Theme.labelSize
    contentItem: Text {
        text: combo.displayText
        color: Theme.text
        font: combo.font
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle {
        radius: Theme.radius
        color: Theme.surface
        border.color: combo.visualFocus ? Theme.accent : Theme.line
        border.width: combo.visualFocus ? 2 : 1
    }
    indicator: Icon {
        name: "down"
        size: 16
        color: Theme.secondary
        x: combo.width - width - 10
        anchors.verticalCenter: parent.verticalCenter
    }
    delegate: ItemDelegate {
        id: option
        required property int index
        required property var model
        width: combo.width
        height: 38
        highlighted: combo.highlightedIndex === index
        contentItem: Text {
            text: combo.textAt(option.index)
            color: Theme.text
            font: combo.font
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            color: option.highlighted ? Theme.subtle : Theme.surface
        }
        padding: 12
    }
    popup: Popup {
        popupType: Popup.Item
        margins: 12
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutsideParent
        y: combo.height + 4
        width: combo.width
        padding: 4
        implicitHeight: Math.min(280, contentItem.implicitHeight + 8, combo.Window.height - 24)
        background: Rectangle {
            radius: Theme.radius
            color: Theme.surface
            border.color: Theme.line
        }
        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: combo.popup.visible ? combo.delegateModel : null
            currentIndex: combo.highlightedIndex
            ScrollBar.vertical: QuietScrollBar {}
        }
    }
}
