pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

QuietDialog {
    id: dialog
    required property var service
    required property var controller
    readonly property var state: service.state
    readonly property var counts: state.batchCounts || ({})
    property string requestedSource: "all"
    property real clockNow: Date.now()
    readonly property int retrySeconds: Math.max(0, Math.ceil(((state.batchRetryAt || 0) - clockNow) / 1000))
    readonly property string phaseText: ({
            idle: qsTr("准备就绪"),
            running: qsTr("正在查找"),
            waiting: qsTr("等待来源恢复 · %1 秒后自动重试").arg(retrySeconds),
            paused: qsTr("已暂停 · 剩余任务已保留"),
            manual: qsTr("等待手动查找结束"),
            completed: qsTr("处理完成"),
            cancelled: qsTr("已取消 · 可重新查询取消项")
        })[state.batchPhase || "idle"] || ""
    Timer {
        interval: 1000
        repeat: true
        running: dialog.opened && dialog.state.batchPhase === "waiting"
        onTriggered: dialog.clockNow = Date.now()
    }
    function outcome(status) {
        return ({
                saved: qsTr("已补全"),
                kept: qsTr("已保留"),
                review: qsTr("待确认"),
                missing: qsTr("来源暂无结果"),
                "cover-missing": qsTr("发行记录缺图"),
                error: qsTr("请求失败"),
                retry: qsTr("等待重试"),
                cancelled: qsTr("已取消")
            })[status] || status;
    }
    signal reviewRequested(string context, string kind)
    objectName: "onlineBatchDialog"
    title: qsTr("补全缺失资料")
    width: Math.min(740, parent ? parent.width - 32 : 740)
    height: Math.min(570, parent ? parent.height - 32 : 570)
    showAccept: false
    showCancel: false
    onOpened: {
        clockNow = Date.now();
        service.inspectBatch();
    }
    OnlineSourceInfo {
        id: sourceInfo
        detailText: dialog.state.batchMessage || ""
        reports: dialog.state.batchSourceReports || []
    }
    contentItem: ColumnLayout {
        spacing: 12
        RowLayout {
            Layout.fillWidth: true
            OnlineSourcePicker {
                id: sourcePicker
                objectName: "batchSourcePicker"
                kind: resource.currentIndex === 0 ? "lyrics" : "cover"
                Layout.preferredWidth: 170
                enabled: !dialog.controller.onlinePreparing && dialog.state.batchRemaining === 0
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("联网补全缺失项，保留已有资料")
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
                elide: Text.ElideRight
            }
            QuietButton {
                objectName: "batchSourceInfoButton"
                text: qsTr("来源与隐私")
                compact: true
                onClicked: sourceInfo.open()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            QuietComboBox {
                id: resource
                objectName: "batchResourceKind"
                Layout.fillWidth: true
                model: [qsTr("缺失的歌词"), qsTr("缺失的封面")]
                enabled: !dialog.controller.onlinePreparing && dialog.state.batchRemaining === 0
            }
            QuietCheckBox {
                id: selectedOnly
                text: qsTr("当前详情页")
                enabled: dialog.controller.selectedTrackCount > 0
                checked: enabled
            }
            QuietButton {
                objectName: "batchStart"
                text: qsTr("开始")
                primary: true
                enabled: !dialog.controller.onlinePreparing && dialog.state.batchRemaining === 0
                onClicked: {
                    dialog.requestedSource = sourcePicker.sourceId;
                    dialog.controller.prepareOnlineBatch(resource.currentIndex === 0 ? "lyrics" : "cover", selectedOnly.enabled && selectedOnly.checked);
                }
            }
        }
        Text {
            objectName: "batchProgressSummary"
            Layout.fillWidth: true
            text: dialog.controller.onlinePreparing ? qsTr("正在核对本地封面与歌词…") : qsTr("已处理 %1 / %2 · 剩余 %3%4").arg(dialog.state.batchDone).arg(dialog.state.batchTotal).arg(dialog.state.batchRemaining).arg(dialog.state.batchCancelled > 0 ? qsTr(" · 已取消 %1").arg(dialog.state.batchCancelled) : "")
            color: Theme.text
            font.pixelSize: Theme.labelSize
            wrapMode: Text.Wrap
        }
        Text {
            objectName: "batchOutcomeSummary"
            Layout.fillWidth: true
            visible: !!dialog.state.batchKind && dialog.state.batchDone > 0
            text: (dialog.state.batchKind === "cover" ? qsTr("封面") : qsTr("歌词")) + qsTr(" · 已补全 %1 · 待确认 %2 · 暂无资料 %3 · 请求失败 %4").arg(dialog.counts.saved || 0).arg(dialog.counts.review || 0).arg((dialog.counts.missing || 0) + (dialog.counts["cover-missing"] || 0)).arg(dialog.counts.error || 0)
            wrapMode: Text.Wrap
            color: Theme.secondary
            font.pixelSize: Theme.captionSize
        }
        Text {
            objectName: "batchStatusText"
            Layout.fillWidth: true
            text: dialog.phaseText
            visible: !!dialog.state.batchPhase && dialog.state.batchPhase !== "idle"
            textFormat: Text.PlainText
            color: Theme.secondary
            font.pixelSize: Theme.captionSize
            wrapMode: Text.Wrap
        }
        ProgressBar {
            id: batchProgress
            Layout.fillWidth: true
            implicitHeight: 4
            background: Rectangle {
                color: Theme.line
                radius: 2
            }
            contentItem: Item {
                Rectangle {
                    width: batchProgress.visualPosition * parent.width
                    height: parent.height
                    radius: 2
                    color: Theme.accent
                }
            }
            from: 0
            to: Math.max(1, dialog.state.batchTotal)
            value: dialog.state.batchDone
            indeterminate: dialog.controller.onlinePreparing
        }
        RowLayout {
            QuietButton {
                objectName: "batchPauseResume"
                text: dialog.state.batchPaused ? qsTr("继续") : qsTr("暂停")
                compact: true
                enabled: dialog.state.batchRemaining > 0 && !dialog.controller.onlinePreparing
                onClicked: dialog.state.batchPaused ? dialog.service.resumeBatch() : dialog.service.pauseBatch()
            }
            QuietButton {
                objectName: "batchCancel"
                text: qsTr("取消")
                compact: true
                enabled: dialog.controller.onlinePreparing || dialog.state.batchRemaining > 0
                onClicked: {
                    dialog.controller.cancelOnlinePreparation();
                    dialog.service.cancelBatch();
                }
            }
            QuietButton {
                objectName: "batchRetryFailed"
                text: qsTr("重试未完成项")
                compact: true
                visible: dialog.state.batchRetryable > 0
                enabled: dialog.state.batchPaused && !dialog.controller.onlinePreparing
                onClicked: dialog.service.retryBatchFailures()
            }
            Item {
                Layout.fillWidth: true
            }
        }
        ListView {
            id: rows
            objectName: "batchResultList"
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 100
            clip: true
            model: dialog.state.batchResults
            spacing: 4
            delegate: ItemDelegate {
                id: row
                required property var modelData
                width: rows.width - 12
                height: 60
                enabled: modelData.status !== "saved" && modelData.status !== "kept"
                onClicked: dialog.reviewRequested(JSON.stringify(modelData.context), modelData.kind)
                background: Rectangle {
                    radius: 5
                    color: row.hovered ? Theme.subtle : "transparent"
                }
                contentItem: Column {
                    spacing: 6
                    Text {
                        width: parent.width
                        text: row.modelData.title + " · " + row.modelData.artist
                        textFormat: Text.PlainText
                        color: Theme.text
                        font.pixelSize: Theme.labelSize
                        elide: Text.ElideRight
                    }
                    Text {
                        width: parent.width
                        text: dialog.outcome(row.modelData.status)
                        ToolTip.visible: row.hovered && !!row.modelData.message
                        ToolTip.text: row.modelData.message || ""
                        textFormat: Text.PlainText
                        color: row.modelData.status === "error" ? Theme.danger : Theme.secondary
                        font.pixelSize: Theme.captionSize
                        elide: Text.ElideRight
                    }
                }
            }
            ScrollBar.vertical: QuietScrollBar {}
        }
    }
    footer: RowLayout {
        height: 64
        QuietButton {
            text: qsTr("清理搜索缓存")
            compact: true
            Layout.leftMargin: 24
            enabled: !dialog.state.busy && dialog.state.batchPaused
            onClicked: dialog.service.clearSearchCache()
        }
        Item {
            Layout.fillWidth: true
        }
        QuietButton {
            objectName: "onlineBatchClose"
            text: dialog.controller.onlinePreparing || (dialog.state.batchRemaining > 0 && !dialog.state.batchPaused) ? qsTr("关闭，任务继续") : qsTr("关闭")
            Layout.rightMargin: 24
            onClicked: dialog.close()
        }
    }
}
