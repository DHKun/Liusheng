pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

FocusScope {
    id: page
    required property var controller
    property real positionMs: controller.positionMs || 0
    property bool displayed: visible
    property bool windowActive: true
    property bool coverHidden: false
    property string layoutMode: "split"
    property bool lyricsOnly: false
    readonly property bool split: width >= 940 && layoutMode === "split" && !lyricsOnly
    readonly property bool showCover: layoutMode !== "lyrics" && !lyricsOnly
    readonly property bool showLyrics: split || layoutMode === "lyrics" || lyricsOnly
    property alias optionsMenu: options
    property alias layoutMenu: layouts
    property alias coverItem: cover
    property alias lyricView: lyricView
    property alias listeningColors: listeningPalette
    property alias ambientRunning: backdrop.animating
    signal closeRequested
    focus: visible
    Keys.onEscapePressed: closeRequested()

    ListeningPalette {
        id: listeningPalette
        seed: page.controller.currentCoverUrl ? (page.controller.currentAccent || "#6f9d99") : "#6f9d99"
        tinted: Theme.coverTheme && !!page.controller.currentCoverUrl
    }
    AmbientBackdrop {
        id: backdrop
        anchors.fill: parent
        colors: listeningPalette
        animate: page.displayed && page.windowActive && page.controller.playing && Theme.ambientMotion
    }
    RowLayout {
        id: top
        x: page.width >= 1000 ? 32 : 20
        y: 20
        width: parent.width - x * 2
        height: 40
        spacing: 12
        QuietButton {
            objectName: "closeListening"
            glyph: "down"
            hint: qsTr("返回曲库 · Esc")
            onClicked: page.closeRequested()
        }
        Text {
            text: qsTr("正在播放")
            color: listeningPalette.secondary
            font.pixelSize: Theme.captionSize
            font.letterSpacing: 2
            Layout.fillWidth: true
        }
        QuietButton {
            id: layoutButton
            objectName: "listeningLayoutButton"
            text: page.layoutMode === "cover" ? qsTr("纯封面") : page.layoutMode === "lyrics" || page.lyricsOnly ? qsTr("聚焦歌词") : qsTr("封面与歌词")
            glyph: "lyrics"
            hint: qsTr("播放页布局")
            onClicked: layouts.openBelow(layoutButton)
            QuietMenu {
                id: layouts
                objectName: "listeningLayoutMenu"
                QuietMenuItem {
                    text: qsTr("封面与歌词")
                    checkable: true
                    checked: page.layoutMode === "split" && !page.lyricsOnly
                    onTriggered: {
                        page.layoutMode = "split";
                        page.lyricsOnly = false;
                    }
                }
                QuietMenuItem {
                    text: qsTr("聚焦歌词")
                    checkable: true
                    checked: page.layoutMode === "lyrics" || page.lyricsOnly
                    onTriggered: {
                        page.layoutMode = "lyrics";
                        page.lyricsOnly = false;
                    }
                }
                QuietMenuItem {
                    text: qsTr("纯封面")
                    checkable: true
                    checked: page.layoutMode === "cover" && !page.lyricsOnly
                    onTriggered: {
                        page.layoutMode = "cover";
                        page.lyricsOnly = false;
                    }
                }
            }
        }
        QuietButton {
            visible: page.width < 940 && page.layoutMode === "split"
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
                    objectName: "findOnlineLyrics"
                    text: qsTr("查找歌词…")
                    enabled: page.controller.hasCurrentTrack
                    onTriggered: page.controller.requestOnlineDetails("current", 0, "lyrics")
                }
                QuietMenuItem {
                    objectName: "findOnlineCover"
                    text: qsTr("查找封面…")
                    enabled: page.controller.hasCurrentTrack
                    onTriggered: page.controller.requestOnlineDetails("current", 0, "cover")
                }
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
                QuietMenuItem {
                    text: qsTr("回到当前句")
                    enabled: page.controller.lyricLineCount > 0
                    onTriggered: lyricView.resumeFollow()
                }
            }
        }
    }
    Item {
        id: stage
        anchors.top: top.bottom
        anchors.topMargin: page.height < 540 ? 16 : 32
        anchors.bottom: parent.bottom
        anchors.bottomMargin: page.height < 540 ? 28 : 46
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Math.max(40, page.width * 0.07)
        anchors.rightMargin: Math.max(40, page.width * 0.07)
        readonly property real gap: page.split ? Math.max(56, page.width * 0.07) : 0
        Item {
            id: coverPane
            objectName: "nowPlayingCoverPane"
            visible: page.showCover
            width: page.split ? (stage.width - stage.gap) * 0.46 : stage.width
            height: parent.height
            Column {
                width: Math.min(420, coverPane.width, Math.max(160, coverPane.height - 100))
                anchors.centerIn: parent
                spacing: 0
                Item {
                    width: parent.width
                    height: width
                    Rectangle {
                        x: 5
                        y: 9
                        width: parent.width - 10
                        height: parent.height
                        color: "#10000000"
                        radius: 3
                        visible: !page.coverHidden
                    }
                    CoverArt {
                        id: cover
                        anchors.fill: parent
                        source: page.controller.currentCoverUrl
                        title: page.controller.currentTitle
                        resolution: 768
                        opacity: page.coverHidden ? 0 : 1
                    }
                }
                Text {
                    width: parent.width
                    topPadding: 24
                    objectName: "listeningTitle"
                    text: page.controller.currentTitle || qsTr("选择一首歌曲")
                    textFormat: Text.PlainText
                    color: listeningPalette.text
                    font.family: Theme.fontFamily
                    font.pixelSize: page.split ? 23 : 25
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    horizontalAlignment: page.split ? Text.AlignLeft : Text.AlignHCenter
                }
                Text {
                    width: parent.width
                    topPadding: 8
                    objectName: "listeningArtist"
                    text: page.controller.currentArtist || qsTr("")
                    textFormat: Text.PlainText
                    color: listeningPalette.secondary
                    font.family: Theme.fontFamily
                    font.pixelSize: 14
                    elide: Text.ElideRight
                    horizontalAlignment: page.split ? Text.AlignLeft : Text.AlignHCenter
                }
            }
        }
        Item {
            objectName: "nowPlayingLyricsPane"
            visible: page.showLyrics
            x: page.split ? coverPane.width + stage.gap : (stage.width - width) / 2
            width: page.split ? stage.width - coverPane.width - stage.gap : Math.min(760, stage.width)
            height: parent.height
            LyricsView {
                id: lyricView
                anchors.fill: parent
                controller: page.controller
                colors: listeningPalette
                positionMs: page.positionMs
                displayed: page.displayed && page.showLyrics
                showSecondary: Theme.lyricSecondary
            }
        }
    }
    Text {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 12
        text: qsTr("歌词偏移 %1 秒").arg((page.controller.lyricsOffsetMs / 1000).toFixed(1))
        visible: page.controller.lyricsOffsetMs !== 0
        color: listeningPalette.secondary
        font.pixelSize: Theme.noteSize
    }
}
