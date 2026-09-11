pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs

QuietDialog {
    id: dialog
    required property var service
    readonly property var state: service.state
    property real clockNow: Date.now()
    readonly property int retrySeconds: Math.max(0, Math.ceil(((state.retryAt || 0) - clockNow) / 1000))
    Timer {
        interval: 1000
        repeat: true
        running: dialog.opened && dialog.state.status === "waiting"
        onTriggered: dialog.clockNow = Date.now()
    }
    readonly property var target: state.context || ({})
    readonly property bool cover: state.kind === "cover"
    property string contextToken: ""
    property bool applyPending: false
    property string pendingKey: ""
    signal selectionApplied(string message)
    function beginApply() {
        pendingKey = cover && albumScope.checked && target.albumKey ? target.albumKey : target.trackKey;
        applyPending = true;
    }
    readonly property var candidate: state.selected >= 0 && state.selected < state.candidates.length ? state.candidates[state.selected] : null
    title: cover ? qsTr("查找封面") : qsTr("查找歌词")
    objectName: "onlineAssetsDialog"
    width: Math.min(840, parent ? parent.width - 32 : 840)
    height: Math.min(640, parent ? parent.height - 32 : 640)
    showAccept: false
    showCancel: false
    onClosed: {
        applyPending = false;
        service.close();
    }
    function populate() {
        titleField.text = target.title || "";
        artistField.text = target.artist || target.albumArtist || "";
        albumField.text = target.album || "";
        albumScope.checked = target.scope === "album" && !!target.albumKey;
    }
    onOpened: {
        clockNow = Date.now();
        applyPending = false;
        contextToken = (target.identity || "") + ":" + state.kind + ":" + (target.scope || "track");
        populate();
    }
    Connections {
        target: dialog.service
        function onChanged() {
            const token = (dialog.target.identity || "") + ":" + dialog.state.kind + ":" + (dialog.target.scope || "track");
            if (dialog.state.status === "error" || dialog.state.status === "retry")
                dialog.applyPending = false;
            if (dialog.contextToken !== token) {
                dialog.applyPending = false;
                dialog.contextToken = token;
                dialog.populate();
            }
        }
        function onApplied(identity, key, kind, message) {
            if (dialog.opened && dialog.applyPending && identity === dialog.target.identity && key === dialog.pendingKey && kind === dialog.state.kind) {
                dialog.applyPending = false;
                dialog.selectionApplied(message);
                dialog.close();
            }
        }
    }
    FileDialog {
        id: localFile
        parentWindow: dialog.parent ? dialog.parent.Window.window : null
        title: dialog.cover ? qsTr("选择封面图片") : qsTr("导入歌词")
        fileMode: FileDialog.OpenFile
        nameFilters: dialog.cover ? [qsTr("图片 (*.jpg *.jpeg *.png *.webp)")] : [qsTr("歌词 (*.lrc *.alrc *.ttml *.yrc *.qrc *.txt)")]
        onAccepted: {
            dialog.beginApply();
            dialog.service.importLocal(selectedFile, albumScope.checked);
        }
    }
    QuietDialog {
        id: resetConfirmation
        title: qsTr("恢复本地资料")
        anchors.centerIn: parent
        acceptText: qsTr("恢复默认")
        contentItem: Text {
            text: qsTr("恢复本地资料，并暂停此项自动补全。")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: Theme.text
            font.pixelSize: Theme.labelSize
        }
        onAccepted: dialog.service.restoreDefault(albumScope.checked)
    }
    OnlineSourceInfo {
        id: sourceInfo
        reports: dialog.state.sourceReports || []
        detailText: dialog.state.queryHint || ""
        onClosed: sourceInfoButton.forceActiveFocus()
    }
    contentItem: ColumnLayout {
        spacing: 10
        RowLayout {
            Layout.fillWidth: true
            OnlineSourcePicker {
                id: sourcePicker
                objectName: "onlineSourcePicker"
                kind: dialog.state.kind
                Layout.preferredWidth: 170
                enabled: !dialog.state.busy
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("联网检索歌曲信息")
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
                elide: Text.ElideRight
            }
            QuietButton {
                id: sourceInfoButton
                objectName: "onlineSourceInfoButton"
                text: qsTr("来源与隐私")
                compact: true
                onClicked: sourceInfo.open()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            QuietField {
                id: titleField
                objectName: "onlineTitle"
                Layout.fillWidth: true
                placeholderText: qsTr("歌曲名")
                Accessible.name: placeholderText
                maximumLength: 512
                onAccepted: searchButton.clicked()
            }
            QuietField {
                id: artistField
                objectName: "onlineArtist"
                Layout.fillWidth: true
                placeholderText: qsTr("艺术家")
                Accessible.name: placeholderText
                maximumLength: 512
                onAccepted: searchButton.clicked()
            }
            QuietField {
                id: albumField
                objectName: "onlineAlbum"
                Layout.fillWidth: true
                placeholderText: qsTr("专辑，可选")
                Accessible.name: placeholderText
                maximumLength: 512
                onAccepted: searchButton.clicked()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            QuietButton {
                id: searchButton
                objectName: "onlineSearch"
                text: qsTr("查找")
                glyph: "search"
                compact: true
                primary: true
                enabled: !dialog.state.busy && !!dialog.target.trackKey && (titleField.text.trim().length > 0 || (dialog.cover && albumField.text.trim().length > 0))
                onClicked: {
                    if (enabled)
                        dialog.service.searchWithSource(titleField.text, artistField.text, albumField.text, sourcePicker.sourceId);
                }
            }
            QuietButton {
                objectName: "onlineCancelRequest"
                text: qsTr("取消请求")
                compact: true
                visible: dialog.state.busy && !dialog.applyPending
                onClicked: dialog.service.cancel()
            }
            QuietButton {
                text: qsTr("导入本地…")
                compact: true
                enabled: !dialog.state.busy && !!dialog.target.trackKey
                onClicked: localFile.open()
            }
            Item {
                Layout.fillWidth: true
            }
            BusyIndicator {
                implicitWidth: 24
                implicitHeight: 24
                running: dialog.state.busy
                visible: running
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 100
            spacing: 16
            ListView {
                id: results
                objectName: "onlineCandidates"
                Layout.preferredWidth: Math.min(300, dialog.width * 0.4)
                Layout.fillHeight: true
                clip: true
                spacing: 4
                model: dialog.state.candidates
                enabled: !dialog.applyPending && !dialog.state.busy
                currentIndex: dialog.state.selected
                keyNavigationEnabled: true
                Keys.onReturnPressed: {
                    if (currentIndex >= 0)
                        dialog.service.preview(currentIndex);
                }
                delegate: ItemDelegate {
                    id: result
                    required property var modelData
                    required property int index
                    width: results.width - 10
                    height: 70
                    padding: 10
                    onClicked: {
                        results.forceActiveFocus();
                        dialog.service.preview(index);
                    }
                    Accessible.name: modelData.title + ", " + modelData.artist
                    background: Rectangle {
                        radius: 5
                        color: dialog.state.selected === result.index ? Theme.accentWash : result.hovered ? Theme.subtle : "transparent"
                        border.color: Theme.accent
                        border.width: result.activeFocus ? 1 : 0
                    }
                    contentItem: Column {
                        spacing: 4
                        Text {
                            width: parent.width
                            text: result.modelData.title
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: Theme.text
                            font.pixelSize: Theme.labelSize
                            font.weight: Font.Medium
                        }
                        Text {
                            width: parent.width
                            text: result.modelData.artist + (dialog.cover ? (result.modelData.date ? " · " + result.modelData.date : "") : result.modelData.lyricsPending ? "" : result.modelData.synced ? qsTr(" · 同步") : result.modelData.instrumental ? qsTr(" · 纯音乐") : qsTr(" · 文本"))
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: Theme.secondary
                            font.pixelSize: Theme.captionSize
                        }
                        Text {
                            width: parent.width
                            text: (result.modelData.provider || "") + (result.modelData.album ? " · " + result.modelData.album : "")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: Theme.muted
                            font.pixelSize: 10
                        }
                    }
                }
                ScrollBar.vertical: QuietScrollBar {}
                Text {
                    anchors.centerIn: parent
                    width: parent.width - 20
                    visible: results.count === 0
                    objectName: "onlineEmptyState"
                    text: dialog.state.busy ? qsTr("查找中…") : dialog.state.status === "missing" ? qsTr("暂无结果，可换来源或导入本地") : dialog.state.status === "error" || dialog.state.status === "retry" ? qsTr("暂时无法查询") : qsTr("输入歌名与歌手后查找")
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    color: Theme.muted
                    font.pixelSize: Theme.captionSize
                }
            }
            Rectangle {
                Layout.fillHeight: true
                width: 1
                color: Theme.line
            }
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 8
                Image {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: dialog.cover
                    source: dialog.state.previewUrl || ""
                    fillMode: Image.PreserveAspectFit
                    sourceSize.width: 768
                    sourceSize.height: 768
                    asynchronous: true
                }
                ScrollView {
                    id: textPreview
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: !dialog.cover
                    clip: true
                    contentWidth: availableWidth
                    TextArea {
                        width: textPreview.availableWidth
                        text: dialog.state.previewText || qsTr("选择结果预览")
                        textFormat: TextEdit.PlainText
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.Wrap
                        color: Theme.text
                        font.pixelSize: Theme.labelSize
                        background: null
                    }
                    ScrollBar.vertical: QuietScrollBar {}
                }
                Text {
                    Layout.fillWidth: true
                    text: dialog.candidate && dialog.candidate.delta >= 0 ? qsTr("时长差 %1 秒").arg(dialog.candidate.delta.toFixed(1)) : ""
                    visible: text.length > 0
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: Theme.secondary
                    font.pixelSize: Theme.captionSize
                }
                Text {
                    objectName: "onlineCandidateNote"
                    Layout.fillWidth: true
                    text: dialog.candidate ? dialog.candidate.note || "" : ""
                    visible: text.length > 0
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    maximumLineCount: 3
                    elide: Text.ElideRight
                    color: Theme.secondary
                    font.pixelSize: Theme.captionSize
                }
                QuietButton {
                    text: dialog.candidate ? dialog.candidate.provider || qsTr("来源页面") : qsTr("来源页面")
                    compact: true
                    visible: dialog.candidate !== null
                    onClicked: dialog.service.openSource()
                }
            }
        }
        QuietCheckBox {
            id: albumScope
            objectName: "onlineAlbumScope"
            text: qsTr("应用到同目录的同专辑歌曲")
            enabled: !dialog.applyPending
            visible: dialog.cover && !!dialog.target.albumKey
        }
        Text {
            objectName: "onlineMessage"
            Layout.fillWidth: true
            text: dialog.state.status === "waiting" ? qsTr("%1 秒后自动重试，可取消请求").arg(dialog.retrySeconds) : dialog.state.status === "candidates" ? (dialog.state.partialResults ? qsTr("部分来源暂不可用，已保留可用结果") : "") : dialog.state.message || ""
            visible: text.length > 0 && (["idle", "preview", "candidates"].indexOf(dialog.state.status) < 0 || (dialog.state.status === "candidates" && !!dialog.state.partialResults))
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
            color: dialog.state.status === "error" ? Theme.danger : Theme.secondary
            font.pixelSize: Theme.captionSize
        }
    }
    footer: RowLayout {
        height: 64
        QuietButton {
            text: qsTr("恢复默认…")
            compact: true
            Layout.leftMargin: 24
            enabled: !!dialog.target.trackKey && !dialog.state.busy
            onClicked: resetConfirmation.open()
        }
        Item {
            Layout.fillWidth: true
        }
        QuietButton {
            text: qsTr("关闭")
            onClicked: dialog.close()
        }
        QuietButton {
            objectName: "onlineApply"
            text: dialog.applyPending ? qsTr("正在应用…") : qsTr("应用并关闭")
            primary: true
            Layout.rightMargin: 24
            enabled: !dialog.applyPending && !dialog.state.busy && dialog.candidate !== null && (dialog.cover ? !!dialog.state.previewUrl : !dialog.candidate.lyricsPending)
            onClicked: {
                dialog.beginApply();
                dialog.service.choose(albumScope.checked);
            }
        }
    }
}
