import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "ui" as UI

Item {
    id: stage
    width: 1000
    height: 680

    Component {
        id: buttonComponent
        UI.QuietButton {
            text: "Play"
            glyph: "play"
            property int clicks: 0
            onClicked: clicks++
        }
    }
    Component {
        id: iconComponent
        UI.Icon {}
    }
    Component {
        id: fieldComponent
        UI.QuietField {
            width: 240
            search: true
            placeholderText: "Search"
        }
    }
    Component {
        id: checkComponent
        UI.QuietCheckBox {
            text: "Restore session"
        }
    }
    Component {
        id: sliderComponent
        UI.QuietSlider {
            width: 240
            from: 0
            to: 100
            stepSize: 1
        }
    }
    Component {
        id: comboComponent
        UI.QuietComboBox {
            model: ["System", "Light", "Dark"]
        }
    }
    Component {
        id: dialogComponent
        UI.QuietDialog {
            title: "Confirmation"
            property int saves: 0
            property int cancels: 0
            onAccepted: saves++
            onRejected: cancels++
            contentItem: UI.QuietField {
                placeholderText: "Name"
            }
        }
    }
    QtObject {
        id: mockController
        property bool playbackInitializing: false
        property int currentQueueIndex: -1
        property string currentTrackPath: ""
        property int calledIndex: -1
        property int queueCount: 2
        property int movedFrom: -1
        property int movedTo: -1
        function moveQueueTrack(from, to) {
            movedFrom = from;
            movedTo = to;
        }
        function playAllTrack(i) {
            calledIndex = i;
        }
        function playQueueTrack(i) {
            calledIndex = i;
        }
        function playSelectedTrack(i) {
            calledIndex = i;
        }
    }
    ListModel {
        id: songs
        ListElement {
            sourceIndex: 17
            rowTitle: "A very long song title that should remain in its column"
            rowArtist: "Artist"
            rowAlbum: "Album"
            rowPath: "/one.wav"
            rowNumber: "01"
            rowDuration: 200000
        }
        ListElement {
            sourceIndex: 29
            rowTitle: "Second"
            rowArtist: "Artist"
            rowAlbum: "Album"
            rowPath: "/two.wav"
            rowNumber: "02"
            rowDuration: 190000
        }
    }
    Component {
        id: tableComponent
        UI.TrackTable {
            width: 760
            height: 300
            controller: mockController
            trackModel: songs
        }
    }

    Component {
        id: navComponent
        UI.NavigationItem {
            width: 176
            text: "Navigation"
            glyph: "album"
        }
    }
    Component {
        id: menuComponent
        UI.QuietMenu {
            property int actions: 0
            UI.QuietMenuItem {
                text: "First"
                onTriggered: parent.actions++
            }
            UI.QuietMenuItem {
                text: "Second"
            }
        }
    }
    Component {
        id: popoverComponent
        UI.QuietPopover {
            width: 260
            height: 160
            UI.QuietComboBox {
                id: choice
                objectName: "nestedCombo"
                width: 200
                model: ["System", "Light", "Dark"]
            }
        }
    }
    ListModel {
        id: queuedSongs
        ListElement {
            sourceIndex: 0
            rowTitle: "First"
            rowArtist: "Artist"
            rowAlbum: "Album"
            rowPath: "/one.wav"
            rowNumber: "01"
            rowDuration: 200000
        }
        ListElement {
            sourceIndex: 1
            rowTitle: "Second"
            rowArtist: "Artist"
            rowAlbum: "Album"
            rowPath: "/two.wav"
            rowNumber: "02"
            rowDuration: 190000
        }
    }
    Component {
        id: queueComponent
        UI.TrackTable {
            width: 380
            height: 280
            controller: mockController
            trackModel: queuedSongs
            mode: "queue"
            condensed: true
        }
    }
    Component {
        id: scrollbarComponent
        UI.QuietScrollBar {
            height: 200
        }
    }
    QtObject {
        id: visualController
        property string currentTitle: "A long music title / 很长的曲名 / 長いタイトル"
        property string currentArtist: "Artist"
        property string currentCoverUrl: ""
        property string currentAccent: "#885b38"
        property bool hasCurrentTrack: true
        property bool seekable: true
        property int lyricLineCount: 3
        property int currentLyricIndex: 1
        property real positionMs: 0
        property bool playing: false
        property string currentTrackPath: "/fixture/song.wav"
        property string lyricCuesJson: '[{"time":0,"text":"第一行","secondary":"First line"},{"time":1000,"text":"第二行","secondary":""}]'
        property int lyricsRevision: 1
        property bool lyricsLoading: false
        property string lyricsError: ""
        property int lyricsOffsetMs: 0
        function lyricText(index) {
            return ["音乐在这里流淌", "いつかまた音楽を聴きましょう", "A quiet conversation"][index];
        }
        function lyricTimeMs(index) {
            return index * 1000;
        }
        function seekTo(ms) {
        }
        function requestAlbumCover(index) {
        }
        function requestArtistCover(index) {
        }
        signal artworkRevisionChanged
    }
    Component {
        id: immersiveComponent
        UI.ImmersivePlayer {
            width: 1000
            height: 640
            controller: visualController
        }
    }
    Component {
        id: collectionComponent
        UI.CollectionView {
            width: 1016
            height: 600
            controller: visualController
            collectionModel: 12
            delegate: Item {
                required property int index
                width: GridView.view.cellWidth
                height: GridView.view.cellHeight
            }
        }
    }
    QtObject {
        id: outputMock
        property string outputStatus: "USB audio / " + "A device with a long descriptive name ".repeat(8)
        property string outputError: "连接暂时不可用，请检查设备。".repeat(30)
        property bool exclusiveOutput: false
        property bool outputSwitching: false
        property bool hardwareVolumeAvailable: false
        property int hardwareVolumePercent: 100
        property bool hardwareMuted: false
        property bool hardwareMuteAvailable: false
        property string audioDetails: "48000 Hz · 24 bit · 2 channels\n".repeat(30)
        function refreshHardwareVolume() {
        }
    }
    Component {
        id: outputComponent
        UI.OutputPopover {
            controller: outputMock
            detailsVisible: true
        }
    }
    TestCase {
        name: "QuietControls"
        when: windowShown
        function init() {
            UI.Theme.previewAppearance = "light";
            UI.Theme.reducedMotion = true;
            mockController.calledIndex = -1;
            mockController.movedFrom = -1;
            mockController.movedTo = -1;
        }
        function test_button_mouse_keyboard_and_disabled() {
            const button = createTemporaryObject(buttonComponent, stage, {
                x: 20,
                y: 20
            });
            verify(button);
            mouseClick(button);
            compare(button.clicks, 1);
            button.forceActiveFocus();
            keyClick(Qt.Key_Space);
            compare(button.clicks, 2);
            button.enabled = false;
            mouseClick(button);
            compare(button.clicks, 2);
        }
        function test_every_icon_decodes_in_both_themes() {
            const icon = createTemporaryObject(iconComponent, stage, {
                x: 20,
                y: 20
            });
            const names = icon.names;
            verify(names.length >= 25);
            for (const theme of ["light", "dark"]) {
                UI.Theme.previewAppearance = theme;
                for (const name of names) {
                    icon.name = name;
                    wait(5);
                    tryCompare(icon, "ready", true);
                }
            }
        }
        function test_search_field_edit_and_clear() {
            const field = createTemporaryObject(fieldComponent, stage, {
                x: 20,
                y: 20
            });
            field.forceActiveFocus();
            keyClick(Qt.Key_A);
            keyClick(Qt.Key_B);
            compare(field.text, "ab");
            mouseClick(field, field.width - 16, field.height / 2);
            compare(field.text, "");
            verify(field.activeFocus);
        }
        function test_checkbox_keyboard() {
            const box = createTemporaryObject(checkComponent, stage, {
                x: 20,
                y: 20
            });
            box.forceActiveFocus();
            keyClick(Qt.Key_Space);
            verify(box.checked);
            keyClick(Qt.Key_Space);
            verify(!box.checked);
        }
        function test_slider_keyboard() {
            const slider = createTemporaryObject(sliderComponent, stage, {
                x: 20,
                y: 20,
                value: 50
            });
            slider.forceActiveFocus();
            keyClick(Qt.Key_Right);
            compare(slider.value, 51);
            keyClick(Qt.Key_Left);
            compare(slider.value, 50);
        }
        function test_combobox_keyboard() {
            const combo = createTemporaryObject(comboComponent, stage, {
                x: 20,
                y: 20
            });
            combo.forceActiveFocus();
            keyClick(Qt.Key_Down);
            compare(combo.currentIndex, 1);
            compare(combo.currentText, "Light");
        }
        function test_dialog_keeps_content_clear_of_header_and_footer() {
            const dialog = createTemporaryObject(dialogComponent, stage);
            dialog.open();
            tryCompare(dialog, "opened", true);
            verify(dialog.contentItem.height >= 38, "The name field must retain its input hit area");
            verify(dialog.height >= dialog.header.height + dialog.footer.height + dialog.topPadding + dialog.bottomPadding + 38);
            dialog.contentItem.forceActiveFocus();
            keyClick(Qt.Key_A);
            compare(dialog.contentItem.text, "a");
            dialog.reject();
        }
        function test_dialog_escape_preserves_changes() {
            const dialog = createTemporaryObject(dialogComponent, stage);
            dialog.open();
            tryCompare(dialog, "opened", true);
            keyClick(Qt.Key_Escape);
            tryCompare(dialog, "opened", false);
            compare(dialog.saves, 0);
        }
        function test_table_keyboard_uses_source_index() {
            const table = createTemporaryObject(tableComponent, stage, {
                x: 10,
                y: 10
            });
            const list = findChild(table, "trackTable");
            verify(list);
            tryCompare(list, "count", 2);
            list.currentIndex = 0;
            list.forceActiveFocus();
            keyClick(Qt.Key_Return);
            compare(mockController.calledIndex, 17);
            keyClick(Qt.Key_Down);
            keyClick(Qt.Key_Return);
            compare(mockController.calledIndex, 29);
        }
        function test_navigation_alignment_matches_settings() {
            const first = createTemporaryObject(navComponent, stage, {
                x: 10,
                y: 10
            });
            const settings = createTemporaryObject(navComponent, stage, {
                x: 10,
                y: 70,
                text: "Settings",
                glyph: "settings"
            });
            wait(10);
            const a = findChild(first, "navigationLabel");
            const b = findChild(settings, "navigationLabel");
            compare(a.mapToItem(stage, 0, 0).x, b.mapToItem(stage, 0, 0).x);
            const ia = findChild(first, "navigationGlyph");
            const ib = findChild(settings, "navigationGlyph");
            compare(ia.mapToItem(stage, 0, 0).x, ib.mapToItem(stage, 0, 0).x);
            first.iconOnly = true;
            settings.iconOnly = true;
            first.width = settings.width = 48;
            wait(10);
            tryVerify(function () {
                return Math.abs(ia.mapToItem(first, 0, 0).x - 14) < 0.01;
            });
            verify(!a.visible && !b.visible);
        }
        function test_menu_position_is_clamped_at_all_edges() {
            const button = createTemporaryObject(buttonComponent, stage, {
                x: 20,
                y: 20
            });
            const menu = createTemporaryObject(menuComponent, stage);
            for (const point of [[-50, -50], [970, -50], [-50, 650], [970, 650]]) {
                menu.openAt(button, point[0], point[1]);
                tryCompare(menu, "opened", true);
                compare(menu.popupType, Popup.Item);
                verify(menu.x >= 0 && menu.y >= 0);
                verify(menu.x + menu.width <= stage.width + 1);
                verify(menu.y + menu.height <= stage.height + 1);
                keyClick(Qt.Key_Escape);
                tryCompare(menu, "opened", false);
            }
        }
        function test_menu_outside_click_dismisses() {
            const button = createTemporaryObject(buttonComponent, stage, {
                x: 200,
                y: 100
            });
            const menu = createTemporaryObject(menuComponent, stage);
            menu.openBelow(button);
            tryCompare(menu, "opened", true);
            mouseClick(stage, 10, 500);
            tryCompare(menu, "opened", false);
        }
        function test_nested_combo_closes_before_popover() {
            const button = createTemporaryObject(buttonComponent, stage, {
                x: 250,
                y: 30
            });
            const popup = createTemporaryObject(popoverComponent, stage);
            popup.openBelow(button);
            tryCompare(popup, "opened", true);
            const combo = findChild(popup, "nestedCombo");
            mouseClick(combo);
            tryCompare(combo.popup, "opened", true);
            compare(combo.popup.popupType, Popup.Item);
            keyClick(Qt.Key_Escape);
            tryCompare(combo.popup, "opened", false);
            verify(popup.opened);
            keyClick(Qt.Key_Escape);
            tryCompare(popup, "opened", false);
        }
        function test_table_right_click_menu_and_return_focus() {
            const table = createTemporaryObject(tableComponent, stage, {
                x: 20,
                y: 20
            });
            const list = table.listView;
            tryCompare(list, "count", 2);
            const row = list.itemAtIndex(1);
            mouseClick(row, 130, 20, Qt.RightButton);
            tryCompare(row.actionMenu, "opened", true);
            compare(row.actionMenu.popupType, Popup.Item);
            keyClick(Qt.Key_Escape);
            tryCompare(row.actionMenu, "opened", false);
            verify(list.activeFocus);
            list.currentIndex = 1;
            keyClick(Qt.Key_F10, Qt.ShiftModifier);
            tryCompare(row.actionMenu, "opened", true);
            keyClick(Qt.Key_Escape);
        }
        function test_table_long_mixed_labels_remain_bounded() {
            const table = createTemporaryObject(tableComponent, stage, {
                x: 20,
                y: 20,
                width: 390
            });
            const list = table.listView;
            tryCompare(list, "count", 2);
            verify(!table.columns);
            songs.setProperty(0, "rowTitle", "很长的中文标题 / 長い日本語タイトル / An extremely long English title ".repeat(8));
            wait(20);
            const row = list.itemAtIndex(0);
            verify(row.width <= table.width);
            compare(row.height, UI.Theme.rowHeight);
            songs.setProperty(0, "rowTitle", "First song");
        }
        function test_queue_drag_reorders_without_starting_playback() {
            const table = createTemporaryObject(queueComponent, stage, {
                x: 20,
                y: 20
            });
            tryCompare(table, "count", 2);
            const row = table.listView.itemAtIndex(0);
            const handle = findChild(row, "queueDragHandle");
            verify(handle);
            mouseDrag(handle, 12, 20, 0, 64, Qt.LeftButton);
            compare(mockController.movedFrom, 0);
            compare(mockController.movedTo, 1);
            compare(mockController.calledIndex, -1);
        }
        function test_scrollbar_only_draws_for_overflow() {
            const bar = createTemporaryObject(scrollbarComponent, stage, {
                size: 1
            });
            verify(!bar.contentItem.visible);
            bar.size = 0.4;
            verify(bar.contentItem.visible);
        }
        function test_outside_click_keeps_search_focus_after_row_menu() {
            const table = createTemporaryObject(tableComponent, stage, {
                x: 20,
                y: 20
            });
            const search = createTemporaryObject(fieldComponent, stage, {
                x: 20,
                y: 400
            });
            tryCompare(table, "count", 2);
            const row = table.listView.itemAtIndex(0);
            row.openActions();
            tryCompare(row.actionMenu, "opened", true);
            mouseClick(search, 45, 18);
            tryCompare(row.actionMenu, "opened", false);
            verify(search.activeFocus);
        }
        function test_grid_uses_every_computed_column() {
            const grid = createTemporaryObject(collectionComponent, stage);
            UI.Theme.compactGrid = true;
            wait(30);
            compare(Math.floor(grid.width / grid.cellWidth), grid.columns);
            compare(grid.itemAtIndex(grid.columns - 1).y, 0);
            verify(grid.itemAtIndex(grid.columns).y >= grid.cellHeight);
            verify(grid.cellHeight * 2 <= grid.height);
        }
        function test_icons_stay_square_in_wide_table_columns() {
            const icon = createTemporaryObject(iconComponent, stage, {
                width: 46,
                height: 13,
                name: "clock"
            });
            const image = findChild(icon, "glyphImage");
            compare(image.width, image.height);
            compare(image.width, 13);
        }
        function test_wide_now_playing_keeps_lyrics_visible() {
            const page = createTemporaryObject(immersiveComponent, stage);
            const lyrics = findChild(page, "nowPlayingLyricsPane");
            const cover = findChild(page, "nowPlayingCoverPane");
            tryVerify(function () {
                return lyrics.width >= 300 && cover.width >= 280;
            });
            verify(lyrics.visible && cover.visible);
            page.width = 820;
            page.lyricsOnly = true;
            tryVerify(function () {
                return lyrics.visible && lyrics.width >= 500;
            });
            verify(!cover.visible);
        }
        function test_long_output_details_scroll_inside_window() {
            const popup = createTemporaryObject(outputComponent, stage);
            popup.open();
            tryCompare(popup, "opened", true);
            verify(popup.height <= stage.height - 128);
            verify(popup.contentItem.contentHeight > popup.contentItem.availableHeight);
            keyClick(Qt.Key_Escape);
            tryCompare(popup, "opened", false);
        }
        function luminance(c) {
            const rgb = [c.r, c.g, c.b].map(v => v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4));
            return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
        }
        function contrast(a, b) {
            const x = luminance(a), y = luminance(b);
            return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
        }
        function test_readable_theme_contrast() {
            for (const name of ["light", "dark"]) {
                UI.Theme.previewAppearance = name;
                verify(contrast(UI.Theme.text, UI.Theme.background) >= 7);
                verify(contrast(UI.Theme.secondary, UI.Theme.background) >= 4.5);
                verify(contrast(UI.Theme.accent, UI.Theme.accentWash) >= 4.5);
            }
        }
    }
}
