pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import QtQuick.Dialogs
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
    readonly property var preferences: { try { return JSON.parse(appController.settingsJson) } catch (error) { return ({}) } }
    readonly property string musicRootLabel: (preferences.music_roots || []).join(" · ")
    readonly property bool playlistsPage: activePage === "playlists"
    readonly property bool startupBenchmark: Application.arguments.indexOf("--startup-benchmark") >= 0
    readonly property bool uiTest: Application.arguments.indexOf("--ui-test") >= 0
    property bool firstFrameSeen: false
    property bool restoredUi: false
    property bool immersiveCreated: false
    property bool settingsCreated: false
    property int uiTestStep: 0
    property int outputSmokePhase: 0
    property bool immersiveOpen: false
    property string activePage: "albums"
    readonly property bool albumsPage: activePage === "albums"
    readonly property bool artistsPage: activePage === "artists"
    readonly property bool allTracksPage: activePage === "allTracks"
    readonly property bool queuePage: activePage === "queue"

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

    function showPage(page) {
        root.activePage = page
        appController.closeAlbum()
        appController.closeArtist()
    }

    width: 1240
    height: 760
    minimumWidth: 960
    minimumHeight: 620
    visible: true
    title: qsTr("留声")
    color: ink

    onClosing: function(close) {
        root.persistUi()
        if (root.preferences.close_to_tray !== false && trayIcon.available && !root.smokeTest && !root.outputSmokeTest && !root.startupBenchmark && !root.uiTest) {
            close.accepted = false
            root.hide()
        }
    }

    AppController {
        id: appController
    }

    Platform.SystemTrayIcon {
        id: trayIcon

        visible: available
        icon.source: "qrc:/qt/qml/io/github/dhkun/Liusheng/qml/assets/tray.svg"
        tooltip: appController.hasCurrentTrack
                 ? qsTr("留声 · %1").arg(appController.currentTitle)
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
                enabled: appController.hasCurrentTrack
                onTriggered: appController.previousTrack()
            }

            Platform.MenuItem {
                text: appController.playing ? qsTr("暂停") : qsTr("继续播放")
                enabled: appController.hasCurrentTrack
                onTriggered: appController.togglePlayback()
            }

            Platform.MenuItem {
                text: qsTr("下一首")
                enabled: appController.hasCurrentTrack
                onTriggered: appController.nextTrack()
            }

            Platform.MenuSeparator {}

            Platform.MenuItem {
                text: qsTr("退出")
                onTriggered: { root.persistUi(); Qt.quit() }
            }
        }
    }

    Component.onCompleted: {
        root.raise()
        root.requestActivate()
        if (root.outputSmokeTest) {
            root.outputSmokePhase = 1
            appController.requestExclusiveOutput(true)
        } else if (root.smokeTest) {
            Qt.callLater(root.close)
        } else {
            appController.scanLibrary()
            const files = DesktopBridge.initialFiles()
            if (files !== "[]") appController.openFiles(files)
        }
    }
    onFrameSwapped: {
        if (!firstFrameSeen) { firstFrameSeen = true; DesktopBridge.profileMark("first_frame_ms"); benchmarkExit.restart() }
    }
    function persistUi() {
        appController.saveUiState(JSON.stringify({page: activePage, width: width, height: height,
            album_scroll: albumGrid.contentY, track_scroll: allTrackList.contentY}))
    }
    function openSettings() { settingsCreated = true; if (settingsLoader.item) settingsLoader.item.open() }
    function restoreUi() {
        if (restoredUi || !appController.libraryReady) return
        restoredUi = true
        if (preferences.restore_session === false) return
        let saved = ({})
        try { saved = JSON.parse(appController.savedUiJson) } catch (error) { return }
        if (saved.width >= minimumWidth) width = Math.min(saved.width, Screen.desktopAvailableWidth)
        if (saved.height >= minimumHeight) height = Math.min(saved.height, Screen.desktopAvailableHeight)
        if (["albums", "artists", "allTracks", "queue", "playlists"].indexOf(saved.page) >= 0) activePage = saved.page
        Qt.callLater(function() {
            albumGrid.contentY = Math.max(0, Math.min(saved.album_scroll || 0, Math.max(0, albumGrid.contentHeight - albumGrid.height)))
            allTrackList.contentY = Math.max(0, Math.min(saved.track_scroll || 0, Math.max(0, allTrackList.contentHeight - allTrackList.height)))
        })
    }
    Timer { interval: 5000; repeat: true; running: appController.libraryReady; onTriggered: root.persistUi() }
    Timer { id: benchmarkExit; interval: 10; onTriggered: {
        if (root.startupBenchmark && root.firstFrameSeen && appController.libraryReady) { DesktopBridge.profileMark("interactive_ms"); Qt.quit() }
    } }
    Timer { interval: 20000; running: root.startupBenchmark || root.uiTest; onTriggered: { console.error("UI validation timeout"); Qt.exit(2) } }
    Connections { target: DesktopBridge; function onOpenRequested(pathsJson) { appController.openFiles(pathsJson) } function onRaiseRequested() { root.restoreFromTray() } }
    Connections {
        target: appController
        function onRaiseRequested() { root.restoreFromTray() }
        function onQuitRequested() { root.persistUi(); Qt.quit() }
        function onLibraryReadyChanged() { root.restoreUi(); benchmarkExit.restart() }
        function onLibraryRevisionChanged() { albumModel.refresh(); artistModel.refresh(); trackModel.refresh(); selectedModel.refresh() }
        function onQueueRevisionChanged() { queueModel.refresh() }
        function onArtworkRevisionChanged() { albumModel.refresh(); artistModel.refresh() }
    }
    UiModel { id: albumModel; kind: "albums"; onQueryChanged: refresh(); Component.onCompleted: refresh() }
    UiModel { id: artistModel; kind: "artists"; onQueryChanged: refresh(); Component.onCompleted: refresh() }
    UiModel { id: trackModel; kind: "tracks"; Component.onCompleted: refresh() }
    UiModel { id: selectedModel; kind: "selected"; Component.onCompleted: refresh() }
    UiModel { id: queueModel; kind: "queue"; Component.onCompleted: refresh() }
    PlaybackClock { id: playbackClock; sourcePosition: appController.positionMs; duration: appController.currentDurationMs; playing: appController.playing; displayed: root.visible }
    FileDialog {
        id: audioFiles
        title: qsTr("打开本地音乐")
        fileMode: FileDialog.OpenFiles
        nameFilters: [qsTr("音乐与歌单 (*.flac *.mp3 *.m4a *.ogg *.wav *.aiff *.aif *.m3u8 *.m3u)"), qsTr("所有文件 (*)")]
        onAccepted: appController.openFiles(JSON.stringify(selectedFiles.map(url => url.toString())))
    }
    DropArea { anchors.fill: parent; onDropped: function(drop) { if (drop.hasUrls) { appController.openFiles(JSON.stringify(drop.urls.map(url => url.toString()))); drop.acceptProposedAction() } } }
    Shortcut { sequence: "Ctrl+O"; onActivated: audioFiles.open() }
    Shortcut { sequence: "Ctrl+,"; onActivated: root.openSettings() }
    Shortcut { sequence: "Ctrl+F"; onActivated: { root.showPage("allTracks"); trackSearch.forceActiveFocus() } }
    Shortcut { sequence: "Space"; enabled: appController.hasCurrentTrack && !(root.activeFocusItem && (root.activeFocusItem.hasOwnProperty("text") && root.activeFocusItem.hasOwnProperty("readOnly"))); onActivated: appController.togglePlayback() }
    Loader {
        id: settingsLoader
        active: root.settingsCreated
        asynchronous: true
        sourceComponent: Component { SettingsDialog { controller: appController; parent: Overlay.overlay; anchors.centerIn: parent } }
        onLoaded: item.open()
    }


    Connections {
        target: appController

        function onOutputSwitchingChanged() {
            if (!root.outputSmokeTest || appController.outputSwitching)
                return
            if (root.outputSmokePhase === 1 && appController.exclusiveOutput) {
                root.outputSmokePhase = 2
                appController.requestExclusiveOutput(false)
            } else if (root.outputSmokePhase === 2 && !appController.exclusiveOutput) {
                console.info("output smoke test passed")
                Qt.quit()
            } else {
                console.error("output smoke test failed: " + appController.outputError)
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
                onClicked: root.showPage("albums")
            }
            NavButton {
                text: qsTr("艺术家")
                selected: root.artistsPage
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
                onClicked: root.showPage("artists")
            }
            NavButton {
                text: qsTr("全部歌曲")
                selected: root.allTracksPage
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
                onClicked: root.showPage("allTracks")
            }
            NavButton {
                text: qsTr("播放队列")
                selected: root.queuePage
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
                onClicked: root.showPage("queue")
            }
            NavButton {
                text: qsTr("歌单")
                selected: root.playlistsPage
                onClicked: root.showPage("playlists")
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
            }

            NavButton { text: qsTr("打开文件…"); accentColor: root.amber; foregroundColor: root.fog; onClicked: audioFiles.open() }
            NavButton { text: qsTr("设置"); accentColor: root.amber; foregroundColor: root.fog; onClicked: root.openSettings() }
            Item { Layout.fillHeight: true }

            OutputModeSwitch {
                Layout.fillWidth: true
                Layout.bottomMargin: 12
                supportsExclusive: Qt.platform.os === "linux"
                exclusive: appController.exclusiveOutput
                busy: appController.outputSwitching
                statusText: appController.outputStatus
                errorText: appController.outputError
                surfaceColor: root.graphite
                foregroundColor: root.fog
                mutedColor: root.muted
                accentColor: root.amber
                errorColor: root.rust
                onModeRequested: function(exclusive) {
                    appController.requestExclusiveOutput(exclusive)
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
                        text: appController.status
                        color: root.fog
                        elide: Text.ElideRight
                        font.family: "Noto Sans CJK SC"
                        font.pixelSize: 12
                        font.weight: Font.Medium
                    }
                    Text {
                        width: parent.width
                        text: root.musicRootLabel
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
                    text: root.playlistsPage ? qsTr("歌单") : root.queuePage
                          ? qsTr("播放队列")
                          : root.allTracksPage
                          ? qsTr("全部歌曲")
                          : root.artistsPage
                            ? appController.artistOpen
                              ? appController.artistName(appController.selectedArtistIndex)
                              : qsTr("艺术家")
                          : appController.albumOpen
                            ? appController.albumTitle(appController.selectedAlbumIndex)
                            : qsTr("专辑")
                    color: root.fog
                    elide: Text.ElideRight
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: appController.albumOpen && root.albumsPage
                                    || appController.artistOpen && root.artistsPage
                                    ? 40 : 62
                    font.weight: Font.Black
                    font.letterSpacing: appController.albumOpen && root.albumsPage
                                        || appController.artistOpen && root.artistsPage
                                        ? -1 : -2
                }
                Text {
                    visible: root.playlistsPage || (root.queuePage
                             ? appController.queueCount > 0
                             : root.allTracksPage
                             ? appController.trackCount > 0
                             : root.artistsPage
                               ? appController.artistCount > 0
                             : appController.albumCount > 0 || appController.albumOpen)
                    text: root.playlistsPage ? qsTr("%1 份歌单").arg(appController.playlistCount) : root.queuePage
                          ? qsTr("第 %1 首，共 %2 首")
                            .arg(appController.currentQueueIndex + 1)
                            .arg(appController.queueCount)
                          : root.allTracksPage
                          ? appController.trackFilter.length > 0
                            ? qsTr("%1 个结果").arg(appController.visibleTrackCount)
                            : qsTr("%1 首歌曲").arg(appController.trackCount)
                          : root.artistsPage
                            ? appController.artistOpen
                              ? qsTr("%1 张专辑，%2 首歌曲")
                                .arg(appController.artistAlbumCount(appController.selectedArtistIndex))
                                .arg(appController.selectedTrackCount)
                              : qsTr("%1 位艺术家，%2 首歌曲")
                                .arg(appController.artistCount)
                                .arg(appController.trackCount)
                          : appController.albumOpen
                            ? qsTr("%1，%2 首")
                              .arg(appController.albumArtist(appController.selectedAlbumIndex))
                              .arg(appController.selectedTrackCount)
                            : qsTr("%1 张专辑，%2 首歌曲")
                              .arg(appController.albumCount)
                              .arg(appController.trackCount)
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
                visible: root.queuePage
                         ? false
                         : root.allTracksPage
                         ? appController.trackCount > 0
                         : root.artistsPage
                           ? appController.artistCount > 0
                         : appController.albumCount > 0
                text: appController.albumOpen && root.albumsPage
                      ? qsTr("返回专辑")
                      : appController.artistOpen && root.artistsPage
                        ? qsTr("返回艺术家")
                        : appController.scanning ? qsTr("扫描中") : qsTr("重新扫描")
                enabled: appController.albumOpen && root.albumsPage
                         || appController.artistOpen && root.artistsPage
                         || !appController.scanning
                accentColor: root.amber
                foregroundColor: root.darkMode ? "#12181b" : "#ffffff"
                focusColor: root.fog
                disabledColor: root.muted
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                onClicked: {
                    if (appController.albumOpen && root.albumsPage)
                        appController.closeAlbum()
                    else if (appController.artistOpen && root.artistsPage)
                        appController.closeArtist()
                    else
                        appController.scanLibrary()
                }
            }
        }

        Item {
            id: emptyState

            visible: root.queuePage
                     ? appController.queueCount === 0
                     : root.allTracksPage
                     ? appController.trackCount === 0
                     : root.artistsPage
                       ? appController.artistCount === 0
                     : !appController.albumOpen && appController.albumCount === 0
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
                    text: root.queuePage ? qsTr("队列为空") : appController.status
                    color: root.fog
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: 28
                    font.weight: Font.DemiBold
                }
                Text {
                    width: parent.width
                    text: root.queuePage
                          ? qsTr("从曲库中播放一首歌，队列会显示在这里。")
                          : root.allTracksPage
                          ? qsTr("扫描完成后，这里会显示曲库中的全部歌曲。")
                          : root.artistsPage
                            ? qsTr("扫描完成后，这里会按艺术家整理本地音乐。")
                          : appController.trackCount > 0
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
                    visible: !root.queuePage
                    text: qsTr("音乐目录  %1").arg(root.musicRootLabel)
                    color: root.teal
                    font.family: "JetBrains Mono"
                    font.pixelSize: 11
                }

                ActionButton {
                    visible: !root.queuePage
                    text: appController.scanning ? qsTr("扫描中") : qsTr("重新扫描")
                    enabled: !appController.scanning
                    accentColor: root.amber
                    foregroundColor: root.darkMode ? "#12181b" : "#ffffff"
                    focusColor: root.fog
                    disabledColor: root.muted
                    onClicked: appController.scanLibrary()
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

        TextField {
            id: collectionSearch
            visible: (root.albumsPage && !appController.albumOpen) || (root.artistsPage && !appController.artistOpen)
            anchors.right: parent.right; anchors.rightMargin: 48; anchors.top: parent.top; anchors.topMargin: 72
            width: Math.min(220, parent.width * 0.27)
            placeholderText: root.artistsPage ? qsTr("搜索艺术家 / 拼音") : qsTr("搜索专辑 / 拼音")
            selectByMouse: true
            onTextChanged: collectionSearchTimer.restart()
        }
        Timer { id: collectionSearchTimer; interval: 180; onTriggered: { albumModel.query = collectionSearch.text; artistModel.query = collectionSearch.text } }
        Loader {
            active: root.playlistsPage
            asynchronous: true
            anchors.left: parent.left; anchors.right: parent.right; anchors.top: pageHeader.bottom; anchors.bottom: parent.bottom
            anchors.leftMargin: 48; anchors.rightMargin: 48; anchors.topMargin: 24; anchors.bottomMargin: 18
            sourceComponent: Component { PlaylistsPage { controller: appController; foregroundColor: root.fog; mutedColor: root.muted; accentColor: root.amber } }
        }
        GridView {
            id: albumGrid

            visible: root.albumsPage && !appController.albumOpen && appController.albumCount > 0
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pageHeader.bottom
            anchors.bottom: parent.bottom
            anchors.leftMargin: 48
            anchors.rightMargin: 34
            anchors.topMargin: 24
            anchors.bottomMargin: 18
            clip: true
            model: albumModel
            cellWidth: width / Math.max(1, Math.floor(width / 210))
            cellHeight: cellWidth + 74
            boundsBehavior: Flickable.StopAtBounds

            ScrollBar.vertical: ScrollBar {
                policy: ScrollBar.AsNeeded
            }

            delegate: AlbumCard {
                id: albumDelegate

                required property int index
                        required property var model
                        function requestCover() { if (visible && root.albumsPage) appController.requestAlbumCover(model.sourceIndex) }
                        Component.onCompleted: Qt.callLater(requestCover)
                        onVisibleChanged: Qt.callLater(requestCover)
                        Connections { target: appController; function onArtworkRevisionChanged() { albumDelegate.requestCover() } }


                width: albumGrid.cellWidth - 18
                height: albumGrid.cellHeight - 18
                albumTitle: albumDelegate.model.rowTitle
                albumArtist: albumDelegate.model.rowArtist
                coverSource: albumDelegate.model.rowCover
                trackCount: albumDelegate.model.rowTrackCount
                albumYear: albumDelegate.model.rowYear
                surfaceColor: root.graphite
                foregroundColor: root.fog
                mutedColor: root.muted
                accentColor: root.albumAccent(albumDelegate.index)
                surroundingColor: root.ink
                onActivated: appController.openAlbum(albumDelegate.model.sourceIndex)
            }
        }

        Item {
            id: albumDetail

            visible: root.albumsPage && appController.albumOpen
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
                albumTitle: appController.albumTitle(appController.selectedAlbumIndex)
                albumArtist: appController.albumArtist(appController.selectedAlbumIndex)
                coverSource: appController.albumCoverUrl(appController.selectedAlbumIndex)
                trackCount: appController.albumTrackCount(appController.selectedAlbumIndex)
                albumYear: appController.albumYear(appController.selectedAlbumIndex)
                surfaceColor: root.graphite
                foregroundColor: root.fog
                mutedColor: root.muted
                accentColor: root.albumAccent(appController.selectedAlbumIndex)
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
                    model: selectedModel
                    boundsBehavior: Flickable.StopAtBounds

                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AsNeeded
                    }

                    delegate: TrackActionRow {
                        id: trackDelegate

                        required property int index
                        required property var model

                        width: trackList.width
                        trackNumber: trackDelegate.model.rowNumber
                        trackTitle: trackDelegate.model.rowTitle
                        trackArtist: trackDelegate.model.rowArtist
                        durationMs: trackDelegate.model.rowDuration
                        foregroundColor: root.fog
                        mutedColor: root.muted
                        accentColor: root.amber
                        surfaceColor: root.graphite
                        queueActionsAvailable: appController.hasCurrentTrack && appController.seekable
                        current: appController.currentTrackPath.length > 0
                                 && trackDelegate.model.rowPath
                                    === appController.currentTrackPath
                        interactive: !appController.playbackInitializing
                        onActivated: appController.playSelectedTrack(trackDelegate.model.sourceIndex)
                        onEnqueueRequested: function(playNext) {
                            appController.enqueueSelectedTrack(trackDelegate.model.sourceIndex, playNext)
                        }
                    }
                }
            }
        }

        Item {
            id: artistIndex

            visible: root.artistsPage && !appController.artistOpen
                     && appController.artistCount > 0
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pageHeader.bottom
            anchors.bottom: parent.bottom
            anchors.leftMargin: 48
            anchors.rightMargin: 48
            anchors.topMargin: 24
            anchors.bottomMargin: 18

            ListView {
                id: artistList

                anchors.fill: parent
                clip: true
                model: artistModel
                spacing: 4
                boundsBehavior: Flickable.StopAtBounds

                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                }

                delegate: ArtistListRow {
                    id: artistDelegate

                    required property int index
                        required property var model
                        function requestCover() { if (visible && root.artistsPage) appController.requestArtistCover(model.sourceIndex) }
                        Component.onCompleted: Qt.callLater(requestCover)
                        onVisibleChanged: Qt.callLater(requestCover)
                        Connections { target: appController; function onArtworkRevisionChanged() { artistDelegate.requestCover() } }


                    width: artistList.width
                    sequence: String(artistDelegate.index + 1).padStart(2, "0")
                    artistName: artistDelegate.model.rowTitle
                    coverSource: artistDelegate.model.rowCover
                    trackCount: artistDelegate.model.rowTrackCount
                    albumCount: artistDelegate.model.rowAlbumCount
                    backgroundColor: root.ink
                    surfaceColor: root.graphite
                    foregroundColor: root.fog
                    mutedColor: root.muted
                    accentColor: root.albumAccent(artistDelegate.index)
                    onActivated: appController.openArtist(artistDelegate.model.sourceIndex)
                }
            }
        }

        Item {
            id: artistDetail

            visible: root.artistsPage && appController.artistOpen
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pageHeader.bottom
            anchors.bottom: parent.bottom
            anchors.leftMargin: 48
            anchors.rightMargin: 48
            anchors.topMargin: 24
            anchors.bottomMargin: 18

            CoverArt {
                id: artistPortrait

                width: Math.min(220, artistDetail.width * 0.28)
                height: width
                anchors.left: parent.left
                anchors.top: parent.top
                source: appController.artistCoverUrl(appController.selectedArtistIndex)
                title: appController.artistName(appController.selectedArtistIndex)
                surfaceColor: root.graphite
                foregroundColor: root.fog
                accentColor: root.albumAccent(appController.selectedArtistIndex)
                surroundingColor: root.ink
                cornerRadius: width / 2
                frameWidth: 1
                frameColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.1)
            }

            Rectangle {
                width: artistPortrait.width * 0.56
                height: 3
                radius: 2
                anchors.top: artistPortrait.bottom
                anchors.topMargin: 22
                anchors.horizontalCenter: artistPortrait.horizontalCenter
                color: root.albumAccent(appController.selectedArtistIndex)
            }

            Item {
                anchors.left: artistPortrait.right
                anchors.leftMargin: 40
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: parent.bottom

                Text {
                    id: artistTrackListTitle

                    text: qsTr("歌曲")
                    color: root.fog
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                }

                ListView {
                    id: artistTrackList

                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: artistTrackListTitle.bottom
                    anchors.topMargin: 10
                    anchors.bottom: parent.bottom
                    clip: true
                    model: selectedModel
                    boundsBehavior: Flickable.StopAtBounds

                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AsNeeded
                    }

                    delegate: TrackActionRow {
                        id: artistTrackDelegate

                        required property int index
                        required property var model

                        width: artistTrackList.width
                        trackNumber: artistTrackDelegate.model.rowNumber
                        trackTitle: artistTrackDelegate.model.rowTitle
                        trackArtist: artistTrackDelegate.model.rowArtist
                        trackAlbum: artistTrackDelegate.model.rowAlbum
                        durationMs: artistTrackDelegate.model.rowDuration
                        foregroundColor: root.fog
                        mutedColor: root.muted
                        accentColor: root.amber
                        surfaceColor: root.graphite
                        queueActionsAvailable: appController.hasCurrentTrack && appController.seekable
                        current: appController.currentTrackPath.length > 0
                                 && artistTrackDelegate.model.rowPath
                                    === appController.currentTrackPath
                        interactive: !appController.playbackInitializing
                        onActivated: appController.playSelectedTrack(artistTrackDelegate.model.sourceIndex)
                        onEnqueueRequested: function(playNext) {
                            appController.enqueueSelectedTrack(artistTrackDelegate.model.sourceIndex, playNext)
                        }
                    }
                }
            }
        }

        Item {
            id: queuePage

            visible: root.queuePage && appController.queueCount > 0
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pageHeader.bottom
            anchors.bottom: parent.bottom
            anchors.leftMargin: 48
            anchors.rightMargin: 48
            anchors.topMargin: 24
            anchors.bottomMargin: 18

            Item {
                id: queueNowPlaying

                width: Math.min(210, queuePage.width * 0.26)
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom

                CoverArt {
                    id: queueArtwork

                    width: parent.width
                    height: width
                    anchors.top: parent.top
                    source: appController.currentCoverUrl
                    title: appController.currentTitle
                    surfaceColor: root.graphite
                    foregroundColor: root.fog
                    accentColor: root.rust
                    surroundingColor: root.ink
                    cornerRadius: 18
                    frameWidth: 1
                    frameColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.1)
                }

                Column {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: queueArtwork.bottom
                    anchors.topMargin: 18
                    spacing: 7

                    Text {
                        width: parent.width
                        text: qsTr("当前播放")
                        color: root.amber
                        font.family: "Noto Sans CJK SC"
                        font.pixelSize: 11
                        font.weight: Font.DemiBold
                        font.letterSpacing: 1.5
                    }

                    Text {
                        width: parent.width
                        text: appController.currentTitle
                        color: root.fog
                        elide: Text.ElideRight
                        font.family: "Noto Sans CJK SC"
                        font.pixelSize: 18
                        font.weight: Font.Bold
                    }

                    Text {
                        width: parent.width
                        text: appController.currentArtist
                        color: root.muted
                        elide: Text.ElideRight
                        font.family: "Noto Sans CJK SC"
                        font.pixelSize: 12
                    }
                }
            }

            Item {
                anchors.left: queueNowPlaying.right
                anchors.leftMargin: 40
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: parent.bottom

                Item {
                    id: queueListHeader

                    anchors.left: parent.left
                    anchors.right: parent.right
                    height: 38

                    Column {
                        anchors.left: parent.left
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2

                        Text {
                            text: qsTr("播放顺序")
                            color: root.fog
                            font.family: "Noto Sans CJK SC"
                            font.pixelSize: 16
                            font.weight: Font.DemiBold
                        }

                        Text {
                            text: qsTr("%1 首").arg(appController.queueCount)
                            color: root.muted
                            font.family: "Noto Sans CJK SC"
                            font.pixelSize: 10
                        }
                    }

                    Button {
                        id: clearQueueButton

                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        width: 86
                        height: 32
                        text: qsTr("清空队列")
                        focusPolicy: Qt.StrongFocus
                        Accessible.name: text
                        onClicked: appController.clearQueue()

                        contentItem: Text {
                            text: clearQueueButton.text
                            color: clearQueueButton.hovered || clearQueueButton.activeFocus
                                   ? root.rust
                                   : root.muted
                            font.family: "Noto Sans CJK SC"
                            font.pixelSize: 11
                            font.weight: Font.Medium
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }

                        background: Rectangle {
                            color: clearQueueButton.hovered
                                   ? Qt.rgba(root.rust.r, root.rust.g, root.rust.b, 0.1)
                                   : "transparent"
                            radius: 16
                            border.width: clearQueueButton.activeFocus ? 1 : 0
                            border.color: root.rust

                            Behavior on color {
                                ColorAnimation { duration: 120 }
                            }
                        }
                    }
                }

                ListView {
                    id: queueList

                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: queueListHeader.bottom
                    anchors.topMargin: 10
                    anchors.bottom: parent.bottom
                    clip: true
                    model: queueModel
                    currentIndex: appController.currentQueueIndex
                    boundsBehavior: Flickable.StopAtBounds
                    onCurrentIndexChanged: {
                        if (currentIndex >= 0)
                            positionViewAtIndex(currentIndex, ListView.Contain)
                    }

                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AsNeeded
                    }

                    delegate: QueueTrackRow {
                        id: queueTrackDelegate

                        required property int index
                        required property var model

                        width: queueList.width
                        trackNumber: queueTrackDelegate.model.rowNumber
                        trackTitle: queueTrackDelegate.model.rowTitle
                        trackArtist: queueTrackDelegate.model.rowArtist
                        trackAlbum: queueTrackDelegate.model.rowAlbum
                        durationMs: queueTrackDelegate.model.rowDuration
                        foregroundColor: root.fog
                        mutedColor: root.muted
                        accentColor: root.amber
                        dangerColor: root.rust
                        current: queueTrackDelegate.index === appController.currentQueueIndex
                        interactive: !appController.playbackInitializing
                        onActivated: appController.playQueueTrack(queueTrackDelegate.model.sourceIndex)
                        onRemoveRequested: appController.removeQueueTrack(queueTrackDelegate.model.sourceIndex)
                        canMoveUp: queueTrackDelegate.model.sourceIndex > 0
                        canMoveDown: queueTrackDelegate.model.sourceIndex + 1 < appController.queueCount
                        onReorderRequested: displacement => appController.moveQueueTrack(queueTrackDelegate.model.sourceIndex,
                            Math.max(0, Math.min(appController.queueCount - 1, queueTrackDelegate.model.sourceIndex + displacement)))
                        onMoveUpRequested: appController.moveQueueTrack(queueTrackDelegate.model.sourceIndex, queueTrackDelegate.model.sourceIndex - 1)
                        onMoveDownRequested: appController.moveQueueTrack(queueTrackDelegate.model.sourceIndex, queueTrackDelegate.model.sourceIndex + 1)
                    }
                }
            }
        }

        Item {
            id: allTracksPage
            visible: root.allTracksPage && appController.trackCount > 0
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: pageHeader.bottom
            anchors.bottom: parent.bottom
            anchors.leftMargin: 48
            anchors.rightMargin: 48
            anchors.topMargin: 24
            anchors.bottomMargin: 18

            Timer {
                id: trackFilterTimer
                interval: 180
                onTriggered: appController.filterTracks(trackSearch.text)
            }

            TextField {
                id: trackSearch

                height: 44
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                rightPadding: clearSearch.visible ? 72 : 16
                leftPadding: 16
                placeholderText: qsTr("搜索歌曲、艺术家或专辑")
                placeholderTextColor: root.muted
                color: root.fog
                selectionColor: root.amber
                selectedTextColor: root.darkMode ? "#12181b" : "#ffffff"
                selectByMouse: true
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 13
                Keys.onEscapePressed: {
                    text = ""
                    trackFilterTimer.stop()
                    appController.filterTracks(text)
                }
                onTextChanged: trackFilterTimer.restart()

                background: Rectangle {
                    radius: 10
                    color: Qt.rgba(root.graphite.r,
                                   root.graphite.g,
                                   root.graphite.b,
                                   0.82)
                    border.width: trackSearch.activeFocus ? 1 : 0
                    border.color: root.amber
                }
            }

            Button {
                id: clearSearch

                visible: trackSearch.text.length > 0
                width: 64
                height: trackSearch.height
                anchors.right: trackSearch.right
                anchors.verticalCenter: trackSearch.verticalCenter
                text: qsTr("清除")
                focusPolicy: Qt.StrongFocus
                onClicked: {
                    trackSearch.text = ""
                    trackFilterTimer.stop()
                    appController.filterTracks(trackSearch.text)
                    trackSearch.forceActiveFocus()
                }

                contentItem: Text {
                    text: clearSearch.text
                    color: root.amber
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                }

                background: Item {}
            }

            Text {
                visible: appController.visibleTrackCount === 0
                         && appController.trackFilter.length > 0
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.top: trackSearch.bottom
                anchors.topMargin: 72
                text: qsTr("没有匹配的歌曲")
                color: root.muted
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 16
            }

            RowLayout {
                id: trackOptions
                anchors.left: parent.left; anchors.right: parent.right; anchors.top: trackSearch.bottom; anchors.topMargin: 8
                height: 36
                ComboBox {
                    id: sortBox
                    model: [qsTr("专辑顺序"), qsTr("标题"), qsTr("艺术家"), qsTr("年份（新到旧）"), qsTr("时长")]
                    currentIndex: appController.sortOrder
                    onActivated: appController.sortTracks(currentIndex, formatBox.currentIndex === 0 ? "" : formatBox.currentText)
                }
                ComboBox { id: formatBox; model: [qsTr("所有格式"), "flac", "mp3", "m4a", "ogg", "wav", "aiff"]; onActivated: appController.sortTracks(sortBox.currentIndex, currentIndex === 0 ? "" : currentText) }
                Label { text: appController.searching ? qsTr("搜索中…") : qsTr("%1 首").arg(appController.visibleTrackCount); color: root.muted; Layout.fillWidth: true }
            }
            ListView {
                id: allTrackList

                visible: appController.visibleTrackCount > 0
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: trackOptions.bottom
                anchors.topMargin: 14
                anchors.bottom: parent.bottom
                clip: true
                model: trackModel
                boundsBehavior: Flickable.StopAtBounds

                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                }

                delegate: TrackActionRow {
                    id: allTrackDelegate

                    required property int index
                        required property var model

                    width: allTrackList.width
                    trackNumber: allTrackDelegate.model.rowNumber
                    trackTitle: allTrackDelegate.model.rowTitle
                    trackArtist: allTrackDelegate.model.rowArtist
                    trackAlbum: allTrackDelegate.model.rowAlbum
                    durationMs: allTrackDelegate.model.rowDuration
                    foregroundColor: root.fog
                    mutedColor: root.muted
                    accentColor: root.amber
                    surfaceColor: root.graphite
                    queueActionsAvailable: appController.hasCurrentTrack && appController.seekable
                    current: {
                        appController.libraryRevision
                        return appController.currentTrackPath.length > 0
                               && allTrackDelegate.model.rowPath
                                  === appController.currentTrackPath
                    }
                    interactive: !appController.playbackInitializing
                    onActivated: appController.playAllTrack(allTrackDelegate.model.sourceIndex)
                    onEnqueueRequested: function(playNext) {
                        appController.enqueueAllTrack(allTrackDelegate.model.sourceIndex, playNext)
                    }
                }
            }
        }
    }

    Rectangle {
        id: queueNotice

        z: 90
        visible: opacity > 0
        opacity: 0
        width: noticeText.implicitWidth + 34
        height: 42
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: playerBar.top
        anchors.bottomMargin: 14
        radius: 21
        color: root.graphite
        border.width: 1
        border.color: Qt.rgba(root.amber.r, root.amber.g, root.amber.b, 0.42)
        Accessible.name: noticeText.text

        Text {
            id: noticeText

            anchors.centerIn: parent
            text: appController.queueNotice
            color: root.fog
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 12
            font.weight: Font.Medium
        }

        Behavior on opacity {
            NumberAnimation { duration: 140 }
        }

        Connections {
            target: appController

            function onQueueNoticeRevisionChanged() {
                appController.queueNoticeRevision
                queueNotice.opacity = 1
                queueNoticeTimer.restart()
            }
        }

        Timer {
            id: queueNoticeTimer

            interval: 1800
            onTriggered: queueNotice.opacity = 0
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
        trackTitle: appController.currentTitle
        trackArtist: appController.currentArtist
        coverSource: appController.currentCoverUrl
        errorText: appController.playbackError
        positionMs: Math.round(playbackClock.position)
        durationMs: appController.currentDurationMs
        hasTrack: appController.hasCurrentTrack
        seekable: appController.seekable
        playing: appController.playing
        busy: appController.playbackInitializing
        showHardwareVolume: Qt.platform.os === "linux"
        volumeAvailable: appController.hardwareVolumeAvailable
        hardwareMuted: appController.hardwareMuted
        hardwareMuteAvailable: appController.hardwareMuteAvailable
        volumePercent: appController.hardwareVolumePercent
        volumeErrorText: appController.hardwareVolumeError
        repeatMode: appController.repeatMode
        shuffleEnabled: appController.shuffleEnabled
        onPlaybackModeRequested: (repeat, shuffle) => appController.requestPlaybackMode(repeat, shuffle)
        onInfoRequested: audioInfo.open()
        queueCount: appController.queueCount
        onPreviousRequested: appController.previousTrack()
        onToggleRequested: appController.togglePlayback()
        onNextRequested: appController.nextTrack()
        onSeekRequested: positionMs => appController.seekTo(Math.round(positionMs))
        onVolumeRequested: percent => appController.requestHardwareVolume(percent)
        onMuteRequested: appController.toggleHardwareMute()
        onVolumeRefreshRequested: appController.refreshHardwareVolume()
        onImmersiveRequested: root.immersiveOpen = true
        onQueueRequested: root.showPage("queue")
    }

    Dialog {
        id: audioInfo
        implicitWidth: 520
        implicitHeight: 250
        width: Math.min(520, root.width - 48)
        title: qsTr("音频信号链")
        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        standardButtons: Dialog.Close
        contentItem: Label { text: appController.audioDetails; wrapMode: Text.Wrap; padding: 12 }
    }
    Timer {
        interval: 350; repeat: true; running: root.uiTest && appController.libraryReady
        onTriggered: {
            DesktopBridge.captureForTest(root, String(root.uiTestStep))
            switch (root.uiTestStep++) {
                case 0: root.showPage("artists"); break
                case 1: root.showPage("allTracks"); appController.filterTracks("a"); break
                case 2: root.showPage("queue"); break
                case 3: root.showPage("playlists"); break
                case 4: root.openSettings(); break
                case 5: if (settingsLoader.item) settingsLoader.item.close(); root.immersiveOpen = true; break
                case 6: root.immersiveOpen = false; root.showPage("albums"); break
                default: console.info("UI validation passed"); Qt.quit()
            }
        }
    }
    onImmersiveOpenChanged: { if (immersiveOpen) immersiveCreated = true }
    Loader {
        anchors.fill: parent
        z: 100
        active: root.immersiveCreated
        visible: root.immersiveOpen
        asynchronous: true
        sourceComponent: Component {
    ImmersivePlayer {
        id: immersivePlayer

        z: 100
        visible: root.immersiveOpen
        anchors.fill: parent
        backgroundColor: root.ink
        surfaceColor: root.graphite
        foregroundColor: root.fog
        mutedColor: root.muted
        accentColor: appController.currentAccent
        secondaryColor: appController.currentAccent
        warmColor: root.rust
        trackTitle: appController.currentTitle
        trackArtist: appController.currentArtist
        coverSource: appController.currentCoverUrl
        lyricsError: appController.lyricsError
        lyricsOffsetMs: appController.lyricsOffsetMs
        onLyricsOffsetRequested: offset => appController.requestLyricsOffset(offset)
        positionMs: Math.round(playbackClock.position)
        durationMs: appController.currentDurationMs
        lyricLineCount: appController.lyricLineCount
        currentLyricIndex: appController.currentLyricIndex
        lyricsRevision: appController.lyricsRevision
        hasTrack: appController.hasCurrentTrack
        seekable: appController.seekable
        playing: appController.playing
        lyricsLoading: appController.lyricsLoading
        lyricsSynced: appController.lyricsSynced
        lyricTextProvider: function(index) { return appController.lyricText(index) }
        lyricTimeProvider: function(index) { return appController.lyricTimeMs(index) }
        onCloseRequested: root.immersiveOpen = false
        onPreviousRequested: appController.previousTrack()
        onToggleRequested: appController.togglePlayback()
        onNextRequested: appController.nextTrack()
        onSeekRequested: positionMs => appController.seekTo(positionMs)
    }        }
    }

    readonly property bool functionalTest: Application.arguments.indexOf("--functional-test") >= 0
    property int functionalStep: 0
    function checkTest(condition, message) { if (!condition) { console.error("FUNCTIONAL FAIL: " + message); Qt.exit(3); return false }; return true }
    Timer {
        interval: 400; repeat: true; running: root.functionalTest && appController.libraryReady && !appController.scanning
        onTriggered: {
            const directory = DesktopBridge.testDirectory()
            if (!root.checkTest(directory.length > 0, "isolated workspace required")) return
            switch (root.functionalStep++) {
                case 0:
                    if (!root.checkTest(appController.queueCount === 3 && !appController.playing, "paused session restore")) return
                    appController.seekTo(200); break
                case 1:
                    if (!root.checkTest(appController.positionMs === 200, "seek while restored")) return
                    appController.nextTrack(); break
                case 2:
                    if (!root.checkTest(appController.currentQueueIndex === 1 && !appController.playing, "next while paused")) return
                    appController.moveQueueTrack(1, 0); break
                case 3:
                    if (!root.checkTest(appController.currentQueueIndex === 0, "stable selection after move")) return
                    appController.removeQueueTrack(0); break
                case 4:
                    if (!root.checkTest(appController.queueCount === 2 && appController.currentTrackPath.indexOf("Track 1") >= 0, "remove restored current")) return
                    appController.requestPlaybackMode(2, true); appController.requestLyricsOffset(100)
                    appController.saveQueuePlaylist("QA playlist"); break
                case 5:
                    if (!root.checkTest(appController.playlistCount === 1 && appController.repeatMode === 2 && appController.shuffleEnabled, "playlist and modes")) return
                    appController.renamePlaylist(0, "QA renamed"); appController.exportQueue(directory + "/export.m3u8"); break
                case 6:
                    if (!root.checkTest(appController.playlistName(0).indexOf("QA renamed") >= 0, "rename playlist")) return
                    appController.importPlaylist(directory + "/export.m3u8"); break
                case 7:
                    if (!root.checkTest(appController.playlistCount === 2, "M3U8 import")) return
                    appController.deletePlaylist(1); appController.filterTracks("Track 1"); break
                case 8:
                    if (!root.checkTest(appController.playlistCount === 1 && appController.visibleTrackCount === 1, "search and delete")) return
                    root.persistUi(); console.info("Functional validation passed"); Qt.quit()
            }
        }
    }
    Timer { interval: 20000; running: root.functionalTest; onTriggered: { console.error("Functional validation timeout"); Qt.exit(4) } }

}
