pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: bar

    property color surfaceColor: "#11191d"
    property color foregroundColor: "#e8edf0"
    property color mutedColor: "#829198"
    property color accentColor: "#d9a15f"

    color: surfaceColor
    border.width: 1
    border.color: Qt.rgba(foregroundColor.r, foregroundColor.g, foregroundColor.b, 0.08)

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        spacing: 18

        Rectangle {
            Layout.preferredWidth: 52
            Layout.preferredHeight: 52
            radius: 12
            color: Qt.rgba(bar.accentColor.r, bar.accentColor.g, bar.accentColor.b, 0.14)

            Rectangle {
                width: 20
                height: 20
                radius: 10
                color: "transparent"
                border.width: 1
                border.color: bar.accentColor
                anchors.centerIn: parent
            }
        }

        ColumnLayout {
            Layout.preferredWidth: 250
            spacing: 3

            Text {
                text: qsTr("当前没有播放曲目")
                color: bar.foregroundColor
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 14
                font.weight: Font.Medium
            }
            Text {
                text: qsTr("从曲库选择一首歌")
                color: bar.mutedColor
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 12
            }
        }

        Item { Layout.fillWidth: true }

        RowLayout {
            spacing: 8

            Repeater {
                model: [qsTr("上一曲"), qsTr("播放"), qsTr("下一曲")]

                Button {
                    id: transportButton

                    required property string modelData

                    text: modelData
                    enabled: false
                    implicitWidth: modelData === qsTr("播放") ? 68 : 60
                    implicitHeight: 36
                    opacity: 0.42
                    contentItem: Text {
                        text: transportButton.text
                        color: bar.foregroundColor
                        font.family: "Noto Sans CJK SC"
                        font.pixelSize: 12
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        radius: 18
                        color: transportButton.modelData === qsTr("播放")
                               ? Qt.rgba(bar.accentColor.r, bar.accentColor.g, bar.accentColor.b, 0.2)
                               : "transparent"
                        border.width: 1
                        border.color: Qt.rgba(bar.foregroundColor.r,
                                              bar.foregroundColor.g,
                                              bar.foregroundColor.b,
                                              0.12)
                    }
                }
            }
        }

        Item { Layout.fillWidth: true }

        Text {
            text: "0:00  /  0:00"
            color: bar.mutedColor
            font.family: "JetBrains Mono"
            font.pixelSize: 11
        }
    }
}
