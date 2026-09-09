pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

QuietDialog {
    id: dialog
    required property var updater
    title: qsTr("软件更新")
    width: Math.min(600, parent ? parent.width - 40 : 600)
    height: Math.min(updater.status === "available" ? 590 : 350, parent ? parent.height - 40 : 590)
    showAccept: false
    showCancel: false
    onRejected: {
        if (updater.status === "available")
            updater.remindLater();
    }
    onOpened: packages.currentIndex = updater.recommendedIndex
    readonly property var selectedAsset: packages.currentIndex >= 0 && packages.currentIndex < updater.assets.length ? updater.assets[packages.currentIndex] : null
    property string selectionKey: ""
    Connections {
        target: dialog.updater
        function onChanged() {
            const key = dialog.updater.latestVersion + ":" + dialog.updater.assets.length;
            if (dialog.selectionKey !== key) {
                dialog.selectionKey = key;
                packages.currentIndex = dialog.updater.recommendedIndex;
            }
        }
    }
    contentItem: ColumnLayout {
        spacing: 16
        RowLayout {
            Layout.fillWidth: true
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 6
                Text {
                    objectName: "updateStatusText"
                    text: dialog.updater.message
                    textFormat: Text.PlainText
                    color: Theme.text
                    font.pixelSize: Theme.headingSize
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                Text {
                    text: qsTr("当前版本 v%1").arg(dialog.updater.currentVersion)
                    color: Theme.secondary
                    font.pixelSize: Theme.captionSize
                }
            }
            BusyIndicator {
                implicitWidth: 28
                implicitHeight: 28
                running: dialog.updater.busy
                visible: running
                Accessible.name: qsTr("正在检查更新")
            }
        }
        Text {
            visible: dialog.updater.status === "available" && dialog.updater.ignored
            text: qsTr("你已跳过这个版本，仍可在这里手动下载。")
            color: Theme.secondary
            font.pixelSize: Theme.captionSize
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        RowLayout {
            Layout.fillWidth: true
            QuietButton {
                objectName: "updateCheckAgain"
                glyph: "refresh"
                text: qsTr("重新检查")
                compact: true
                enabled: !dialog.updater.busy
                onClicked: dialog.updater.check(true)
            }
            QuietButton {
                objectName: "updateReleasePage"
                text: qsTr("GitHub 发布页")
                compact: true
                onClicked: dialog.updater.openReleasePage()
            }
            Item {
                Layout.fillWidth: true
            }
        }
        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: Theme.line
        }
        ScrollView {
            id: notesScroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: availableWidth
            ColumnLayout {
                width: notesScroll.availableWidth
                spacing: 12
                Text {
                    visible: dialog.updater.status === "available" && dialog.updater.publishedAt.length > 0
                    text: qsTr("发布时间：%1").arg(dialog.updater.publishedAt.slice(0, 10))
                    color: Theme.secondary
                    font.pixelSize: Theme.captionSize
                }
                Text {
                    objectName: "updateReleaseNotes"
                    Layout.fillWidth: true
                    text: dialog.updater.status === "available" ? (dialog.updater.notes || qsTr("本次发布暂未填写更新说明，可在 GitHub 查看详情。")) : qsTr("检查 GitHub 上的最新正式 Release。下载由浏览器处理，安装时由你确认。")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: Theme.text
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.labelSize
                    lineHeight: 1.35
                }
            }
            ScrollBar.vertical: QuietScrollBar {}
        }
        ColumnLayout {
            visible: dialog.updater.status === "available"
            Layout.fillWidth: true
            spacing: 8
            QuietComboBox {
                id: packages
                objectName: "updatePackageChoice"
                visible: dialog.updater.assets.length > 0
                Layout.fillWidth: true
                model: dialog.updater.assets
                textRole: "label"
                currentIndex: dialog.updater.recommendedIndex
                Accessible.name: qsTr("选择更新安装包")
            }
            Text {
                Layout.fillWidth: true
                text: dialog.selectedAsset ? qsTr("%1 · %2 MiB\n%3
配置与曲库保留。下载校验值见发布页 SHA256SUMS。").arg(dialog.selectedAsset.name).arg((dialog.selectedAsset.size / 1048576).toFixed(1)).arg(dialog.updater.installHint) : qsTr("该 Release 的当前平台安装包尚未齐全，请到发布页查看。")
                textFormat: Text.PlainText
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
                wrapMode: Text.WrapAnywhere
            }
        }
        Text {
            Layout.fillWidth: true
            visible: dialog.updater.actionError.length > 0
            text: dialog.updater.actionError
            textFormat: Text.PlainText
            color: Theme.danger
            font.pixelSize: Theme.captionSize
            wrapMode: Text.Wrap
        }
    }
    footer: RowLayout {
        height: 64
        spacing: 8
        QuietButton {
            objectName: "skipUpdateVersion"
            Layout.leftMargin: 24
            text: qsTr("跳过此版本")
            visible: dialog.updater.status === "available" && !dialog.updater.ignored
            onClicked: {
                if (dialog.updater.skipVersion())
                    dialog.close();
            }
        }
        Item {
            Layout.fillWidth: true
        }
        QuietButton {
            objectName: "updateLaterButton"
            text: dialog.updater.status === "available" ? qsTr("稍后提醒") : qsTr("关闭")
            onClicked: dialog.reject()
        }
        QuietButton {
            objectName: "updateDownloadButton"
            text: qsTr("下载更新")
            primary: true
            visible: dialog.updater.status === "available" && dialog.selectedAsset !== null
            enabled: !dialog.updater.busy
            onClicked: dialog.updater.openAsset(packages.currentIndex)
        }
        Item {
            Layout.preferredWidth: 16
        }
    }
}
