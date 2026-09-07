pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import io.github.dhkun.Liusheng 1.0

Drawer {
    id: panel
    required property var controller
    signal saveRequested
    width: Math.min(400, parent ? parent.width - 40 : 400)
    popupType: Popup.Item
    edge: Qt.RightEdge
    modal: false
    padding: 24
    focus: true
    dragMargin: 0
    background: Rectangle {
        color: Theme.surface
        Rectangle {
            width: 1
            height: parent.height
            color: Theme.line
        }
    }
    enter: Transition {
        NumberAnimation {
            property: "position"
            to: 1
            duration: Theme.slow
            easing.type: Easing.OutCubic
        }
    }
    exit: Transition {
        NumberAnimation {
            property: "position"
            to: 0
            duration: Theme.slow
            easing.type: Easing.OutCubic
        }
    }
    UiModel {
        id: queueModel
        kind: "queue"
        Component.onCompleted: refresh()
    }
    Connections {
        target: panel.controller
        function onQueueRevisionChanged() {
            queueModel.refresh();
        }
    }
    contentItem: ColumnLayout {
        spacing: 16
        RowLayout {
            Layout.fillWidth: true
            Text {
                text: qsTr("播放队列")
                color: Theme.text
                font.pixelSize: 19
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }
            QuietButton {
                glyph: "close"
                compact: true
                hint: qsTr("关闭队列")
                onClicked: panel.close()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Text {
                text: qsTr("%1 首歌曲").arg(panel.controller.queueCount)
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
                Layout.fillWidth: true
            }
            QuietButton {
                id: queueMore
                glyph: "more"
                compact: true
                hint: qsTr("队列操作")
                onClicked: queueActions.openBelow(queueMore)
                QuietMenu {
                    id: queueActions
                    y: parent.height + 4
                    x: parent.width - width
                    QuietMenuItem {
                        text: qsTr("保存为歌单…")
                        enabled: panel.controller.queueCount > 0
                        onTriggered: panel.saveRequested()
                    }
                    QuietMenuItem {
                        text: qsTr("清空队列…")
                        destructive: true
                        enabled: panel.controller.queueCount > 0
                        onTriggered: clearDialog.open()
                    }
                }
            }
        }
        TrackTable {
            controller: panel.controller
            trackModel: queueModel
            mode: "queue"
            condensed: true
            Layout.fillWidth: true
            Layout.fillHeight: true
        }
        Text {
            visible: panel.controller.queueCount === 0
            text: qsTr("选择音乐，即可开始播放。\n歌曲的更多菜单可加入队列。")
            color: Theme.secondary
            wrapMode: Text.Wrap
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
            Layout.bottomMargin: 40
            font.pixelSize: Theme.captionSize
        }
    }
    QuietDialog {
        id: clearDialog
        parent: Overlay.overlay
        anchors.centerIn: parent
        title: qsTr("清空播放队列？")
        acceptText: qsTr("清空队列")
        contentItem: Text {
            text: qsTr("当前播放会停止，已保存的歌单将保留。")
            color: Theme.secondary
            font.pixelSize: Theme.labelSize
            wrapMode: Text.Wrap
        }
        onAccepted: panel.controller.clearQueue()
    }
}
