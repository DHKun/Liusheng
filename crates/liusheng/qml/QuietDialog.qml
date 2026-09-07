pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog
    property string acceptText: qsTr("保存")
    property bool acceptEnabled: true
    property bool showCancel: true
    property bool showAccept: true
    popupType: Popup.Item
    parent: Overlay.overlay
    margins: 16
    closePolicy: Popup.CloseOnEscape
    modal: true
    width: Math.min(480, parent ? parent.width - 40 : 480)
    // Header/footer have deliberate 64px hit areas; use those actual heights,
    // rather than RowLayout's smaller implicit size, when reserving content.
    implicitHeight: implicitContentHeight + topPadding + bottomPadding + (header && header.visible ? header.height + spacing : 0) + (footer && footer.visible ? footer.height + spacing : 0)
    padding: 24
    topPadding: 16
    bottomPadding: 16
    focus: true
    background: QuietSurface {}
    Overlay.modal: Rectangle {
        color: Theme.dark ? "#80000000" : "#40242522"
    }
    header: RowLayout {
        height: 64
        Text {
            text: dialog.title
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: Theme.headingSize
            font.weight: Font.DemiBold
            Layout.fillWidth: true
            Layout.leftMargin: 24
            elide: Text.ElideRight
        }
        QuietButton {
            glyph: "close"
            hint: qsTr("关闭")
            Layout.rightMargin: 16
            onClicked: dialog.reject()
        }
    }
    footer: RowLayout {
        height: 64
        Item {
            Layout.fillWidth: true
        }
        QuietButton {
            visible: dialog.showCancel
            text: qsTr("取消")
            onClicked: dialog.reject()
        }
        QuietButton {
            visible: dialog.showAccept
            text: dialog.acceptText
            primary: true
            enabled: dialog.acceptEnabled
            Layout.rightMargin: 24
            onClicked: dialog.accept()
        }
    }
    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0
            to: 1
            duration: Theme.fast
        }
    }
    exit: Transition {
        NumberAnimation {
            property: "opacity"
            to: 0
            duration: Theme.fast
        }
    }
}
