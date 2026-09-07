pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs

Item {
    id: page
    required property var controller
    property alias nameView: nameDialog
    property alias deleteView: deleteDialog
    property alias nameField: playlistName
    property alias importView: importFile
    property alias exportView: exportFile
    property alias moreMenu: fileMenu
    property int selectedIndex: -1
    function saveQueue() {
        nameDialog.renaming = false;
        nameDialog.open();
    }
    FileDialog {
        id: importFile
        parentWindow: page.Window.window
        title: qsTr("导入歌单")
        nameFilters: [qsTr("M3U8 歌单 (*.m3u8 *.m3u)")]
        onAccepted: page.controller.importPlaylist(selectedFile.toString())
    }
    FileDialog {
        id: exportFile
        parentWindow: page.Window.window
        title: qsTr("导出当前播放队列")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "m3u8"
        nameFilters: [qsTr("M3U8 歌单 (*.m3u8)")]
        onAccepted: page.controller.exportQueue(selectedFile.toString())
    }
    QuietDialog {
        id: nameDialog
        parent: Overlay.overlay
        anchors.centerIn: parent
        property bool renaming: false
        title: renaming ? qsTr("重命名歌单") : qsTr("保存播放队列")
        acceptEnabled: playlistName.text.trim().length > 0
        onOpened: {
            playlistName.text = "";
            playlistName.forceActiveFocus();
        }
        onAccepted: {
            if (renaming)
                page.controller.renamePlaylist(page.selectedIndex, playlistName.text);
            else
                page.controller.saveQueuePlaylist(playlistName.text);
        }
        contentItem: QuietField {
            id: playlistName
            placeholderText: qsTr("歌单名称")
            onAccepted: {
                if (nameDialog.acceptEnabled)
                    nameDialog.accept();
            }
        }
    }
    QuietDialog {
        id: deleteDialog
        parent: Overlay.overlay
        anchors.centerIn: parent
        title: qsTr("删除这份歌单？")
        acceptText: qsTr("删除歌单")
        onAccepted: page.controller.deletePlaylist(page.selectedIndex)
        contentItem: Text {
            text: qsTr("这份歌单的记录会被移除，音乐文件将保留。")
            color: Theme.secondary
            font.pixelSize: Theme.labelSize
            wrapMode: Text.Wrap
        }
    }
    RowLayout {
        id: header
        width: parent.width
        height: 64
        spacing: 10
        Column {
            Layout.fillWidth: true
            spacing: 5
            Text {
                text: qsTr("歌单")
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: Theme.titleSize
                font.weight: Font.DemiBold
            }
            Text {
                text: qsTr("%1 份收藏").arg(page.controller.playlistCount)
                color: Theme.secondary
                font.pixelSize: Theme.noteSize
            }
        }
        QuietButton {
            text: qsTr("保存队列")
            glyph: "plus"
            primary: true
            enabled: page.controller.queueCount > 0
            onClicked: page.saveQueue()
        }
        QuietButton {
            id: playlistMore
            glyph: "more"
            hint: qsTr("歌单操作")
            onClicked: fileMenu.openBelow(playlistMore)
            QuietMenu {
                id: fileMenu
                QuietMenuItem {
                    text: qsTr("导入 M3U8…")
                    onTriggered: importFile.open()
                }
                QuietMenuItem {
                    text: qsTr("导出当前队列…")
                    enabled: page.controller.queueCount > 0
                    onTriggered: exportFile.open()
                }
            }
        }
    }
    ListView {
        id: list
        anchors.top: header.bottom
        anchors.topMargin: 26
        anchors.bottom: parent.bottom
        width: parent.width
        clip: true
        spacing: 8
        model: page.controller.playlistCount
        reuseItems: true
        ScrollBar.vertical: QuietScrollBar {}
        delegate: Rectangle {
            id: row
            required property int index
            readonly property string name: {
                page.controller.playlistRevision;
                return page.controller.playlistName(index);
            }
            width: list.width
            height: 78
            radius: 6
            color: hover.hovered ? Theme.subtle : "transparent"
            HoverHandler {
                id: hover
            }
            RowLayout {
                anchors.fill: parent
                anchors.margins: 12
                spacing: 18
                Rectangle {
                    Layout.preferredWidth: 52
                    Layout.preferredHeight: 52
                    color: Theme.subtle
                    Icon {
                        anchors.centerIn: parent
                        name: "playlist"
                        color: Theme.secondary
                        size: 24
                    }
                }
                Text {
                    Layout.fillWidth: true
                    textFormat: Text.PlainText
                    text: row.name
                    color: Theme.text
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.bodySize
                    elide: Text.ElideRight
                }
                QuietButton {
                    glyph: "play"
                    hint: qsTr("播放 %1").arg(row.name)
                    onClicked: page.controller.playPlaylist(row.index)
                }
                QuietButton {
                    id: rowMore
                    glyph: "more"
                    hint: qsTr("歌单更多操作")
                    onClicked: {
                        page.selectedIndex = row.index;
                        actions.openBelow(rowMore);
                    }
                    QuietMenu {
                        id: actions
                        QuietMenuItem {
                            text: qsTr("重命名…")
                            onTriggered: {
                                nameDialog.renaming = true;
                                nameDialog.open();
                            }
                        }
                        QuietMenuItem {
                            text: qsTr("删除歌单…")
                            destructive: true
                            onTriggered: deleteDialog.open()
                        }
                    }
                }
            }
        }
        ColumnLayout {
            anchors.centerIn: parent
            width: Math.min(360, parent.width)
            spacing: 16
            visible: list.count === 0
            Icon {
                name: "playlist"
                size: 34
                color: Theme.muted
                Layout.alignment: Qt.AlignHCenter
            }
            Text {
                text: qsTr("给喜欢的音乐一个位置")
                color: Theme.text
                font.pixelSize: Theme.headingSize
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            Text {
                text: qsTr("将播放队列保存为歌单，或导入已有的 M3U8。")
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            QuietButton {
                text: qsTr("导入歌单")
                glyph: "import"
                primary: true
                Layout.alignment: Qt.AlignHCenter
                onClicked: importFile.open()
            }
        }
    }
}
