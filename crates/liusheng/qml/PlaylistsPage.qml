pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs

Item {
    id: page
    required property var controller
    property color foregroundColor: "#e8edf0"
    property color mutedColor: "#829198"
    property color accentColor: "#d9a15f"
    property int selectedIndex: -1
    FileDialog {
        id: importFile
        title: qsTr("导入歌单")
        nameFilters: [qsTr("M3U8 歌单 (*.m3u8 *.m3u)")]
        onAccepted: page.controller.importPlaylist(selectedFile.toString())
    }
    FileDialog {
        id: exportFile
        title: qsTr("导出当前播放队列")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "m3u8"
        nameFilters: [qsTr("M3U8 歌单 (*.m3u8)")]
        onAccepted: page.controller.exportQueue(selectedFile.toString())
    }
    Dialog {
        id: nameDialog
        property bool renaming: false
        title: renaming ? qsTr("重命名歌单") : qsTr("保存当前队列为歌单")
        anchors.centerIn: parent
        modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel
        onOpened: { playlistName.text = ""; playlistName.forceActiveFocus() }
        onAccepted: {
            if (renaming) page.controller.renamePlaylist(page.selectedIndex, playlistName.text)
            else page.controller.saveQueuePlaylist(playlistName.text)
        }
        TextField { id: playlistName; width: 300; placeholderText: qsTr("歌单名称"); selectByMouse: true; onAccepted: nameDialog.accept() }
    }
    Dialog {
        id: deleteDialog
        title: qsTr("删除选中的歌单？")
        anchors.centerIn: parent
        modal: true
        standardButtons: Dialog.Yes | Dialog.No
        onAccepted: page.controller.deletePlaylist(page.selectedIndex)
        Label { text: qsTr("此操作删除歌单记录，音频文件保留。") }
    }
    ColumnLayout {
        anchors.fill: parent
        spacing: 18
        Flow {
            Layout.fillWidth: true
            Layout.preferredHeight: childrenRect.height
            spacing: 8
            Button { text: qsTr("保存当前队列"); enabled: page.controller.queueCount > 0; onClicked: { nameDialog.renaming = false; nameDialog.open() } }
            Button { text: qsTr("导入 M3U8…"); onClicked: importFile.open() }
            Button { text: qsTr("导出当前队列…"); enabled: page.controller.queueCount > 0; onClicked: exportFile.open() }
        }
        Label { text: qsTr("歌单独立保存，目录离线时仍可浏览。"); color: page.mutedColor }
        ListView {
            id: list
            Layout.fillWidth: true; Layout.fillHeight: true
            clip: true; spacing: 8
            model: page.controller.playlistCount
            reuseItems: true
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: row
                required property int index
                width: list.width; height: 64; radius: 12
                color: Qt.rgba(page.foregroundColor.r, page.foregroundColor.g, page.foregroundColor.b, 0.05)
                RowLayout {
                    anchors.fill: parent; anchors.margins: 12; spacing: 10
                    Label {
                        Layout.fillWidth: true
                        text: { page.controller.playlistRevision; return page.controller.playlistName(row.index) }
                        color: page.foregroundColor; elide: Text.ElideRight
                    }
                    Button { text: qsTr("播放"); onClicked: page.controller.playPlaylist(row.index) }
                    Button { text: qsTr("改名"); onClicked: { page.selectedIndex = row.index; nameDialog.renaming = true; nameDialog.open() } }
                    Button { text: qsTr("删除"); onClicked: { page.selectedIndex = row.index; deleteDialog.open() } }
                }
            }
            Label { anchors.centerIn: parent; visible: list.count === 0; text: qsTr("将当前队列保存为你的第一份歌单"); color: page.mutedColor }
        }
    }
}
