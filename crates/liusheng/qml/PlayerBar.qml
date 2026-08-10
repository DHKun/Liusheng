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
    property bool seekable: false
    property bool playing: false
    property bool busy: false
    property bool volumeAvailable: false
    property bool hardwareMuted: false
    property bool hardwareMuteAvailable: false
    property int volumePercent: 100
    property string volumeErrorText

    signal previousRequested
    signal toggleRequested
    signal nextRequested
    signal seekRequested(real positionMs)
    signal volumeRequested(int percent)
    signal muteRequested
    signal volumeRefreshRequested

    function timeText(milliseconds) {
        const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000))
        const minutes = Math.floor(totalSeconds / 60)
        const seconds = totalSeconds % 60
        return minutes + ":" + (seconds < 10 ? "0" : "") + seconds
    }

    color: surfaceColor
    border.width: 1
    border.color: Qt.rgba(foregroundColor.r, foregroundColor.g, foregroundColor.b, 0.08)

    Slider {
        id: seekSlider

        property real pendingSeekMs: bar.positionMs

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        height: 20
        from: 0
        to: Math.max(1, bar.durationMs)
        stepSize: 1000
        enabled: bar.hasTrack && bar.seekable && !bar.busy && bar.durationMs > 0
        hoverEnabled: true
        Accessible.name: qsTr("播放进度")
        onPressedChanged: {
            if (pressed) {
                pendingSeekMs = value
            } else if (enabled) {
                bar.seekRequested(Math.round(pendingSeekMs))
            }
        }
        onMoved: {
            pendingSeekMs = value
            if (!pressed)
                bar.seekRequested(Math.round(value))
        }

        Binding {
            target: seekSlider
            property: "value"
            value: bar.positionMs
            when: !seekSlider.pressed
            restoreMode: Binding.RestoreBindingOrValue
        }

        background: Rectangle {
            x: seekSlider.leftPadding
            y: seekSlider.topPadding + seekSlider.availableHeight / 2 - height / 2
            width: seekSlider.availableWidth
            height: 3
            radius: 2
            color: Qt.rgba(bar.foregroundColor.r,
                           bar.foregroundColor.g,
                           bar.foregroundColor.b,
                           0.11)

            Rectangle {
                width: seekSlider.visualPosition * parent.width
                height: parent.height
                radius: parent.radius
                color: bar.accentColor
            }
        }

        handle: Rectangle {
            x: seekSlider.leftPadding
               + seekSlider.visualPosition * (seekSlider.availableWidth - width)
            y: seekSlider.topPadding + seekSlider.availableHeight / 2 - height / 2
            width: seekSlider.pressed || seekSlider.hovered ? 12 : 8
            height: width
            radius: width / 2
            color: bar.accentColor
            border.width: seekSlider.activeFocus ? 2 : 0
            border.color: bar.foregroundColor
            opacity: seekSlider.enabled ? 1 : 0

            Behavior on width {
                NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
            }
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        anchors.topMargin: 8
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
                  .arg(bar.timeText(seekSlider.pressed
                                    ? seekSlider.pendingSeekMs
                                    : bar.positionMs))
                  .arg(bar.timeText(bar.durationMs))
            color: bar.mutedColor
            font.family: "JetBrains Mono"
            font.pixelSize: 11
        }

        HardwareVolumeControl {
            Layout.preferredWidth: 202
            Layout.preferredHeight: 52
            available: bar.volumeAvailable
            muted: bar.hardwareMuted
            canMute: bar.hardwareMuteAvailable
            percent: bar.volumePercent
            errorText: bar.volumeErrorText
            foregroundColor: bar.foregroundColor
            mutedColor: bar.mutedColor
            accentColor: bar.accentColor
            onVolumeRequested: percent => bar.volumeRequested(percent)
            onMuteRequested: bar.muteRequested()
            onRefreshRequested: bar.volumeRefreshRequested()
        }
    }
}
