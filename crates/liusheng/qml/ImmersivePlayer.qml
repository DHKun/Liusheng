pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: page
    required property var controller
    signal closeRequested
    color: Theme.background
    readonly property bool split: width >= 940
    property bool lyricsOnly: false
    property alias optionsMenu: options
    focus: visible
    Keys.onEscapePressed: closeRequested()
    Rectangle {
        anchors.fill: parent
        color: page.controller.currentAccent || Theme.accent
        opacity: Theme.dark ? 0.035 : 0.025
    }
    RowLayout {
        id: top
        x: 24
        y: 20
        width: parent.width - 48
        height: 40
        QuietButton {
            glyph: "down"
            hint: qsTr("返回曲库 · Esc")
            onClicked: page.closeRequested()
        }
        Text {
            text: qsTr("正在播放")
            color: Theme.secondary
            font.pixelSize: Theme.captionSize
            Layout.fillWidth: true
        }
        QuietButton {
            visible: !page.split
            text: page.lyricsOnly ? qsTr("封面") : qsTr("歌词")
            onClicked: page.lyricsOnly = !page.lyricsOnly
        }
        QuietButton {
            id: lyricOptions
            glyph: "more"
            hint: qsTr("歌词选项")
            onClicked: options.openBelow(lyricOptions)
            QuietMenu {
                id: options
                objectName: "lyricsMenu"
                QuietMenuItem {
                    text: qsTr("歌词提前 0.1 秒")
                    enabled: page.controller.hasCurrentTrack
                    onTriggered: page.controller.requestLyricsOffset(page.controller.lyricsOffsetMs - 100)
                }
                QuietMenuItem {
                    text: qsTr("歌词延后 0.1 秒")
                    enabled: page.controller.hasCurrentTrack
                    onTriggered: page.controller.requestLyricsOffset(page.controller.lyricsOffsetMs + 100)
                }
                QuietMenuItem {
                    text: qsTr("重置歌词偏移")
                    enabled: page.controller.lyricsOffsetMs !== 0
                    onTriggered: page.controller.requestLyricsOffset(0)
                }
            }
        }
    }
    RowLayout {
        id: splitLayout
        anchors.top: top.bottom
        anchors.topMargin: 24
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 40
        anchors.left: parent.left
        anchors.leftMargin: Math.max(48, page.width * 0.065)
        anchors.right: parent.right
        anchors.rightMargin: Math.max(48, page.width * 0.065)
        spacing: Math.max(48, page.width * 0.06)
        ColumnLayout {
            objectName: "nowPlayingCoverPane"
            visible: page.split || !page.lyricsOnly
            Layout.fillWidth: true
            Layout.minimumWidth: 280
            Layout.preferredWidth: page.split ? (splitLayout.width - splitLayout.spacing) * 0.46 : splitLayout.width
            Layout.maximumWidth: page.split ? (splitLayout.width - splitLayout.spacing) * 0.46 : splitLayout.width
            Layout.fillHeight: true
            spacing: 12
            Item {
                Layout.fillHeight: true
            }
            CoverArt {
                Layout.alignment: Qt.AlignHCenter
                Layout.preferredWidth: Math.min(380, (page.height - 175) * 0.8, parent.width)
                Layout.preferredHeight: width
                source: page.controller.currentCoverUrl
                title: page.controller.currentTitle
                resolution: 768
            }
            Text {
                textFormat: Text.PlainText
                text: page.controller.currentTitle || qsTr("此刻，听音乐")
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: 22
                font.weight: Font.DemiBold
                Layout.fillWidth: true
                Layout.topMargin: 12
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
            }
            Text {
                textFormat: Text.PlainText
                text: page.controller.currentArtist || qsTr("从曲库选择一首喜欢的歌")
                color: Theme.secondary
                font.pixelSize: Theme.labelSize
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
            }
            Item {
                Layout.fillHeight: true
            }
        }
        Item {
            objectName: "nowPlayingLyricsPane"
            Layout.minimumWidth: page.split ? 300 : 0
            Layout.preferredWidth: page.split ? (splitLayout.width - splitLayout.spacing) * 0.54 : splitLayout.width
            visible: page.split || page.lyricsOnly
            Layout.fillWidth: true
            Layout.fillHeight: true
            ListView {
                id: lyrics
                objectName: "lyricsList"
                anchors.fill: parent
                visible: page.controller.lyricLineCount > 0
                clip: true
                spacing: 22
                model: page.controller.lyricLineCount
                currentIndex: page.controller.currentLyricIndex
                boundsBehavior: Flickable.StopAtBounds
                highlightRangeMode: ListView.ApplyRange
                preferredHighlightBegin: height * 0.38
                preferredHighlightEnd: height * 0.55
                highlightMoveDuration: Theme.slow
                onCurrentIndexChanged: {
                    if (currentIndex >= 0 && !moving && !manualFollowPause.running)
                        positionViewAtIndex(currentIndex, ListView.Center);
                }
                onMovementStarted: manualFollowPause.restart()
                header: Item {
                    height: lyrics.height * 0.28
                }
                footer: Item {
                    height: lyrics.height * 0.38
                }
                ScrollBar.vertical: QuietScrollBar {}
                delegate: ItemDelegate {
                    id: line
                    required property int index
                    readonly property int timestamp: {
                        page.controller.lyricsRevision;
                        return page.controller.lyricTimeMs(index);
                    }
                    readonly property bool current: index === page.controller.currentLyricIndex
                    width: lyrics.width - 8
                    implicitHeight: label.implicitHeight + 12
                    padding: 6
                    focusPolicy: Qt.StrongFocus
                    enabled: timestamp >= 0 && page.controller.seekable
                    background: Rectangle {
                        radius: Theme.radius
                        color: line.hovered ? Theme.subtle : "transparent"
                        border.width: line.visualFocus ? 1 : 0
                        border.color: Theme.accent
                    }
                    contentItem: Text {
                        id: label
                        objectName: "lyricText"
                        text: {
                            page.controller.lyricsRevision;
                            return page.controller.lyricText(line.index);
                        }
                        textFormat: Text.PlainText
                        color: line.current ? Theme.text : Theme.secondary
                        font.family: Theme.fontFamily
                        font.pixelSize: page.split ? 24 : 22
                        font.weight: Font.DemiBold
                        wrapMode: Text.Wrap
                    }
                    onClicked: page.controller.seekTo(Math.max(0, timestamp + page.controller.lyricsOffsetMs))
                }
            }
            Timer {
                id: manualFollowPause
                interval: 5000
                onTriggered: {
                    if (lyrics.currentIndex >= 0)
                        lyrics.positionViewAtIndex(lyrics.currentIndex, ListView.Center);
                }
            }
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(320, parent.width)
                spacing: 12
                visible: page.controller.lyricLineCount === 0
                Icon {
                    name: "lyrics"
                    size: 28
                    color: Theme.muted
                    Layout.alignment: Qt.AlignHCenter
                }
                Text {
                    text: page.controller.lyricsLoading ? qsTr("正在读取歌词") : qsTr("让音乐自己说话")
                    color: Theme.text
                    font.pixelSize: Theme.headingSize
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                }
                Text {
                    text: page.controller.lyricsError || qsTr("同名 LRC 与内嵌歌词会在这里显示。")
                    color: Theme.secondary
                    font.pixelSize: Theme.captionSize
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                }
            }
            Text {
                anchors.bottom: parent.bottom
                text: qsTr("歌词偏移 %1 秒").arg((page.controller.lyricsOffsetMs / 1000).toFixed(1))
                visible: page.controller.lyricsOffsetMs !== 0
                color: Theme.muted
                font.pixelSize: Theme.noteSize
            }
        }
    }
}
