pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

QuietDialog {
    id: dialog
    property var reports: []
    property string detailText: ""
    objectName: "onlineSourceInfo"
    title: qsTr("来源与隐私")
    anchors.centerIn: parent
    width: Math.min(540, parent ? parent.width - 32 : 540)
    height: Math.min(460, parent ? parent.height - 32 : 460)
    showCancel: false
    acceptText: qsTr("关闭")
    contentItem: ScrollView {
        id: scroll
        clip: true
        contentWidth: availableWidth
        Column {
            width: scroll.availableWidth
            spacing: 16
            Text {
                width: parent.width
                text: qsTr("歌词：LRCLIB、QQ 音乐、网易云音乐。\n封面：MusicBrainz / Cover Art Archive、QQ 音乐、网易云音乐、Deezer。\n\n检索会向所选来源发送歌名、歌手和专辑信息，结合时长核对版本。音频、本地路径和播放记录保留在本机。\n\n选定资料保存在留声资料库，原音频保持原样。自动补全使用扩展来源需在设置中开启。资料版权归权利人，使用范围遵循来源条款。")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: Theme.secondary
                font.pixelSize: Theme.labelSize
            }
            Repeater {
                model: dialog.reports
                Text {
                    required property var modelData
                    width: parent.width
                    text: modelData.provider + " · " + (modelData.status === "error" ? modelData.message : qsTr("%1 个结果").arg(modelData.count))
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: modelData.status === "error" ? Theme.danger : Theme.text
                    font.pixelSize: Theme.captionSize
                }
            }
            Text {
                width: parent.width
                visible: text.length > 0
                text: dialog.detailText
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
            }
        }
        ScrollBar.vertical: QuietScrollBar {}
    }
}
