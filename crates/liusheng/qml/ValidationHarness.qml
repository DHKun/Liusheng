pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Window
import QtQuick.Controls.Basic
import io.github.dhkun.Liusheng 1.0

Item {
    id: harness
    required property var shell
    required property var controller
    Component.onCompleted: console.info("[platform] " + DesktopBridge.platformName())
    property string originalSettings: ""
    property var capturedShots: ({})
    property bool prepared: false
    function clickNamed(name) {
        const item = DesktopBridge.testFind(shell, name);
        return checkTest(item && DesktopBridge.testClick(item), "click " + name);
    }
    function sendEscape() {
        DesktopBridge.testKey(shell, Qt.Key_Escape, 0);
    }
    function bounded(popup, label) {
        console.info("[popup] " + label + " " + JSON.stringify({
            opened: popup.opened,
            type: popup.popupType,
            x: popup.x,
            y: popup.y,
            width: popup.width,
            height: popup.height,
            windowWidth: shell.width,
            windowHeight: shell.height
        }));
        return checkTest(popup && popup.opened && popup.popupType === Popup.Item && popup.x >= -1 && popup.y >= -1 && popup.x + popup.width <= shell.width + 1 && popup.y + popup.height <= shell.height + 1, label + " opens inside parent scene");
    }
    property int step: 0
    property string shot: "00-albums-light"
    Timer {
        interval: 500
        repeat: true
        running: harness.shell.uiTest && harness.controller.libraryReady && !harness.controller.scanning
        onTriggered: {
            if (!harness.prepared) {
                harness.shell.width = 1280;
                harness.shell.height = 800;
                harness.prepared = true;
                return;
            }
            if (DesktopBridge.wayland && !harness.checkTest(DesktopBridge.visibleNativePopups() === 0, "no native grabbing popup on Wayland"))
                return;
            if (!harness.capturedShots[harness.shot]) {
                DesktopBridge.captureForTest(harness.shell, harness.shot);
                harness.capturedShots[harness.shot] = true;
            }
            switch (harness.step++) {
            case 0:
                harness.shell.showPage("artists");
                harness.shot = "01-artists";
                break;
            case 1:
                harness.shell.showPage("allTracks");
                harness.shot = "02-songs";
                break;
            case 2:
                harness.shell.showPage("queue");
                harness.shot = "03-queue";
                break;
            case 3:
                harness.shell.showPage("playlists");
                harness.shot = "04-playlists";
                break;
            case 4:
                harness.shell.openSettings("library");
                harness.shot = "05-settings-library";
                break;
            case 5:
                if (harness.shell.settingsView.item)
                    harness.shell.settingsView.item.tab = "appearance";
                harness.shot = "06-settings-appearance";
                break;
            case 6:
                if (harness.shell.settingsView.item)
                    harness.shell.settingsView.item.close();
                harness.shell.immersiveOpen = true;
                harness.shot = "07-now-playing";
                break;
            case 7:
                if (!harness.checkTest(DesktopBridge.testFind(harness.shell.immersiveView.item, "nowPlayingLyricsPane").width >= 300, "wide lyrics retain a visible column"))
                    return;
                harness.shell.immersiveOpen = false;
                harness.shell.showPage("albums");
                Theme.previewAppearance = "dark";
                harness.shot = "08-albums-dark";
                break;
            case 8:
                if (harness.controller.albumCount > 0)
                    harness.controller.openAlbum(0);
                harness.shot = "09-album-detail-dark";
                break;
            case 9:
                harness.shell.width = 820;
                harness.shell.height = 560;
                harness.shot = "10-compact-detail";
                break;
            case 10:
                harness.shell.showPage("allTracks");
                harness.shot = "11-compact-songs";
                break;
            case 11:
                harness.shell.openSettings("playback");
                harness.shot = "12-compact-settings";
                break;
            case 12:
                if (harness.shell.settingsView.item)
                    harness.shell.settingsView.item.close();
                harness.shell.immersiveOpen = true;
                harness.shot = "13-compact-now-playing";
                break;
            case 13:
                harness.shell.immersiveOpen = false;
                harness.shell.showPage("albums");
                harness.shell.width = 1280;
                harness.shell.height = 800;
                Theme.previewAppearance = "light";
                harness.shell.libraryView.searchFor("no-such-album-qa");
                harness.shot = "14-search-empty";
                break;
            case 14:
                if (!harness.checkTest(harness.shell.libraryView.resultCount === 0, "album search refresh"))
                    return;
                harness.shell.libraryView.searchFor("");
                harness.shot = "15-search-cleared";
                break;
            case 15:
                if (!harness.checkTest(harness.shell.libraryView.resultCount === harness.controller.albumCount, "album search clear"))
                    return;
                harness.shell.showPage("artists");
                harness.shell.libraryView.searchFor("no-such-artist-qa");
                break;
            case 16:
                if (!harness.checkTest(harness.shell.libraryView.resultCount === 0, "artist search refresh"))
                    return;
                harness.shell.libraryView.searchFor("");
                harness.shell.outputView.open();
                harness.shot = "16-output";
                break;
            case 17:
                if (!harness.checkTest(harness.shell.libraryView.resultCount === harness.controller.artistCount, "artist search clear"))
                    return;
                harness.shell.outputView.close();
                break;
            case 18:
                harness.shell.showPage("albums");
                if (!harness.clickNamed("libraryMore"))
                    return;
                harness.shot = "18-library-menu";
                break;
            case 19:
                if (!harness.bounded(harness.shell.libraryView.moreMenu, "library menu"))
                    return;
                harness.sendEscape();
                break;
            case 20:
                if (!harness.checkTest(!harness.shell.libraryView.moreMenu.opened, "menu closes with Escape"))
                    return;
                harness.shell.showPage("allTracks");
                break;
            case 21:
                if (!harness.clickNamed("filterButton"))
                    return;
                harness.shot = "19-filter";
                break;
            case 22:
                if (!harness.bounded(harness.shell.libraryView.filtersPopup, "filters"))
                    return;
                DesktopBridge.testClick(harness.shell.libraryView.sortControl);
                harness.shot = "20-sort-options";
                break;
            case 23:
                if (!harness.checkTest(harness.shell.libraryView.sortControl.popup.opened, "nested sort choices"))
                    return;
                harness.sendEscape();
                break;
            case 24:
                if (!harness.checkTest(!harness.shell.libraryView.sortControl.popup.opened && harness.shell.libraryView.filtersPopup.opened, "Escape closes child popup first"))
                    return;
                harness.sendEscape();
                break;
            case 25:
                if (!harness.checkTest(!harness.shell.libraryView.filtersPopup.opened, "filter closes with Escape"))
                    return;
                const table = harness.shell.libraryView.songView.item;
                if (table && table.count > 0) {
                    table.listView.currentIndex = 0;
                    DesktopBridge.testClick(table.listView.itemAtIndex(0), 2);
                }
                harness.shot = "21-track-context-menu";
                break;
            case 26:
                const row = harness.shell.libraryView.songView.item.listView.itemAtIndex(0);
                if (row && !harness.bounded(row.actionMenu, "track context menu"))
                    return;
                harness.sendEscape();
                break;
            case 27:
                harness.shell.showPage("playlists");
                break;
            case 28:
                harness.shell.playlistView.item.saveQueue();
                harness.shot = "22-playlist-name";
                break;
            case 29:
                if (!harness.checkTest(harness.shell.playlistView.item.nameField.height >= 38, "playlist name keeps a visible input field"))
                    return;
                if (!harness.bounded(harness.shell.playlistView.item.nameView, "playlist name"))
                    return;
                if (!harness.checkTest(!harness.shell.playlistView.item.nameView.acceptEnabled, "empty playlist name disables save"))
                    return;
                harness.sendEscape();
                break;
            case 30:
                if (!harness.checkTest(!harness.shell.playlistView.item.nameView.opened, "name cancel"))
                    return;
                harness.shell.playlistView.item.deleteView.open();
                harness.shot = "23-playlist-delete-confirm";
                break;
            case 31:
                if (!harness.bounded(harness.shell.playlistView.item.deleteView, "delete confirmation"))
                    return;
                harness.sendEscape();
                break;
            case 32:
                harness.originalSettings = harness.controller.settingsJson;
                harness.shell.openSettings("library");
                break;
            case 33:
                harness.shell.settingsView.item.rootsField.text = "relative/path";
                harness.shot = "24-invalid-directory";
                break;
            case 34:
                if (!harness.checkTest(!harness.shell.settingsView.item.validDraft, "relative directory rejected"))
                    return;
                harness.sendEscape();
                break;
            case 35:
                if (!harness.checkTest(harness.controller.settingsJson === harness.originalSettings, "cancel preserves preferences"))
                    return;
                harness.shell.showPage("albums");
                harness.shell.openSettings("appearance");
                break;
            case 36:
                DesktopBridge.testClick(harness.shell.settingsView.item.densityControl);
                harness.shot = "25-density-options";
                break;
            case 37:
                if (!harness.checkTest(harness.shell.settingsView.item.densityControl.popup.opened, "density choices"))
                    return;
                harness.sendEscape();
                break;
            case 38:
                harness.sendEscape();
                break;
            case 39:
                harness.shell.immersiveOpen = true;
                break;
            case 40:
                harness.sendEscape();
                break;
            case 41:
                if (!harness.checkTest(!harness.shell.immersiveOpen, "reused now-playing returns focus and closes"))
                    return;
                harness.shell.showTrayMenu();
                harness.shot = "26-wayland-tray-fallback";
                break;
            case 42:
                if (!harness.bounded(harness.shell.trayMenuView, "tray fallback"))
                    return;
                if (!harness.checkTest(harness.shell.nativeTrayMenu !== null && DesktopBridge.visibleNativePopups() === 0, "tray menu is attached without creating a native popup"))
                    return;
                harness.sendEscape();
                break;
            case 43:
                harness.shell.audioFileView.open();
                break;
            case 44:
                if (!harness.checkTest(harness.shell.audioFileView.visible && harness.shell.audioFileView.parentWindow === harness.shell, "file dialog transient parent"))
                    return;
                harness.shell.audioFileView.close();
                break;
            case 45:
                harness.shell.openSettings("library");
                break;
            case 46:
                harness.shell.settingsView.item.folderView.open();
                break;
            case 47:
                if (!harness.checkTest(harness.shell.settingsView.item.folderView.visible && harness.shell.settingsView.item.folderView.parentWindow === harness.shell, "folder dialog transient parent"))
                    return;
                harness.shell.settingsView.item.folderView.close();
                break;
            case 48:
                harness.shell.settingsView.item.close();
                harness.shell.showPage("albums");
                Theme.compactGrid = false;
                harness.shot = "27-comfortable-density";
                break;
            case 49:
                const grid = harness.shell.libraryView.albumView;
                if (!harness.checkTest(Math.floor(grid.width / grid.cellWidth) === grid.columns, "computed grid columns fit actual view"))
                    return;
                Theme.compactGrid = true;
                harness.shot = "28-compact-density";
                break;
            case 50:
                harness.shell.width = 1280;
                harness.shell.height = 800;
                Theme.previewAppearance = "light";
                harness.shell.immersiveOpen = true;
                harness.shot = "29-listening-light";
                break;
            case 51:
                harness.clickNamed("listeningLayoutButton");
                break;
            case 52:
                if (!harness.bounded(harness.shell.immersiveView.item.layoutMenu, "listening layout menu"))
                    return;
                harness.sendEscape();
                harness.shell.immersiveView.item.layoutMode = "lyrics";
                harness.shot = "30-listening-lyrics";
                break;
            case 53:
                if (!harness.checkTest(harness.shell.immersiveOpen && harness.shell.immersiveView.item.showLyrics, "layout popup escape preserves page"))
                    return;
                Theme.previewAppearance = "dark";
                harness.shot = "31-listening-dark";
                break;
            case 54:
                harness.shell.immersiveView.item.layoutMode = "cover";
                harness.shot = "32-listening-cover";
                break;
            case 55:
                harness.shell.immersiveOpen = false;
                harness.shell.immersiveOpen = true;
                harness.shell.width = 820;
                harness.shell.height = 560;
                harness.shell.immersiveView.item.layoutMode = "split";
                harness.shell.immersiveView.item.lyricsOnly = true;
                harness.shot = "33-listening-compact";
                break;
            case 56:
                console.info("[listening-focus] " + JSON.stringify({
                    open: harness.shell.immersiveOpen,
                    active: harness.shell.active,
                    itemFocus: harness.shell.immersiveView.item.activeFocus,
                    loaderFocus: harness.shell.immersiveView.focus,
                    loaderEnabled: harness.shell.immersiveView.enabled,
                    itemEnabled: harness.shell.immersiveView.item.enabled,
                    itemVisible: harness.shell.immersiveView.item.visible,
                    loaderVisible: harness.shell.immersiveView.visible,
                    progress: harness.shell.listeningProgress,
                    focusedItem: String(harness.shell.activeFocusItem),
                    settings: harness.shell.settingsView.item.opened,
                    file: harness.shell.audioFileView.visible,
                    queue: harness.shell.queueOpened,
                    output: harness.shell.outputView.opened,
                    layouts: harness.shell.immersiveView.item.layoutMenu.opened,
                    options: harness.shell.immersiveView.item.optionsMenu.opened
                }));
                if (!harness.checkTest(!harness.shell.coverTransition.running && harness.shell.immersiveView.item.showLyrics, "resize cancels cover flight and keeps lyrics"))
                    return;
                harness.sendEscape();
                break;
            case 57:
                console.info("[listening-closed] " + JSON.stringify({
                    open: harness.shell.immersiveOpen,
                    active: harness.shell.active,
                    motion: harness.shell.immersiveView.item.ambientRunning
                }));
                if (!harness.checkTest(!harness.shell.immersiveOpen && !harness.shell.immersiveView.item.ambientRunning, "closed listening page stops background work"))
                    return;
                break;
            case 58:
                harness.shell.width = 1280;
                harness.shell.height = 800;
                Theme.previewAppearance = "light";
                if (!harness.checkTest(harness.shell.updateService.secureTransportAvailable(), "TLS provider is packaged"))
                    return;
                harness.shell.openSettings("about");
                harness.shot = "34-about-updates";
                break;
            case 59:
                harness.clickNamed("checkUpdatesButton");
                harness.shot = "35-update-dialog";
                break;
            case 60:
                if (!harness.bounded(harness.shell.updateView.item, "manual update dialog"))
                    return;
                if (!harness.checkTest(harness.shell.updateService.status === "error" && !harness.shell.updateService.busy, "test session makes no public update request"))
                    return;
                harness.sendEscape();
                break;
            case 61:
                if (!harness.checkTest(!harness.shell.updateView.item.opened && harness.shell.settingsView.item.opened, "update Escape preserves settings parent"))
                    return;
                harness.clickNamed("automaticUpdateChoice");
                harness.sendEscape();
                break;
            case 62:
                if (!harness.checkTest(JSON.parse(harness.controller.settingsJson).check_updates_on_startup === true, "cancel preserves startup update preference"))
                    return;
                harness.shell.width = 820;
                harness.shell.height = 560;
                Theme.previewAppearance = "dark";
                harness.shell.openUpdates(true);
                harness.shot = "36-update-compact";
                break;
            case 63:
                if (!harness.bounded(harness.shell.updateView.item, "compact update dialog"))
                    return;
                harness.sendEscape();
                break;
            case 64:
                harness.shell.openSettings("about");
                break;
            case 65:
                harness.clickNamed("automaticUpdateChoice");
                harness.shell.settingsView.item.accept();
                break;
            case 66:
                if (!harness.checkTest(JSON.parse(harness.controller.settingsJson).check_updates_on_startup === false && !harness.shell.updateService.automaticEnabled, "saved opt-out reaches update service"))
                    return;
                const resetUpdates = JSON.parse(harness.controller.settingsJson);
                resetUpdates.check_updates_on_startup = true;
                harness.controller.applySettings(JSON.stringify(resetUpdates));
                break;
            case 67:
                if (!harness.checkTest(harness.shell.updateService.automaticEnabled, "startup check re-enabled"))
                    return;
                break;
            case 68:
                harness.shell.width = 1280;
                harness.shell.height = 800;
                Theme.previewAppearance = "light";
                harness.controller.requestOnlineDetails("tracks", 0, "lyrics");
                harness.shot = "37-online-lyrics";
                break;
            case 69:
                if (!harness.bounded(harness.shell.onlineView.item, "online lyrics"))
                    return;
                harness.clickNamed("onlineSearch");
                break;
            case 70:
                if (!harness.checkTest(harness.shell.onlineService.state.status === "error" && !harness.shell.onlineService.state.busy, "test mode blocks online music requests"))
                    return;
                harness.sendEscape();
                break;
            case 71:
                if (!harness.checkTest(!harness.shell.onlineView.item.opened, "online dialog Escape"))
                    return;
                harness.shell.width = 820;
                harness.shell.height = 560;
                Theme.previewAppearance = "dark";
                harness.controller.requestOnlineDetails("tracks", 0, "cover");
                harness.shot = "38-online-cover-compact";
                break;
            case 72:
                if (!harness.bounded(harness.shell.onlineView.item, "online cover compact"))
                    return;
                if (!harness.checkTest(!DesktopBridge.testFind(harness.shell, "onlineAlbumScope").visible, "unknown album has track-only override"))
                    return;
                harness.shell.onlineService.importLocal("file://" + DesktopBridge.testDirectory() + "/music/cover.png", false);
                break;
            case 73:
                if (!harness.checkTest(harness.shell.onlineService.state.status === "saved", "online cover storage shares local import pipeline"))
                    return;
                harness.sendEscape();
                harness.shell.openOnlineBatch();
                harness.shot = "39-online-batch";
                break;
            case 74:
                if (!harness.bounded(harness.shell.onlineBatchView.item, "online batch compact"))
                    return;
                harness.sendEscape();
                harness.shell.openSettings("library");
                break;
            case 75:
                harness.shell.settingsView.item.pageScroll.contentItem.contentY = Math.max(0, harness.shell.settingsView.item.pageScroll.contentItem.contentHeight - harness.shell.settingsView.item.pageScroll.contentItem.height);
                harness.clickNamed("automaticOnlineCovers");
                harness.clickNamed("automaticOnlineLyrics");
                harness.clickNamed("onlineExtraSourcesChoice");
                harness.sendEscape();
                break;
            case 76:
                const afterCancel = JSON.parse(harness.controller.settingsJson);
                if (!harness.checkTest(afterCancel.online_covers === false && afterCancel.online_lyrics === false && afterCancel.online_extra_sources === false, "cancel preserves online opt-in preferences"))
                    return;
                harness.shell.openSettings("library");
                break;
            case 77:
                harness.shell.settingsView.item.pageScroll.contentItem.contentY = Math.max(0, harness.shell.settingsView.item.pageScroll.contentItem.contentHeight - harness.shell.settingsView.item.pageScroll.contentItem.height);
                harness.clickNamed("automaticOnlineLyrics");
                if (!harness.checkTest(DesktopBridge.testFind(harness.shell, "automaticOnlineLyrics").checked, "online preference check toggles"))
                    return;
                harness.clickNamed("onlineExtraSourcesChoice");
                harness.shell.settingsView.item.accept();
                break;
            case 78:
                if (!harness.checkTest(JSON.parse(harness.controller.settingsJson).online_lyrics === true && harness.shell.onlineService.autoLyrics, "saved online permission reaches service"))
                    return;
                const privacyReset = JSON.parse(harness.controller.settingsJson);
                if (!harness.checkTest(JSON.parse(harness.controller.settingsJson).online_extra_sources === true && harness.shell.onlineService.autoExtraSources, "saved extra-source permission reaches service"))
                    return;
                privacyReset.online_lyrics = false;
                privacyReset.online_extra_sources = false;
                harness.controller.applySettings(JSON.stringify(privacyReset));
                break;
            case 79:
                harness.controller.requestOnlineDetails("tracks", 0, "lyrics");
                break;
            case 80:
                harness.clickNamed("onlineSourceInfoButton");
                harness.shot = "42-source-details";
                break;
            case 81:
                if (!harness.bounded(DesktopBridge.testFind(harness.shell.onlineView.item, "onlineSourceInfo"), "source and privacy details"))
                    return;
                harness.sendEscape();
                break;
            case 82:
                if (!harness.checkTest(harness.shell.onlineView.item.opened && !DesktopBridge.testFind(harness.shell.onlineView.item, "onlineSourceInfo").opened, "source details Escape preserves search parent"))
                    return;
                harness.sendEscape();
                break;
            default:
                if (!harness.checkTest(harness.shell.playerView.height >= 80 && harness.shell.libraryView.width >= 650, "responsive minimum size"))
                    return;
                Theme.previewAppearance = "";
                console.info("UI validation passed");
                Qt.quit();
            }
        }
    }
    property string pinnedTestCover: ""
    property int functionalStep: 0
    function checkTest(condition, message) {
        if (!condition) {
            console.error("FUNCTIONAL FAIL: " + message);
            Qt.exit(3);
            return false;
        }
        ;
        return true;
    }
    Timer {
        interval: 400
        repeat: true
        running: harness.shell.functionalTest && harness.controller.libraryReady && !harness.controller.scanning
        onTriggered: {
            const directory = DesktopBridge.testDirectory();
            if (!harness.checkTest(directory.length > 0, "isolated workspace required"))
                return;
            switch (harness.functionalStep++) {
            case 0:
                if (!harness.checkTest(harness.controller.queueCount === 3 && !harness.controller.playing, "paused session restore"))
                    return;
                harness.controller.seekTo(200);
                break;
            case 1:
                if (!harness.checkTest(harness.controller.positionMs === 200, "seek while restored"))
                    return;
                harness.controller.nextTrack();
                break;
            case 2:
                if (!harness.checkTest(harness.controller.currentQueueIndex === 1 && !harness.controller.playing, "next while paused"))
                    return;
                harness.controller.moveQueueTrack(1, 0);
                break;
            case 3:
                if (!harness.checkTest(harness.controller.currentQueueIndex === 0, "stable selection after move"))
                    return;
                harness.controller.removeQueueTrack(0);
                break;
            case 4:
                if (!harness.checkTest(harness.controller.queueCount === 2 && harness.controller.currentTrackPath.indexOf("Track 1") >= 0, "remove restored current"))
                    return;
                harness.controller.requestPlaybackMode(2, true);
                harness.controller.requestLyricsOffset(100);
                harness.controller.saveQueuePlaylist("QA playlist");
                break;
            case 5:
                if (!harness.checkTest(harness.controller.playlistCount === 1 && harness.controller.repeatMode === 2 && harness.controller.shuffleEnabled, "playlist and modes"))
                    return;
                harness.controller.renamePlaylist(0, "QA renamed");
                harness.controller.exportQueue(directory + "/export.m3u8");
                break;
            case 6:
                if (!harness.checkTest(harness.controller.playlistName(0).indexOf("QA renamed") >= 0, "rename playlist"))
                    return;
                harness.controller.importPlaylist(directory + "/export.m3u8");
                break;
            case 7:
                if (!harness.checkTest(harness.controller.playlistCount === 2, "M3U8 import"))
                    return;
                harness.controller.deletePlaylist(1);
                harness.controller.filterTracks("Track 1");
                break;
            case 8:
                if (!harness.checkTest(harness.controller.playlistCount === 1 && harness.controller.visibleTrackCount === 1, "search and delete"))
                    return;
                harness.controller.requestOnlineDetails("current", -1, "lyrics");
                break;
            case 9:
                if (!harness.bounded(harness.shell.onlineView.item, "online lyric import"))
                    return;
                harness.shell.onlineService.importLocal("file://" + directory + "/online-test.lrc", false);
                break;
            case 10:
                if (!harness.checkTest(harness.controller.lyricSource === "手动导入" && harness.controller.lyricCuesJson.indexOf("Online resource fixture") >= 0, "selected online resource reaches Rust lyrics and QML"))
                    return;
                harness.controller.requestLyricsOffset(350);
                break;
            case 11:
                if (!harness.checkTest(harness.controller.lyricsOffsetMs === 350, "selected lyric resource owns its offset"))
                    return;
                harness.shell.onlineService.restoreDefault(false);
                break;
            case 12:
                if (!harness.checkTest(harness.controller.lyricSource === "本地歌词" && harness.controller.lyricsOffsetMs === 100 && harness.controller.lyricCuesJson.indexOf("开始") >= 0, "reset restores original lyrics and offset"))
                    return;
                harness.sendEscape();
                harness.controller.requestOnlineDetails("current", -1, "cover");
                break;
            case 13:
                if (!harness.bounded(harness.shell.onlineView.item, "online cover import"))
                    return;
                harness.shell.onlineService.importLocal("file://" + directory + "/music/cover.png", false);
                break;
            case 14:
                if (!harness.checkTest(harness.controller.currentCoverUrl.indexOf("/online-assets/objects/") >= 0, "selected cover reaches current artwork"))
                    return;
                harness.pinnedTestCover = harness.controller.currentCoverUrl;
                harness.controller.nextTrack();
                break;
            case 15:
                if (!harness.checkTest(harness.controller.currentTrackPath.indexOf("Track 1") < 0 && harness.controller.currentCoverUrl !== harness.pinnedTestCover, "unknown album keeps per-track cover identity"))
                    return;
                harness.controller.previousTrack();
                break;
            case 16:
                if (!harness.checkTest(harness.controller.currentCoverUrl === harness.pinnedTestCover, "returning track recovers pinned cover"))
                    return;
                harness.shell.onlineService.restoreDefault(false);
                break;
            case 17:
                if (!harness.checkTest(harness.controller.currentCoverUrl.indexOf("/online-assets/") < 0, "cover reset restores original artwork"))
                    return;
                harness.sendEscape();
                harness.controller.requestOnlineDetails("current", -1, "lyrics");
                break;
            case 18:
                if (!harness.bounded(harness.shell.onlineView.item, "acknowledged resource application"))
                    return;
                harness.shell.onlineView.item.beginApply();
                harness.shell.onlineService.importLocal("file://" + directory + "/online-test.lrc", false);
                break;
            case 19:
                if (!harness.checkTest(!harness.shell.onlineView.item.opened && harness.controller.lyricSource === "手动导入" && harness.shell.noticeMessage.indexOf("歌词已应用") >= 0, "durable apply closes dialog and shows nonmodal success"))
                    return;
                harness.shell.onlineService.restoreDefault(false);
                break;
            case 20:
                if (!harness.checkTest(harness.controller.lyricSource === "本地歌词" && harness.controller.lyricsOffsetMs === 100, "default lyric offset survives acknowledged application"))
                    return;
                harness.controller.requestOnlineDetails("current", -1, "lyrics");
                break;
            case 21:
                if (!harness.bounded(harness.shell.onlineView.item, "word timing file import"))
                    return;
                harness.shell.onlineView.item.beginApply();
                harness.shell.onlineService.importLocal("file://" + directory + "/word-timing.ttml", false);
                break;
            case 22:
                const wordCues = JSON.parse(harness.controller.lyricCuesJson);
                if (!harness.checkTest(!harness.shell.onlineView.item.opened && wordCues.length === 1 && wordCues[0].timing === "word" && wordCues[0].words[1].start === 1000, "TTML import validates in Rust, persists and projects word timing"))
                    return;
                harness.controller.seekTo(1250);
                harness.shell.immersiveOpen = true;
                break;
            case 23:
                const view = harness.shell.immersiveView.item.lyricView;
                if (!harness.checkTest(view && view.cues.length === 1, "imported lyric reaches line-only playback view"))
                    return;
                const activeRow = view.listView.itemAtIndex(view.activeIndex);
                const wordText = activeRow ? DesktopBridge.testFind(activeRow, "lyricText") : null;
                if (!harness.checkTest(wordText && wordText.color.toString() === view.colors.text.toString() && typeof wordText.revealRects === "undefined", "imported word lyrics use immediate full-line color"))
                    return;
                DesktopBridge.captureForTest(harness.shell, "40-imported-lyrics-line");
                harness.shell.onlineService.restoreDefault(false);
                break;
            case 24:
                if (!harness.checkTest(harness.controller.lyricSource === "本地歌词" && harness.controller.lyricsOffsetMs === 100, "word file reset restores local source"))
                    return;
                harness.controller.seekTo(150);
                break;
            case 25:
                if (!harness.checkTest(harness.shell.immersiveView.item.lyricView.activeIndex === 0, "ordinary LRC follows line time and saved offset"))
                    return;
                DesktopBridge.captureForTest(harness.shell, "41-local-lyrics-line");
                harness.shell.showMinimized();
                break;
            case 26:
                // xdg-shell set_minimized is a request with no minimized-state
                // acknowledgement. The runner checks the emitted protocol call;
                // backends with an observable state keep the strict assertion.
                if (!DesktopBridge.wayland && !harness.checkTest(harness.shell.visibility === Window.Minimized, "window entered minimized state"))
                    return;
                console.info("[tray-restore] after-minimize visibility=" + harness.shell.visibility);
                harness.shell.restoreFromTray();
                break;
            case 27:
                if (!harness.checkTest(harness.shell.windowDisplayed, "tray restore clears minimized state"))
                    return;
                harness.shell.hideToTray();
                break;
            case 28:
                if (!harness.checkTest(!harness.shell.visible, "hide-to-tray preserves running application"))
                    return;
                harness.shell.showTrayMenu();
                break;
            case 29:
                if (!harness.bounded(harness.shell.trayMenuView, "hidden listening view tray fallback"))
                    return;
                if (!harness.checkTest(harness.shell.windowDisplayed && DesktopBridge.visibleNativePopups() === 0, "hidden-window fallback uses only a mapped in-scene menu"))
                    return;
                harness.sendEscape();
                break;
            case 30:
                if (!harness.checkTest(!harness.shell.trayMenuView.opened && harness.shell.immersiveOpen, "Escape closes tray fallback before listening page"))
                    return;
                harness.shell.showMaximized();
                break;
            case 31:
                if (!harness.checkTest(harness.shell.visibility === Window.Maximized, "maximized window state established"))
                    return;
                harness.shell.hideToTray();
                break;
            case 32:
                harness.shell.restoreFromTray();
                break;
            case 33:
                if (!harness.checkTest(harness.shell.visibility === Window.Maximized, "tray restoration retains maximized state"))
                    return;
                harness.shell.showNormal();
                harness.shell.immersiveOpen = false;
                break;
            case 34:
                harness.shell.persistUi();
                console.info("Functional validation passed");
                Qt.quit();
            }
        }
    }
    Timer {
        interval: 20000
        running: harness.shell.functionalTest
        onTriggered: {
            console.error("Functional validation timeout");
            Qt.exit(4);
        }
    }
}
