pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Window
import QtQuick.Dialogs
import Qt.labs.platform as Platform
import io.github.dhkun.Liusheng 1.0

ApplicationWindow {
    id: root
    objectName: "mainWindow"
    width: 1280
    height: 800
    minimumWidth: 820
    minimumHeight: 560
    visible: true
    title: qsTr("留声")
    color: Theme.background
    font.family: Theme.fontFamily
    font.pixelSize: Theme.labelSize
    palette.window: Theme.background
    palette.base: Theme.surface
    palette.button: Theme.surface
    palette.text: Theme.text
    palette.windowText: Theme.text
    palette.buttonText: Theme.text
    palette.highlight: Theme.accentWash
    palette.highlightedText: Theme.text

    readonly property bool smokeTest: Application.arguments.indexOf("--smoke-test") >= 0
    readonly property bool outputSmokeTest: Application.arguments.indexOf("--output-smoke-test") >= 0
    readonly property bool startupBenchmark: Application.arguments.indexOf("--startup-benchmark") >= 0
    readonly property bool uiTest: Application.arguments.indexOf("--ui-test") >= 0
    readonly property bool functionalTest: Application.arguments.indexOf("--functional-test") >= 0
    readonly property var preferences: {
        try {
            return JSON.parse(appController.settingsJson);
        } catch (error) {
            return ({});
        }
    }
    readonly property bool compact: width < 1000
    readonly property bool queueOpened: queueLoader.item ? queueLoader.item.opened : false
    readonly property string activeError: appController.playbackError || appController.outputError
    property string dismissedError: ""
    property string activePage: "albums"
    property string librarySection: "albums"
    property bool firstFrameSeen: false
    property bool restoredUi: false
    property bool immersiveCreated: false
    property bool immersiveOpen: false
    readonly property bool windowDisplayed: visible && visibility !== Window.Minimized
    property real listeningProgress: immersiveOpen && immersiveLoader.status === Loader.Ready ? 1 : 0
    property alias coverTransition: coverFlight
    Behavior on listeningProgress {
        enabled: !Theme.reducedMotion && root.windowDisplayed
        NumberAnimation {
            duration: Theme.spatial
            easing.type: Easing.InOutCubic
        }
    }
    onWidthChanged: coverFlight.cancel()
    onHeightChanged: coverFlight.cancel()

    property bool settingsCreated: false
    property bool playlistsCreated: false
    property bool queueCreated: false
    property bool pendingPlaylistSave: false
    property string settingsTab: "library"
    property int outputSmokePhase: 0
    property alias libraryView: libraryPage
    property alias settingsView: settingsLoader
    property alias queueView: queueLoader
    property alias playerView: playerBar
    property alias outputView: outputPanel
    property alias trayMenuView: trayQuickMenu
    property alias playlistView: playlistLoader
    property alias audioFileView: audioFiles
    property alias immersiveView: immersiveLoader
    property alias updateView: updateLoader
    property alias updateService: updates
    property bool updateCreated: false
    property bool updateStartupDone: false
    UpdateService {
        id: updates
        automaticEnabled: root.preferences.check_updates_on_startup !== false
    }
    Timer {
        interval: 5000
        running: root.firstFrameSeen && appController.libraryReady && !root.updateStartupDone && !root.uiTest && !root.functionalTest && !root.smokeTest && !root.startupBenchmark && !root.outputSmokeTest
        onTriggered: {
            root.updateStartupDone = true;
            updates.startupCheck();
        }
    }
    function openUpdates(manual) {
        updates.initialize();
        if (manual)
            updates.check(true);
        updateCreated = true;
        if (updateLoader.item)
            updateLoader.item.open();
    }

    Binding {
        target: Theme
        property: "appearance"
        value: root.preferences.appearance || "system"
    }
    Binding {
        target: Theme
        property: "reducedMotion"
        value: root.preferences.reduced_motion === true
    }
    Binding {
        target: Theme
        property: "compactGrid"
        value: root.preferences.compact_grid !== false
    }
    Binding {
        target: Theme
        property: "coverTheme"
        value: root.preferences.cover_theme !== false
    }
    Binding {
        target: Theme
        property: "ambientMotion"
        value: root.preferences.ambient_motion !== false
    }
    Binding {
        target: Theme
        property: "lyricSecondary"
        value: root.preferences.lyric_secondary !== false
    }
    AppController {
        id: appController
    }
    PlaybackClock {
        id: playbackClock
        sourcePosition: appController.positionMs
        duration: appController.currentDurationMs
        playing: appController.playing
        displayed: root.windowDisplayed
    }

    function restoreFromTray() {
        show();
        if (!DesktopBridge.wayland)
            raise();
        requestActivate();
    }
    function showPage(page) {
        if (page === "queue") {
            toggleQueue();
            return;
        }
        immersiveOpen = false;
        if (queueLoader.item)
            queueLoader.item.close();
        appController.closeAlbum();
        appController.closeArtist();
        activePage = page;
        if (page === "playlists")
            playlistsCreated = true;
        else
            librarySection = page;
    }
    function openSettings(tab) {
        settingsTab = tab || "library";
        settingsCreated = true;
        if (settingsLoader.item)
            settingsLoader.item.open();
    }
    function toggleQueue() {
        if (!queueCreated) {
            queueCreated = true;
            return;
        }
        if (queueLoader.item) {
            if (queueLoader.item.opened)
                queueLoader.item.close();
            else
                queueLoader.item.open();
        }
    }
    function saveQueuePlaylist() {
        showPage("playlists");
        if (playlistLoader.item)
            playlistLoader.item.saveQueue();
        else
            pendingPlaylistSave = true;
    }
    function persistUi() {
        appController.saveUiState(JSON.stringify({
            page: activePage,
            width: width,
            height: height,
            album_scroll: libraryPage.albumScroll,
            track_scroll: libraryPage.trackScroll
        }));
    }
    function restoreUi() {
        if (restoredUi || !appController.libraryReady)
            return;
        restoredUi = true;
        if (preferences.restore_session === false)
            return;
        let saved = ({});
        try {
            saved = JSON.parse(appController.savedUiJson);
        } catch (error) {
            return;
        }
        if (saved.width >= minimumWidth)
            width = Math.max(minimumWidth, Math.min(saved.width, Screen.desktopAvailableWidth));
        if (saved.height >= minimumHeight)
            height = Math.max(minimumHeight, Math.min(saved.height, Screen.desktopAvailableHeight));
        if (["albums", "artists", "allTracks", "queue", "playlists"].indexOf(saved.page) >= 0)
            showPage(saved.page);
        Qt.callLater(function () {
            libraryPage.restoreScroll(saved.album_scroll || 0, saved.track_scroll || 0);
        });
    }
    onClosing: close => {
        persistUi();
        if (preferences.close_to_tray !== false && DesktopBridge.trayAvailable() && !smokeTest && !outputSmokeTest && !startupBenchmark && !uiTest && !functionalTest) {
            close.accepted = false;
            hide();
        }
    }
    Component.onCompleted: {
        if ((uiTest || functionalTest) && DesktopBridge.testDirectory().length === 0) {
            console.error("UI validation requires an isolated marked test workspace; use scripts/check-ui.py");
            Qt.callLater(function () {
                Qt.exit(2);
            });
            return;
        }
        if (outputSmokeTest) {
            outputSmokePhase = 1;
            appController.requestExclusiveOutput(true);
        } else if (smokeTest)
            Qt.callLater(root.close);
        else {
            appController.scanLibrary();
            const files = DesktopBridge.initialFiles();
            if (files !== "[]")
                appController.openFiles(files);
        }
    }
    onFrameSwapped: {
        if (!firstFrameSeen) {
            firstFrameSeen = true;
            DesktopBridge.profileMark("first_frame_ms");
            benchmarkExit.restart();
        }
    }
    function animateListeningEntry() {
        if (!immersiveOpen || !immersiveLoader.item)
            return;
        root.requestActivate();
        immersiveLoader.item.forceActiveFocus();
        if (immersiveLoader.item.showCover)
            coverFlight.fly(playerBar.coverItem, immersiveLoader.item.coverItem);
    }
    onImmersiveOpenChanged: {
        if (immersiveOpen) {
            immersiveCreated = true;
            if (queueLoader.item)
                queueLoader.item.close();
            outputPanel.close();
            Qt.callLater(root.animateListeningEntry);
        } else {
            if (immersiveLoader.item) {
                immersiveLoader.item.optionsMenu.close();
                immersiveLoader.item.layoutMenu.close();
                if (immersiveLoader.item.showCover)
                    coverFlight.fly(immersiveLoader.item.coverItem, playerBar.coverItem);
            }
            playerBar.forceActiveFocus();
        }
    }
    Connections {
        target: appController
        function onRaiseRequested() {
            root.restoreFromTray();
        }
        function onQuitRequested() {
            root.persistUi();
            Qt.quit();
        }
        function onLibraryReadyChanged() {
            root.restoreUi();
            benchmarkExit.restart();
        }
        function onQueueNoticeRevisionChanged() {
            noticeTimer.restart();
        }
        function onOutputSwitchingChanged() {
            if (!root.outputSmokeTest || appController.outputSwitching)
                return;
            if (root.outputSmokePhase === 1 && appController.exclusiveOutput) {
                root.outputSmokePhase = 2;
                appController.requestExclusiveOutput(false);
            } else if (root.outputSmokePhase === 2 && !appController.exclusiveOutput) {
                console.info("output smoke test passed");
                Qt.quit();
            } else {
                console.error("output smoke test failed: " + appController.outputError);
                Qt.exit(1);
            }
        }
    }
    Connections {
        target: DesktopBridge
        function onOpenRequested(pathsJson) {
            appController.openFiles(pathsJson);
        }
        function onRaiseRequested() {
            root.restoreFromTray();
        }
    }
    Timer {
        interval: 5000
        repeat: true
        running: appController.libraryReady
        onTriggered: root.persistUi()
    }
    Timer {
        id: benchmarkExit
        interval: 10
        onTriggered: {
            if (root.startupBenchmark && root.firstFrameSeen && appController.libraryReady) {
                DesktopBridge.profileMark("interactive_ms");
                Qt.quit();
            }
        }
    }
    Timer {
        interval: root.uiTest ? 60000 : 20000
        running: root.startupBenchmark || root.outputSmokeTest || root.uiTest || root.functionalTest
        onTriggered: {
            console.error("UI validation timeout");
            Qt.exit(2);
        }
    }

    Platform.SystemTrayIcon {
        id: trayIcon
        visible: available
        // The approved brand is supplied as pixels. A theme name takes precedence
        // over icon.source and can substitute an unrelated or older desktop icon.
        icon.name: ""
        icon.mask: true
        icon.source: Application.styleHints.colorScheme === Qt.Light ? "qrc:/qt/qml/io/github/dhkun/Liusheng/qml/assets/app-icon/tray-light.svg" : "qrc:/qt/qml/io/github/dhkun/Liusheng/qml/assets/app-icon/tray-dark.svg"
        tooltip: appController.hasCurrentTrack ? qsTr("留声 · %1").arg(appController.currentTitle) : qsTr("留声")
        onActivated: reason => {
            if (reason === Platform.SystemTrayIcon.Context && DesktopBridge.wayland)
                root.showTrayMenu();
            else if (reason === Platform.SystemTrayIcon.Trigger || reason === Platform.SystemTrayIcon.DoubleClick)
                root.restoreFromTray();
        }
        menu: DesktopBridge.wayland ? null : root.nativeTrayMenu
    }
    Timer {
        interval: 3000
        repeat: true
        running: !root.visible && root.firstFrameSeen
        onTriggered: {
            if (!DesktopBridge.trayAvailable())
                root.show();
        }
    }
    property var nativeTrayMenu: DesktopBridge.wayland ? null : nativeTrayMenuFactory.createObject(trayIcon)
    Component {
        id: nativeTrayMenuFactory
        Platform.Menu {
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
                onTriggered: {
                    root.persistUi();
                    Qt.quit();
                }
            }
        }
    }
    QuietMenu {
        id: trayQuickMenu
        objectName: "trayQuickMenu"
        QuietMenuItem {
            text: qsTr("显示留声")
            onTriggered: root.restoreFromTray()
        }
        QuietMenuItem {
            text: qsTr("退出")
            onTriggered: {
                root.persistUi();
                Qt.quit();
            }
        }
    }
    function showTrayMenu() {
        restoreFromTray();
        Qt.callLater(function () {
            trayQuickMenu.openBelow(settingsNavigation);
        });
    }
    FileDialog {
        id: audioFiles
        parentWindow: root
        title: qsTr("打开本地音乐")
        fileMode: FileDialog.OpenFiles
        nameFilters: [qsTr("音乐与歌单 (*.flac *.mp3 *.m4a *.ogg *.wav *.aiff *.aif *.m3u8 *.m3u)"), qsTr("所有文件 (*)")]
        onAccepted: appController.openFiles(JSON.stringify(selectedFiles.map(url => url.toString())))
    }
    Shortcut {
        sequence: "Escape"
        enabled: root.immersiveOpen && !root.queueOpened && !outputPanel.opened && !(settingsLoader.item && settingsLoader.item.opened) && !(updateLoader.item && updateLoader.item.opened) && !audioFiles.visible && !trayQuickMenu.opened && immersiveLoader.item && !immersiveLoader.item.optionsMenu.opened && !immersiveLoader.item.layoutMenu.opened
        onActivated: root.immersiveOpen = false
    }
    Shortcut {
        sequence: "Ctrl+Q"
        onActivated: {
            root.persistUi();
            Qt.quit();
        }
    }
    Shortcut {
        sequence: "Ctrl+O"
        onActivated: audioFiles.open()
    }
    Shortcut {
        sequence: "Ctrl+,"
        onActivated: root.openSettings()
    }
    Shortcut {
        sequence: "Ctrl+F"
        onActivated: {
            root.immersiveOpen = false;
            if (queueLoader.item)
                queueLoader.item.close();
            outputPanel.close();
            if (root.activePage === "playlists")
                root.showPage("allTracks");
            libraryPage.focusSearch();
        }
    }
    Shortcut {
        sequence: "Ctrl+J"
        onActivated: root.toggleQueue()
    }
    Shortcut {
        sequence: "Ctrl+L"
        onActivated: root.immersiveOpen = !root.immersiveOpen
    }
    function focusConsumesSpace() {
        let item = activeFocusItem;
        while (item && item !== root.contentItem) {
            if ((item.hasOwnProperty("text") && item.hasOwnProperty("readOnly")) || typeof item.clicked === "function" || (typeof item.activated === "function" && item.hasOwnProperty("popup")))
                return true;
            item = item.parent;
        }
        return false;
    }
    Shortcut {
        sequence: "Space"
        enabled: appController.hasCurrentTrack && !root.focusConsumesSpace() && !(settingsLoader.item && settingsLoader.item.opened) && !(updateLoader.item && updateLoader.item.opened) && !outputPanel.opened
        onActivated: appController.togglePlayback()
    }
    Shortcut {
        sequence: "Alt+Left"
        enabled: appController.albumOpen || appController.artistOpen
        onActivated: {
            appController.closeAlbum();
            appController.closeArtist();
        }
    }

    Rectangle {
        id: sidebar
        objectName: "navigationSidebar"
        width: root.compact ? 72 : 200
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.bottom: playerBar.top
        color: Theme.sidebar
        visible: root.listeningProgress < 1
        enabled: !root.immersiveOpen
        Rectangle {
            anchors.right: parent.right
            width: 1
            height: parent.height
            color: Theme.line
            opacity: 0.6
        }
        RowLayout {
            x: root.compact ? 21 : 23
            y: 30
            spacing: 12
            Icon {
                name: "brand"
                size: 26
                color: Theme.text
            }
            Text {
                text: qsTr("留声")
                visible: !root.compact
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: Theme.headingSize
                font.weight: Font.DemiBold
                font.letterSpacing: 3
            }
        }
        ColumnLayout {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.topMargin: 106
            spacing: 5
            Text {
                text: qsTr("资料库")
                visible: !root.compact
                color: Theme.muted
                font.pixelSize: 10
                Layout.leftMargin: 12
                Layout.bottomMargin: 10
            }
            Repeater {
                model: [
                    {
                        page: "albums",
                        label: qsTr("专辑"),
                        icon: "album"
                    },
                    {
                        page: "artists",
                        label: qsTr("艺术家"),
                        icon: "artist"
                    },
                    {
                        page: "allTracks",
                        label: qsTr("歌曲"),
                        icon: "music"
                    },
                    {
                        page: "playlists",
                        label: qsTr("歌单"),
                        icon: "playlist"
                    }
                ]
                delegate: NavigationItem {
                    required property var modelData
                    objectName: "nav_" + modelData.page
                    iconOnly: root.compact
                    selected: root.activePage === modelData.page
                    hint: modelData.label
                    glyph: modelData.icon
                    text: modelData.label
                    onClicked: root.showPage(modelData.page)
                }
            }
        }
        ColumnLayout {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.bottomMargin: 22
            spacing: 10
            NavigationItem {
                id: settingsNavigation
                objectName: "settingsNavigation"
                iconOnly: root.compact
                glyph: "settings"
                text: qsTr("设置")
                hint: qsTr("设置 · Ctrl+,")
                Layout.fillWidth: true
                onClicked: root.openSettings()
            }
            Text {
                visible: !root.compact
                text: appController.scanErrors.length > 0 ? qsTr("部分目录需要检查") : appController.scanning ? qsTr("正在更新曲库…") : qsTr("音乐，留在身边。")
                color: appController.scanErrors.length > 0 ? Theme.danger : Theme.muted
                font.pixelSize: 10
                Layout.leftMargin: 10
                Layout.fillWidth: true
                elide: Text.ElideRight
                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: root.openSettings()
                    ToolTip.visible: containsMouse
                    ToolTip.text: appController.status
                }
            }
        }
    }
    UpdateNotice {
        id: updateNotice
        updater: updates
        visible: updates.notifyAvailable && root.windowDisplayed && !root.immersiveOpen
        anchors.left: sidebar.right
        anchors.right: parent.right
        anchors.top: parent.top
        height: 52
        onDetailsRequested: root.openUpdates(false)
    }
    Item {
        id: content
        anchors.left: sidebar.right
        anchors.right: parent.right
        anchors.top: updateNotice.visible ? updateNotice.bottom : parent.top
        anchors.bottom: errorBanner.visible ? errorBanner.top : playerBar.top
        anchors.margins: root.compact ? 24 : 32
        anchors.bottomMargin: 20
        visible: root.listeningProgress < 1
        enabled: !root.immersiveOpen
        LibraryPage {
            id: libraryPage
            anchors.fill: parent
            controller: appController
            section: root.librarySection
            visible: root.activePage !== "playlists"
            onSettingsRequested: root.openSettings()
            onFilesRequested: audioFiles.open()
            onCheckUpdatesRequested: root.openUpdates(true)
            onQuitRequested: {
                root.persistUi();
                Qt.quit();
            }
        }
        Loader {
            id: playlistLoader
            anchors.fill: parent
            active: root.playlistsCreated
            visible: root.activePage === "playlists"
            asynchronous: true
            sourceComponent: Component {
                PlaylistsPage {
                    controller: appController
                }
            }
            onLoaded: {
                if (root.pendingPlaylistSave) {
                    item.saveQueue();
                    root.pendingPlaylistSave = false;
                }
            }
        }
    }
    Rectangle {
        id: errorBanner
        visible: root.activeError.length > 0 && root.activeError !== root.dismissedError
        height: 42
        anchors.left: sidebar.right
        anchors.right: parent.right
        anchors.bottom: playerBar.top
        color: Theme.accentWash
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 18
            anchors.rightMargin: 12
            Text {
                text: root.activeError
                color: Theme.danger
                font.pixelSize: Theme.captionSize
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            QuietButton {
                text: qsTr("查看输出")
                compact: true
                onClicked: outputPanel.open()
            }
            QuietButton {
                glyph: "close"
                compact: true
                hint: qsTr("收起提示")
                onClicked: root.dismissedError = root.activeError
            }
        }
    }
    PlayerBar {
        id: playerBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 96
        controller: appController
        positionMs: playbackClock.position
        queueOpen: root.queueOpened
        immersiveOpen: root.immersiveOpen
        coverHidden: coverFlight.running
        onQueueRequested: root.toggleQueue()
        onImmersiveRequested: root.immersiveOpen = !root.immersiveOpen
        onOutputRequested: outputPanel.open()
    }
    OutputPopover {
        id: outputPanel
        parent: Overlay.overlay
        controller: appController
        x: Math.max(16, root.width - width - 20)
        y: Math.max(12, root.height - playerBar.height - height - 12)
        onSettingsRequested: root.openSettings("playback")
    }
    Loader {
        id: updateLoader
        active: root.updateCreated
        asynchronous: true
        sourceComponent: Component {
            UpdateDialog {
                updater: updates
                parent: Overlay.overlay
                anchors.centerIn: parent
            }
        }
        onLoaded: item.open()
    }
    Loader {
        id: settingsLoader
        active: root.settingsCreated
        asynchronous: true
        sourceComponent: Component {
            SettingsDialog {
                controller: appController
                updater: updates
                onCheckUpdatesRequested: root.openUpdates(true)
                initialTab: root.settingsTab
                parent: Overlay.overlay
                anchors.centerIn: parent
            }
        }
        onLoaded: item.open()
    }
    Loader {
        id: queueLoader
        active: root.queueCreated
        asynchronous: true
        sourceComponent: Component {
            QueuePanel {
                controller: appController
                parent: Overlay.overlay
                height: root.height - playerBar.height
                onSaveRequested: root.saveQueuePlaylist()
            }
        }
        onLoaded: item.open()
    }
    Loader {
        id: immersiveLoader
        focus: root.immersiveOpen
        anchors.fill: parent
        anchors.bottomMargin: playerBar.height
        active: root.immersiveCreated
        visible: root.immersiveOpen || root.listeningProgress > 0
        enabled: root.immersiveOpen
        opacity: root.listeningProgress
        asynchronous: true
        z: 20
        sourceComponent: Component {
            ImmersivePlayer {
                controller: appController
                positionMs: playbackClock.position
                displayed: root.immersiveOpen && root.windowDisplayed
                windowActive: root.active
                coverHidden: coverFlight.running
                onCloseRequested: root.immersiveOpen = false
            }
        }
        onLoaded: Qt.callLater(root.animateListeningEntry)
    }
    CoverFlight {
        id: coverFlight
        source: appController.currentCoverUrl
        title: appController.currentTitle
        allowed: root.windowDisplayed
    }
    Rectangle {
        visible: noticeTimer.running
        z: 40
        width: Math.min(root.width - 40, noticeText.implicitWidth + 36)
        height: 40
        radius: 8
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: playerBar.top
        anchors.bottomMargin: 16
        color: Theme.text
        Text {
            id: noticeText
            anchors.centerIn: parent
            text: appController.queueNotice
            color: Theme.background
            font.pixelSize: Theme.captionSize
        }
        Timer {
            id: noticeTimer
            interval: 2200
        }
    }
    DropArea {
        anchors.fill: parent
        onDropped: drop => {
            if (drop.hasUrls) {
                appController.openFiles(JSON.stringify(drop.urls.map(url => url.toString())));
                drop.acceptProposedAction();
            }
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: 8
            visible: parent.containsDrag
            color: Theme.accentWash
            opacity: 0.94
            border.width: 2
            border.color: Theme.accent
            radius: 10
            z: 100
            Text {
                anchors.centerIn: parent
                text: qsTr("松开，播放这些音乐")
                color: Theme.text
                font.pixelSize: 24
            }
        }
    }
    Loader {
        active: root.uiTest || root.functionalTest
        sourceComponent: Component {
            ValidationHarness {
                shell: root
                controller: appController
            }
        }
    }
}
