pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: bar
    required property var controller
    property real positionMs: 0
    property bool queueOpen: false
    property bool immersiveOpen: false
    signal queueRequested
    signal immersiveRequested
    signal outputRequested
    implicitHeight: 96
    color: Theme.surface
    Rectangle {
        width: parent.width
        height: 1
        color: Theme.line
    }
    Item {
        anchors.fill: parent
        anchors.leftMargin: bar.width < 960 ? 16 : 24
        anchors.rightMargin: 24
        Item {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: Math.min(320, bar.width * 0.26)
            CoverArt {
                id: miniCover
                width: 52
                height: 52
                anchors.verticalCenter: parent.verticalCenter
                source: bar.controller.currentCoverUrl
                title: bar.controller.currentTitle
            }
            Column {
                anchors.left: miniCover.right
                anchors.leftMargin: 12
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 5
                Text {
                    textFormat: Text.PlainText
                    text: bar.controller.hasCurrentTrack ? bar.controller.currentTitle : qsTr("留声")
                    width: parent.width
                    elide: Text.ElideRight
                    color: Theme.text
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.bodySize
                    font.weight: Font.Medium
                }
                Text {
                    textFormat: Text.PlainText
                    text: bar.controller.hasCurrentTrack ? bar.controller.currentArtist : qsTr("从曲库选择一首音乐")
                    width: parent.width
                    elide: Text.ElideRight
                    color: Theme.secondary
                    font.pixelSize: Theme.captionSize
                }
            }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: bar.immersiveRequested()
            }
            Accessible.role: Accessible.Button
            Accessible.name: qsTr("展开正在播放")
            Accessible.onPressAction: bar.immersiveRequested()
        }
        ColumnLayout {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(460, bar.width * 0.41)
            spacing: 0
            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: bar.width < 960 ? 8 : 16
                QuietButton {
                    glyph: "shuffle"
                    compact: true
                    selected: bar.controller.shuffleEnabled
                    hint: bar.controller.shuffleEnabled ? qsTr("关闭随机播放") : qsTr("随机播放")
                    onClicked: bar.controller.requestPlaybackMode(bar.controller.repeatMode, !bar.controller.shuffleEnabled)
                }
                QuietButton {
                    glyph: "previous"
                    hint: qsTr("上一首")
                    compact: true
                    enabled: bar.controller.hasCurrentTrack && !bar.controller.playbackInitializing
                    onClicked: bar.controller.previousTrack()
                }
                QuietButton {
                    objectName: "playPauseButton"
                    glyph: bar.controller.playing ? "pause" : "play"
                    hint: bar.controller.playing ? qsTr("暂停 · Space") : qsTr("播放 · Space")
                    primary: true
                    implicitWidth: 38
                    implicitHeight: 38
                    enabled: bar.controller.hasCurrentTrack && !bar.controller.playbackInitializing
                    onClicked: bar.controller.togglePlayback()
                    BusyIndicator {
                        anchors.centerIn: parent
                        width: 28
                        height: 28
                        running: bar.controller.playbackInitializing
                        visible: running
                    }
                }
                QuietButton {
                    glyph: "next"
                    hint: qsTr("下一首")
                    compact: true
                    enabled: bar.controller.hasCurrentTrack && !bar.controller.playbackInitializing
                    onClicked: bar.controller.nextTrack()
                }
                QuietButton {
                    glyph: bar.controller.repeatMode === 1 ? "repeat-one" : "repeat"
                    compact: true
                    selected: bar.controller.repeatMode > 0
                    hint: [qsTr("顺序播放"), qsTr("单曲循环"), qsTr("列表循环")][bar.controller.repeatMode]
                    onClicked: bar.controller.requestPlaybackMode((bar.controller.repeatMode + 1) % 3, bar.controller.shuffleEnabled)
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                Text {
                    text: Theme.time(seek.pressed ? seek.value : bar.positionMs)
                    color: Theme.muted
                    font.pixelSize: Theme.noteSize
                    Layout.preferredWidth: 42
                    horizontalAlignment: Text.AlignRight
                }
                QuietSlider {
                    id: seek
                    objectName: "playbackSeek"
                    Layout.fillWidth: true
                    from: 0
                    to: Math.max(1, bar.controller.currentDurationMs)
                    stepSize: 100
                    enabled: bar.controller.hasCurrentTrack && bar.controller.seekable && !bar.controller.playbackInitializing
                    Accessible.name: qsTr("播放进度")
                    onPressedChanged: {
                        if (!pressed && enabled)
                            bar.controller.seekTo(Math.round(value));
                    }
                    onMoved: {
                        if (!pressed)
                            bar.controller.seekTo(Math.round(value));
                    }
                    Binding {
                        target: seek
                        property: "value"
                        value: bar.positionMs
                        when: !seek.pressed
                        restoreMode: Binding.RestoreBindingOrValue
                    }
                }
                Text {
                    text: Theme.time(bar.controller.currentDurationMs)
                    color: Theme.muted
                    font.pixelSize: Theme.noteSize
                    Layout.preferredWidth: 42
                }
            }
        }
        RowLayout {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: 124
            spacing: 4
            Item {
                Layout.fillWidth: true
            }
            QuietButton {
                objectName: "lyricsButton"
                glyph: "lyrics"
                compact: true
                selected: bar.immersiveOpen
                hint: qsTr("正在播放与歌词 · Ctrl+L")
                onClicked: bar.immersiveRequested()
            }
            QuietButton {
                objectName: "queueButton"
                glyph: "queue"
                compact: true
                selected: bar.queueOpen
                hint: qsTr("播放队列 · Ctrl+J")
                onClicked: bar.queueRequested()
            }
            QuietButton {
                objectName: "outputButton"
                glyph: "output"
                compact: true
                hint: qsTr("声音输出与硬件音量")
                onClicked: bar.outputRequested()
            }
        }
    }
}
