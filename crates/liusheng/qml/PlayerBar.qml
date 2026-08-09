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
    property string trackTitle
    property string trackArtist
    property string errorText
    property int positionMs
    property int durationMs
    property bool hasTrack: false
    property bool playing: false
    property bool busy: false

    signal previousRequested
    signal toggleRequested
    signal nextRequested

    function timeText(milliseconds) {
        const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000))
        const minutes = Math.floor(totalSeconds / 60)
        const seconds = totalSeconds % 60
        return minutes + ":" + (seconds < 10 ? "0" : "") + seconds
    }

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
                Layout.fillWidth: true
                text: bar.hasTrack ? bar.trackTitle : qsTr("当前没有播放曲目")
                color: bar.foregroundColor
                elide: Text.ElideRight
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 14
                font.weight: Font.Medium
            }
            Text {
                Layout.fillWidth: true
                text: bar.errorText.length > 0
                      ? bar.errorText
                      : bar.hasTrack ? bar.trackArtist : qsTr("从曲库选择一首歌")
                color: bar.errorText.length > 0 ? bar.accentColor : bar.mutedColor
                elide: Text.ElideRight
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 12
            }
        }

        Item { Layout.fillWidth: true }

        RowLayout {
            spacing: 8

            Repeater {
                model: 3

                Button {
                    id: transportButton

                    required property int index

                    text: index === 0
                          ? qsTr("上一曲")
                          : index === 1
                            ? bar.busy ? qsTr("连接中") : bar.playing ? qsTr("暂停") : qsTr("播放")
                            : qsTr("下一曲")
                    enabled: bar.hasTrack && !bar.busy
                    implicitWidth: index === 1 ? 68 : 60
                    implicitHeight: 36
                    opacity: enabled ? 1 : 0.42
                    onClicked: {
                        if (index === 0)
                            bar.previousRequested()
                        else if (index === 1)
                            bar.toggleRequested()
                        else
                            bar.nextRequested()
                    }
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
                        color: transportButton.index === 1
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
            text: qsTr("%1  /  %2")
                  .arg(bar.timeText(bar.positionMs))
                  .arg(bar.timeText(bar.durationMs))
            color: bar.mutedColor
            font.family: "JetBrains Mono"
            font.pixelSize: 11
        }
    }
}
