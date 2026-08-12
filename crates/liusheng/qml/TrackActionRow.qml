import QtQuick
import QtQuick.Controls

TrackListRow {
    id: row

    property color surfaceColor: "#151d22"
    property bool queueActionsAvailable: false
    signal enqueueRequested(bool playNext)

    rightInset: actionButton.visible ? 72 : 0

    Button {
        id: actionButton

        width: 54
        height: 30
        anchors.right: parent.right
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        visible: row.queueActionsAvailable
        enabled: visible && row.interactive
        text: qsTr("操作")
        focusPolicy: Qt.StrongFocus
        Accessible.name: qsTr("安排 %1").arg(row.trackTitle)
        onClicked: {
            actionMenu.open()
            actionMenu.forceActiveFocus()
        }

        contentItem: Text {
            text: actionButton.text
            color: actionButton.hovered || actionButton.activeFocus
                   ? row.accentColor
                   : row.mutedColor
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 11
            font.weight: Font.Medium
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }

        background: Rectangle {
            color: actionButton.hovered
                   ? Qt.rgba(row.accentColor.r, row.accentColor.g, row.accentColor.b, 0.1)
                   : "transparent"
            radius: 15
            border.width: actionButton.activeFocus ? 1 : 0
            border.color: row.accentColor
        }

        Menu {
            id: actionMenu

            x: actionButton.width - width
            y: actionButton.height + 4
            width: 136
            padding: 6
            onOpened: {
                const firstItem = itemAt(0)
                if (firstItem)
                    firstItem.forceActiveFocus()
            }

            background: Rectangle {
                color: row.surfaceColor
                radius: 10
                border.width: 1
                border.color: Qt.rgba(row.foregroundColor.r,
                                      row.foregroundColor.g,
                                      row.foregroundColor.b,
                                      0.12)
            }

            MenuItem {
                text: qsTr("下一首播放")
                Accessible.name: qsTr("将 %1 设为下一首").arg(row.trackTitle)
                onTriggered: row.enqueueRequested(true)
            }

            MenuItem {
                text: qsTr("加入队列")
                Accessible.name: qsTr("将 %1 加入队列").arg(row.trackTitle)
                onTriggered: row.enqueueRequested(false)
            }
        }
    }
}
