pragma ComponentBehavior: Bound
import QtQuick
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
                if (DesktopBridge.wayland && !harness.checkTest(harness.shell.nativeTrayMenu === null, "native tray menu remains uninstantiated"))
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
            default:
                if (!harness.checkTest(harness.shell.playerView.height >= 80 && harness.shell.libraryView.width >= 650, "responsive minimum size"))
                    return;
                Theme.previewAppearance = "";
                console.info("UI validation passed");
                Qt.quit();
            }
        }
    }
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
