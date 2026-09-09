pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts

Rectangle {
    id: notice
    required property var updater
    signal detailsRequested
    implicitHeight: 52
    color: Theme.accentWash
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 24
        anchors.rightMargin: 20
        spacing: 12
        Icon {
            name: "import"
            color: Theme.accent
            size: 18
        }
        Text {
            Layout.fillWidth: true
            text: qsTr("留声 v%1 已发布").arg(notice.updater.latestVersion)
            textFormat: Text.PlainText
            color: Theme.text
            font.pixelSize: Theme.labelSize
            elide: Text.ElideRight
        }
        QuietButton {
            objectName: "updateNoticeDetails"
            text: qsTr("查看更新")
            compact: true
            onClicked: notice.detailsRequested()
        }
        QuietButton {
            objectName: "updateNoticeLater"
            glyph: "close"
            hint: qsTr("稍后提醒")
            compact: true
            onClicked: notice.updater.remindLater()
        }
    }
}
