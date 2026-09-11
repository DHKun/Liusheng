import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "ui" as UI

Item {
    id: stage
    width: 1280
    height: 800
    QtObject {
        id: model
        property string currentTitle: "A quiet place / 夜色の中で / 听见此刻"
        property string currentArtist: "Fixture artist"
        property string currentCoverUrl: ""
        property string currentAccent: "#cc5544"
        property string currentTrackPath: "/fixture/song.wav"
        property bool hasCurrentTrack: true
        property bool seekable: true
        property bool playing: false
        property int lyricLineCount: 4
        property int lyricsOffsetMs: 0
        property string lyricCuesJson: "[]"
        property bool lyricsLoading: false
        property string lyricsError: ""
        property int currentDurationMs: 12000
        property real positionMs: 0
        property int lastSeek: -1
        function seekTo(value) {
            lastSeek = value;
            positionMs = value;
        }
        function requestLyricsOffset(value) {
            lyricsOffsetMs = value;
        }
    }
    UI.ListeningPalette {
        id: testColors
    }
    Component {
        id: colorsComponent
        UI.ListeningPalette {}
    }
    Component {
        id: lyricsComponent
        UI.LyricsView {
            width: 600
            height: 480
            controller: model
            colors: testColors
            positionMs: model.positionMs
        }
    }
    Component {
        id: pageComponent
        UI.ImmersivePlayer {
            width: 1100
            height: 620
            controller: model
            windowActive: true
        }
    }
    Component {
        id: flightComponent
        UI.CoverFlight {}
    }
    Component {
        id: blockComponent
        Item {}
    }
    TestCase {
        name: "ListeningExperience"
        when: windowShown
        function init() {
            UI.Theme.previewAppearance = "light";
            UI.Theme.reducedMotion = true;
            UI.Theme.coverTheme = true;
            UI.Theme.ambientMotion = true;
            UI.Theme.lyricSecondary = true;
            model.positionMs = 0;
            model.lyricsOffsetMs = 0;
            model.playing = false;
            model.seekable = true;
            model.lastSeek = -1;
            model.currentCoverUrl = "";
            model.lyricCuesJson = JSON.stringify([
                {
                    time: 1000,
                    text: "Hello / こんにちは / 你好",
                    secondary: "译文与附文"
                },
                {
                    time: 3000,
                    text: "The long line returns to the same readable measure. 很长的中文歌词应当在边界内换行。",
                    secondary: "A secondary line"
                },
                {
                    time: 5000,
                    text: "",
                    secondary: ""
                },
                {
                    time: 8000,
                    text: "The last line",
                    secondary: ""
                }
            ]);
        }
        function test_lyric_line_switches_color_immediately_at_source_boundaries() {
            UI.Theme.reducedMotion = false;
            model.lyricCuesJson = JSON.stringify([
                {time: 1000, text: "First line", secondary: "第一行"},
                {time: 3000, text: "Second line", secondary: "第二行"}
            ]);
            model.positionMs = 999;
            const view = createTemporaryObject(lyricsComponent, stage);
            tryVerify(() => view.listView.itemAtIndex(0) !== null && view.listView.itemAtIndex(1) !== null);
            const first = findChild(view.listView.itemAtIndex(0), "lyricText");
            const second = findChild(view.listView.itemAtIndex(1), "lyricText");
            compare(first.color, testColors.secondary);
            model.positionMs = 1000;
            compare(first.color, testColors.text);
            compare(second.color, testColors.secondary);
            model.positionMs = 2500;
            compare(first.color, testColors.text);
            model.positionMs = 3000;
            compare(first.color, testColors.secondary);
            compare(second.color, testColors.text);
            model.positionMs = 1200;
            compare(first.color, testColors.text);
            compare(second.color, testColors.secondary);
        }
        function test_imported_word_timing_uses_the_same_line_only_view() {
            model.lyricCuesJson = JSON.stringify([{time: 1000, end: 4000, text: "Hello 世界", secondary: "翻译", timing: "word", words: [{offset: 0, length: 6, start: 1000, end: 1500}, {offset: 6, length: 2, start: 2500, end: 4000}]}]);
            model.positionMs = 1000;
            const view = createTemporaryObject(lyricsComponent, stage);
            tryVerify(() => view.listView.itemAtIndex(0) !== null);
            const text = findChild(view.listView.itemAtIndex(0), "lyricText");
            compare(text.text, "Hello 世界");
            compare(text.color, testColors.text);
            compare(text.textFormat, Text.PlainText);
            compare(typeof text.revealRects, "undefined");
            model.positionMs = 2400;
            compare(text.color, testColors.text);
            view.showSecondary = false;
            compare(text.color, testColors.text);
            model.lyricsOffsetMs = 2000;
            compare(text.color, testColors.secondary);
            compare(findChild(view, "lyricTimingLabel"), null);
        }
        function test_palette_all_saturated_seeds_retain_contrast() {
            const colors = createTemporaryObject(colorsComponent, stage);
            for (const dark of [false, true]) {
                colors.dark = dark;
                for (let r of [0, 0.5, 1])
                    for (let g of [0, 0.5, 1])
                        for (let b of [0, 0.5, 1]) {
                            colors.seed = Qt.rgba(r, g, b, 1);
                            for (const background of [colors.background, colors.edge, colors.surface]) {
                                verify(colors.contrast(colors.text, background) >= 7, "primary contrast " + colors.seed);
                                verify(colors.contrast(colors.secondary, background) >= 4.5, "secondary contrast " + colors.seed);
                            }
                            verify(colors.contrast(colors.accent, colors.background) >= 4.5);
                            verify(colors.contrast(colors.accent, colors.surface) >= 4.5);
                        }
            }
        }
        function test_timing_boundaries_offset_and_instrumental() {
            const view = createTemporaryObject(lyricsComponent, stage);
            compare(view.activeIndex, -1);
            model.positionMs = 1000;
            compare(view.activeIndex, 0);
            model.positionMs = 2999;
            compare(view.activeIndex, 0);
            model.positionMs = 3000;
            compare(view.activeIndex, 1);
            model.positionMs = 5000;
            verify(view.instrumental);
            model.lyricsOffsetMs = 500;
            compare(view.activeIndex, 1);
            model.positionMs = 8500;
            compare(view.activeIndex, 3);
        }
        function test_positive_offset_delays_zero_timestamp_cue() {
            model.lyricCuesJson = '[{"time":0,"text":"Delayed first line","secondary":""}]';
            model.lyricsOffsetMs = 500;
            const view = createTemporaryObject(lyricsComponent, stage);
            compare(view.activeIndex, -1);
            model.positionMs = 499;
            compare(view.activeIndex, -1);
            model.positionMs = 500;
            compare(view.activeIndex, 0);
        }
        function test_secondary_text_does_not_change_seek_time() {
            const view = createTemporaryObject(lyricsComponent, stage);
            model.lyricsOffsetMs = 120;
            view.activate(1);
            compare(model.lastSeek, 3120);
            view.showSecondary = false;
            view.activate(1);
            compare(model.lastSeek, 3120);
            model.seekable = false;
            view.activate(2);
            compare(model.lastSeek, 3120);
        }
        function test_manual_reading_and_explicit_follow_resume() {
            const view = createTemporaryObject(lyricsComponent, stage);
            model.positionMs = 1000;
            view.suspendFollow();
            verify(!view.autoFollow);
            model.positionMs = 1300;
            verify(!view.autoFollow);
            const button = findChild(view, "resumeLyricsFollow");
            verify(button.visible);
            mouseClick(button);
            verify(view.autoFollow);
        }
        function test_seek_discontinuity_and_new_track_restore_follow() {
            const view = createTemporaryObject(lyricsComponent, stage);
            view.suspendFollow();
            model.positionMs = 8000;
            verify(view.autoFollow);
            view.suspendFollow();
            model.currentTrackPath = "/fixture/another.wav";
            verify(view.autoFollow);
        }
        function test_untimed_lyrics_are_readable_and_never_seek() {
            model.lyricCuesJson = '[{"time":null,"text":"<b>literal markup</b>","secondary":""},{"time":null,"text":"第二行","secondary":""}]';
            const view = createTemporaryObject(lyricsComponent, stage);
            compare(view.activeIndex, -1);
            view.activate(0);
            compare(model.lastSeek, -1);
            const text = findChild(view, "lyricText");
            verify(text);
            compare(text.textFormat, Text.PlainText);
        }
        function test_empty_and_malformed_cues_reset_previous_song() {
            const view = createTemporaryObject(lyricsComponent, stage);
            compare(view.cues.length, 4);
            model.lyricCuesJson = "[]";
            compare(view.cues.length, 0);
            model.lyricCuesJson = "invalid-json";
            compare(view.cues.length, 0);
        }
        function test_ambient_stops_on_pause_hide_inactive_and_reduced_motion() {
            UI.Theme.reducedMotion = false;
            const page = createTemporaryObject(pageComponent, stage);
            verify(!page.ambientRunning);
            model.playing = true;
            verify(page.ambientRunning);
            page.windowActive = false;
            verify(!page.ambientRunning);
            page.windowActive = true;
            verify(page.ambientRunning);
            page.displayed = false;
            verify(!page.ambientRunning);
            page.displayed = true;
            UI.Theme.reducedMotion = true;
            verify(!page.ambientRunning);
        }
        function test_layout_modes_stay_in_same_window() {
            const page = createTemporaryObject(pageComponent, stage);
            const cover = findChild(page, "nowPlayingCoverPane");
            const lyrics = findChild(page, "nowPlayingLyricsPane");
            verify(cover.visible && lyrics.visible);
            page.layoutMode = "lyrics";
            verify(!cover.visible && lyrics.visible);
            verify(lyrics.width >= 600);
            page.layoutMode = "cover";
            verify(cover.visible && !lyrics.visible);
            compare(page.layoutMenu.popupType, Popup.Item);
            compare(page.optionsMenu.popupType, Popup.Item);
        }
        function test_narrow_and_short_layout_remain_within_viewport() {
            const page = createTemporaryObject(pageComponent, stage, {
                width: 820,
                height: 464
            });
            const cover = page.coverItem;
            wait(10);
            const point = cover.mapToItem(page, 0, 0);
            verify(point.x >= 0 && point.y >= 0);
            verify(point.x + cover.width <= page.width);
            const artist = findChild(page, "listeningArtist");
            tryVerify(function () {
                return artist.mapToItem(page, 0, artist.height).y <= page.height - 12;
            });
            page.lyricsOnly = true;
            const pane = findChild(page, "nowPlayingLyricsPane");
            verify(pane.visible && pane.width >= 500);
        }
        function test_cover_transition_cancel_and_reduce_motion() {
            const a = createTemporaryObject(blockComponent, stage, {
                x: 20,
                y: 30,
                width: 52,
                height: 52
            });
            const b = createTemporaryObject(blockComponent, stage, {
                x: 200,
                y: 130,
                width: 300,
                height: 300
            });
            const flight = createTemporaryObject(flightComponent, stage, {
                source: Qt.resolvedUrl("ui/assets/app-icon/liusheng.svg")
            });
            flight.fly(a, b);
            verify(!flight.running);
            UI.Theme.reducedMotion = false;
            flight.fly(a, b);
            verify(flight.running);
            flight.cancel();
            verify(!flight.running && !flight.visible);
            flight.fly(b, a);
            flight.allowed = false;
            verify(!flight.running && !flight.visible);
        }
    }
}
