pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import Qt.labs.platform as Platform
import io.github.dhkun.Liusheng 1.0

ApplicationWindow {
    id: root

    readonly property bool darkMode: palette.window.hslLightness < 0.5
    readonly property color ink: darkMode ? "#0b1114" : "#eef2f3"
    readonly property color graphite: darkMode ? "#151d22" : "#ffffff"
    readonly property color fog: darkMode ? "#e8edf0" : "#172126"
    readonly property color muted: darkMode ? "#829198" : "#607078"
    readonly property color amber: darkMode ? "#d9a15f" : "#a56422"
    readonly property color rust: darkMode ? "#b85f4a" : "#a64736"
    readonly property color teal: darkMode ? "#6f9d99" : "#3f7773"
    readonly property bool smokeTest: Application.arguments.indexOf("--smoke-test") >= 0
    readonly property bool outputSmokeTest: Application.arguments.indexOf("--output-smoke-test") >= 0
    property int outputSmokePhase: 0
    property bool immersiveOpen: false
    property string activePage: "albums"
    readonly property bool albumsPage: activePage === "albums"
    readonly property bool allTracksPage: activePage === "allTracks"

    function albumAccent(index) {
        const colors = [root.rust, root.teal, root.amber]
        const normalizedIndex = ((index % colors.length) + colors.length) % colors.length
        return colors[normalizedIndex]
    }

    function restoreFromTray() {
        root.show()
        root.raise()
        root.requestActivate()
    }

    width: 1240
    height: 760
    minimumWidth: 960
    minimumHeight: 620
    visible: true
    title: qsTr("留声")
    color: ink

    onClosing: function(close) {
        if (trayIcon.available && !root.smokeTest && !root.outputSmokeTest) {
            close.accepted = false
            root.hide()
        }
    }

    AppController {
        id: controller
    }

    Platform.SystemTrayIcon {
        id: trayIcon

        visible: available
        icon.source: "qrc:/qt/qml/io/github/dhkun/Liusheng/qml/assets/tray.svg"
        tooltip: controller.hasCurrentTrack
                 ? qsTr("留声 · %1").arg(controller.currentTitle)
                 : qsTr("留声")

        onActivated: function(reason) {
            if (reason === Platform.SystemTrayIcon.Trigger
                    || reason === Platform.SystemTrayIcon.DoubleClick) {
                root.restoreFromTray()
            }
        }

        menu: Platform.Menu {
            Platform.MenuItem {
                text: qsTr("显示留声")
                onTriggered: root.restoreFromTray()
            }

            Platform.MenuSeparator {}

            Platform.MenuItem {
                text: qsTr("上一首")
                enabled: controller.hasCurrentTrack
                onTriggered: controller.previousTrack()
            }

            Platform.MenuItem {
                text: controller.playing ? qsTr("暂停") : qsTr("继续播放")
                enabled: controller.hasCurrentTrack
                onTriggered: controller.togglePlayback()
            }

            Platform.MenuItem {
                text: qsTr("下一首")
                enabled: controller.hasCurrentTrack
                onTriggered: controller.nextTrack()
            }

            Platform.MenuSeparator {}

            Platform.MenuItem {
                text: qsTr("退出")
                onTriggered: Qt.quit()
            }
        }
    }

    Component.onCompleted: {
        root.raise()
        root.requestActivate()
        controller.refreshHardwareVolume()
        if (root.outputSmokeTest) {
            root.outputSmokePhase = 1
            controller.requestExclusiveOutput(true)
        } else if (root.smokeTest) {
            Qt.callLater(root.close)
        } else {
            controller.scanLibrary()
        }
    }

    Connections {
        target: controller

        function onOutputSwitchingChanged() {
            if (!root.outputSmokeTest || controller.outputSwitching)
                return
            if (root.outputSmokePhase === 1 && controller.exclusiveOutput) {
                root.outputSmokePhase = 2
                controller.requestExclusiveOutput(false)
            } else if (root.outputSmokePhase === 2 && !controller.exclusiveOutput) {
                console.info("output smoke test passed")
                Qt.quit()
            } else {
                console.error("output smoke test failed: " + controller.outputError)
                Qt.exit(1)
            }
        }
    }

    Timer {
        interval: 15000
        running: root.outputSmokeTest
        onTriggered: {
            console.error("output smoke test timed out")
            Qt.exit(1)
        }
    }

    Rectangle {
        anchors.fill: parent
        color: root.ink

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: root.ink }
            GradientStop {
                position: 0.72
                color: Qt.tint(root.ink,
                               Qt.rgba(root.teal.r, root.teal.g, root.teal.b, 0.08))
            }
            GradientStop {
                position: 1.0
                color: Qt.tint(root.ink,
                               Qt.rgba(root.rust.r, root.rust.g, root.rust.b, 0.1))
            }
        }
    }

    Rectangle {
        id: sidebar
        width: 224
        anchors.top: parent.top
        anchors.bottom: playerBar.top
        anchors.left: parent.left
        color: Qt.rgba(root.graphite.r, root.graphite.g, root.graphite.b, 0.72)
        border.width: 1
        border.color: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 0

            Text {
                text: qsTr("留声")
                color: root.fog
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 26
                font.weight: Font.Bold
                font.letterSpacing: 6
                Layout.bottomMargin: 8
            }

            Text {
                text: qsTr("本地曲库")
                color: root.muted
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 11
                font.letterSpacing: 2
                Layout.bottomMargin: 42
            }

            NavButton {
                text: qsTr("专辑")
                selected: root.albumsPage
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
                onClicked: {
                    root.activePage = "albums"
                    controller.closeAlbum()
                }
            }
            NavButton {
                text: qsTr("艺术家")
                enabled: false
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
            }
            NavButton {
                text: qsTr("全部歌曲")
                selected: root.allTracksPage
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
                onClicked: {
                    root.activePage = "allTracks"
                    controller.closeAlbum()
                }
            }
            NavButton {
                text: qsTr("歌单")
                enabled: false
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
            }

            Item { Layout.fillHeight: true }

            OutputModeSwitch {
                Layout.fillWidth: true
                Layout.bottomMargin: 12
                exclusive: controller.exclusiveOutput
                busy: controller.outputSwitching
                statusText: controller.outputStatus
                errorText: controller.outputError
                surfaceColor: root.graphite
                foregroundColor: root.fog
                mutedColor: root.muted
                accentColor: root.amber
                errorColor: root.rust
                onModeRequested: function(exclusive) {
                    controller.requestExclusiveOutput(exclusive)
                }
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 74
                radius: 14
                color: Qt.rgba(root.teal.r, root.teal.g, root.teal.b, 0.1)
                border.width: 1
                border.color: Qt.rgba(root.teal.r, root.teal.g, root.teal.b, 0.18)

                Column {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    spacing: 5

                    Text {
                        width: parent.width
                        text: controller.status
                        color: root.fog
                        elide: Text.ElideRight
                        font.family: "Noto Sans CJK SC"
                        font.pixelSize: 12
                        font.weight: Font.Medium
                    }
                    Text {
                        width: parent.width
                        text: "/data/Music"
                        color: root.muted
                        elide: Text.ElideMiddle
                        font.family: "JetBrains Mono"
                        font.pixelSize: 10
                    }
                }
            }
        }
    }

    Item {
        id: content
        anchors.left: sidebar.right
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: playerBar.top
        clip: true

        Item {
            id: pageHeader

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.leftMargin: 48
            anchors.rightMargin: 48
            anchors.topMargin: 48
            height: 88

            Column {
                width: parent.width - 140
                spacing: 7

                Text {
                    width: parent.width
                    text: root.allTracksPage
                          ? qsTr("全部歌曲")
                          : controller.albumOpen
                          ? controller.albumTitle(controller.selectedAlbumIndex)
                          : qsTr("专辑")
                    color: root.fog
                    elide: Text.ElideRight
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: controller.albumOpen && root.albumsPage ? 40 : 62
                    font.weight: Font.Black
                    font.letterSpacing: controller.albumOpen && root.albumsPage ? -1 : -2
                }
                Text {
                    visible: root.allTracksPage
                             ? controller.trackCount > 0
                             : controller.albumCount > 0 || controller.albumOpen
                    text: root.allTracksPage
                          ? qsTr("%1 首歌曲").arg(controller.trackCount)
                          : controller.albumOpen
                          ? qsTr("%1，%2 首")
                            .arg(controller.albumArtist(controller.selectedAlbumIndex))
                            .arg(controller.selectedTrackCount)
                          : qsTr("%1 张专辑，%2 首歌曲")
                            .arg(controller.albumCount)
                            .arg(controller.trackCount)
                    color: root.muted
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: 12
                }
            }

            Rectangle {
                width: 52
                height: 3
                radius: 2
                color: root.amber
                anchors.left: parent.left
                anchors.bottom: parent.bottom
            }

            ActionButton {
                visible: root.allTracksPage
                         ? controller.trackCount > 0
                         : controller.albumCount > 0
                text: controller.albumOpen && root.albumsPage
                      ? qsTr("返回专辑")
                      : controller.scanning ? qsTr("扫描中") : qsTr("重新扫描")
                enabled: controller.albumOpen && root.albumsPage || !controller.scanning
                accentColor: root.amber
                foregroundColor: root.darkMode ? "#12181b" : "#ffffff"
                focusColor: root.fog
                disabledColor: root.muted
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                onClicked: {
                    if (controller.albumOpen && root.albumsPage)
                        controller.closeAlbum()
                    else
                        controller.scanLibrary()
                }
            }
        }

        Item {
            id: emptyState

            visible: root.allTracksPage
                     ? controller.trackCount === 0
                     : !controller.albumOpen && controller.albumCount === 0
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pageHeader.bottom
            anchors.bottom: parent.bottom
            anchors.leftMargin: 48
            anchors.rightMargin: 48
            anchors.topMargin: 18
            anchors.bottomMargin: 36

            Column {
                width: Math.min(360, parent.width * 0.44)
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                spacing: 14

                Text {
                    width: parent.width
                    text: controller.status
                    color: root.fog
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: 28
                    font.weight: Font.DemiBold
                }
                Text {
                    width: parent.width
                    text: root.allTracksPage
                          ? qsTr("扫描完成后，这里会显示曲库中的全部歌曲。")
                          : controller.trackCount > 0
                            ? qsTr("有歌曲缺少专辑信息，请检查音频标签。")
                            : qsTr("扫描完成后，这里会按专辑整理本地音乐。")
                    color: root.muted
                    wrapMode: Text.WordWrap
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: 14
                    lineHeight: 1.55
                }
                Rectangle {
                    width: parent.width
                    height: 1
                    color: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.1)
                }
                Text {
                    text: qsTr("音乐目录  /data/Music")
                    color: root.teal
                    font.family: "JetBrains Mono"
                    font.pixelSize: 11
                }

                ActionButton {
                    text: controller.scanning ? qsTr("扫描中") : qsTr("重新扫描")
                    enabled: !controller.scanning
                    accentColor: root.amber
                    foregroundColor: root.darkMode ? "#12181b" : "#ffffff"
                    focusColor: root.fog
                    disabledColor: root.muted
                    onClicked: controller.scanLibrary()
                }
            }

            VinylMark {
                width: Math.min(parent.height * 0.8, parent.width * 0.46)
                height: width
                anchors.right: parent.right
                anchors.rightMargin: -width * 0.15
                anchors.verticalCenter: parent.verticalCenter
                discColor: root.darkMode ? "#11181c" : "#d9dfe1"
                grooveColor: root.darkMode ? "#536168" : "#7e8b91"
                labelColor: root.rust
                labelTextColor: root.darkMode ? "#f4e9dc" : "#fff8f1"
            }
        }

        GridView {
            id: albumGrid

            visible: root.albumsPage && !controller.albumOpen && controller.albumCount > 0
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pageHeader.bottom
            anchors.bottom: parent.bottom
            anchors.leftMargin: 48
            anchors.rightMargin: 34
            anchors.topMargin: 24
            anchors.bottomMargin: 18
            clip: true
            model: controller.albumCount
            cellWidth: width / Math.max(1, Math.floor(width / 210))
            cellHeight: cellWidth + 74
            boundsBehavior: Flickable.StopAtBounds

            ScrollBar.vertical: ScrollBar {
                policy: ScrollBar.AsNeeded
            }

            delegate: AlbumCard {
                id: albumDelegate

                required property int index

                width: albumGrid.cellWidth - 18
                height: albumGrid.cellHeight - 18
                albumTitle: controller.albumTitle(albumDelegate.index)
                albumArtist: controller.albumArtist(albumDelegate.index)
                coverSource: controller.albumCoverUrl(albumDelegate.index)
                trackCount: controller.albumTrackCount(albumDelegate.index)
                albumYear: controller.albumYear(albumDelegate.index)
                surfaceColor: root.graphite
                foregroundColor: root.fog
                mutedColor: root.muted
                accentColor: root.albumAccent(albumDelegate.index)
                surroundingColor: root.ink
                onActivated: controller.openAlbum(albumDelegate.index)
            }
        }

        Item {
            id: albumDetail

            visible: root.albumsPage && controller.albumOpen
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pageHeader.bottom
            anchors.bottom: parent.bottom
            anchors.leftMargin: 48
            anchors.rightMargin: 48
            anchors.topMargin: 24
            anchors.bottomMargin: 18

            AlbumCard {
                id: detailArtwork

                width: Math.min(220, albumDetail.width * 0.28)
                height: width + 76
                interactive: false
                albumTitle: controller.albumTitle(controller.selectedAlbumIndex)
                albumArtist: controller.albumArtist(controller.selectedAlbumIndex)
                coverSource: controller.albumCoverUrl(controller.selectedAlbumIndex)
                trackCount: controller.albumTrackCount(controller.selectedAlbumIndex)
                albumYear: controller.albumYear(controller.selectedAlbumIndex)
                surfaceColor: root.graphite
                foregroundColor: root.fog
                mutedColor: root.muted
                accentColor: root.albumAccent(controller.selectedAlbumIndex)
                surroundingColor: root.ink
                anchors.left: parent.left
                anchors.top: parent.top
            }

            Item {
                anchors.left: detailArtwork.right
                anchors.leftMargin: 40
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: parent.bottom

                Text {
                    id: trackListTitle

                    text: qsTr("曲目")
                    color: root.fog
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                }

                ListView {
                    id: trackList

                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: trackListTitle.bottom
                    anchors.topMargin: 10
                    anchors.bottom: parent.bottom
                    clip: true
                    model: controller.selectedTrackCount
                    boundsBehavior: Flickable.StopAtBounds

                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AsNeeded
                    }

                    delegate: TrackListRow {
                        id: trackDelegate

                        required property int index

                        width: trackList.width
                        trackNumber: controller.selectedTrackNumber(trackDelegate.index)
                        trackTitle: controller.selectedTrackTitle(trackDelegate.index)
                        trackArtist: controller.selectedTrackArtist(trackDelegate.index)
                        durationMs: controller.selectedTrackDurationMs(trackDelegate.index)
                        foregroundColor: root.fog
                        mutedColor: root.muted
                        accentColor: root.amber
                        current: controller.currentTrackPath.length > 0
                                 && controller.selectedTrackPath(trackDelegate.index)
                                    === controller.currentTrackPath
                        interactive: !controller.playbackInitializing
                        onActivated: controller.playSelectedTrack(trackDelegate.index)
                    }
                }
            }
        }

        ListView {
            id: allTrackList

            visible: root.allTracksPage && controller.trackCount > 0
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pageHeader.bottom
            anchors.bottom: parent.bottom
            anchors.leftMargin: 48
            anchors.rightMargin: 48
            anchors.topMargin: 24
            anchors.bottomMargin: 18
            clip: true
            model: controller.trackCount
            boundsBehavior: Flickable.StopAtBounds

            ScrollBar.vertical: ScrollBar {
                policy: ScrollBar.AsNeeded
            }

            delegate: TrackListRow {
                id: allTrackDelegate

                required property int index

                width: allTrackList.width
                trackNumber: {
                    controller.libraryRevision
                    return controller.allTrackNumber(allTrackDelegate.index)
                }
                trackTitle: {
                    controller.libraryRevision
                    return controller.allTrackTitle(allTrackDelegate.index)
                }
                trackArtist: {
                    controller.libraryRevision
                    return controller.allTrackArtist(allTrackDelegate.index)
                }
                durationMs: {
                    controller.libraryRevision
                    return controller.allTrackDurationMs(allTrackDelegate.index)
                }
                foregroundColor: root.fog
                mutedColor: root.muted
                accentColor: root.amber
                current: {
                    controller.libraryRevision
                    return controller.currentTrackPath.length > 0
                           && controller.allTrackPath(allTrackDelegate.index)
                              === controller.currentTrackPath
                }
                interactive: !controller.playbackInitializing
                onActivated: controller.playAllTrack(allTrackDelegate.index)
            }
        }
    }

    PlayerBar {
        id: playerBar
        height: 92
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        surfaceColor: Qt.rgba(root.graphite.r, root.graphite.g, root.graphite.b, 0.96)
        foregroundColor: root.fog
        mutedColor: root.muted
        accentColor: root.amber
        trackTitle: controller.currentTitle
        trackArtist: controller.currentArtist
        coverSource: controller.currentCoverUrl
        errorText: controller.playbackError
        positionMs: controller.positionMs
        durationMs: controller.currentDurationMs
        hasTrack: controller.hasCurrentTrack
        seekable: controller.seekable
        playing: controller.playing
        busy: controller.playbackInitializing
        volumeAvailable: controller.hardwareVolumeAvailable
        hardwareMuted: controller.hardwareMuted
        hardwareMuteAvailable: controller.hardwareMuteAvailable
        volumePercent: controller.hardwareVolumePercent
        volumeErrorText: controller.hardwareVolumeError
        onPreviousRequested: controller.previousTrack()
        onToggleRequested: controller.togglePlayback()
        onNextRequested: controller.nextTrack()
        onSeekRequested: positionMs => controller.seekTo(Math.round(positionMs))
        onVolumeRequested: percent => controller.requestHardwareVolume(percent)
        onMuteRequested: controller.toggleHardwareMute()
        onVolumeRefreshRequested: controller.refreshHardwareVolume()
        onImmersiveRequested: root.immersiveOpen = true
    }

    ImmersivePlayer {
        id: immersivePlayer

        z: 100
        visible: root.immersiveOpen
        anchors.fill: parent
        backgroundColor: root.ink
        surfaceColor: root.graphite
        foregroundColor: root.fog
        mutedColor: root.muted
        accentColor: root.amber
        secondaryColor: root.teal
        warmColor: root.rust
        trackTitle: controller.currentTitle
        trackArtist: controller.currentArtist
        coverSource: controller.currentCoverUrl
        lyricsError: controller.lyricsError
        positionMs: controller.positionMs
        durationMs: controller.currentDurationMs
        lyricLineCount: controller.lyricLineCount
        currentLyricIndex: controller.currentLyricIndex
        lyricsRevision: controller.lyricsRevision
        hasTrack: controller.hasCurrentTrack
        seekable: controller.seekable
        playing: controller.playing
        lyricsLoading: controller.lyricsLoading
        lyricsSynced: controller.lyricsSynced
        lyricTextProvider: function(index) { return controller.lyricText(index) }
        lyricTimeProvider: function(index) { return controller.lyricTimeMs(index) }
        onCloseRequested: root.immersiveOpen = false
        onPreviousRequested: controller.previousTrack()
        onToggleRequested: controller.togglePlayback()
        onNextRequested: controller.nextTrack()
        onSeekRequested: positionMs => controller.seekTo(positionMs)
    }
}
