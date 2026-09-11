import QtQuick
import QtTest
import "ui" as UI

Item {
    id: stage
    width: 400
    height: 200
    Component { id: clockComponent; UI.PlaybackClock {} }
    TestCase {
        name: "LyricPlaybackClock"
        when: windowShown
        function newClock() {
            const clock = createTemporaryObject(clockComponent, stage, {sourcePosition: 1000, duration: 10000});
            verify(clock !== null);
            return clock;
        }
        function test_pause_freezes_visible_progress_and_source_seek_remains_authoritative() {
            const clock = newClock();
            compare(clock.position, 1000);
            verify(!clock.ticker.running);
            clock.playing = true;
            tryVerify(() => clock.position > 1050);
            const visible = clock.position;
            clock.playing = false;
            compare(clock.position, visible);
            wait(100);
            compare(clock.position, visible);
            verify(!clock.ticker.running);
            clock.sourcePosition = 4000;
            compare(clock.position, 4000);
            wait(80);
            compare(clock.position, 4000);
            clock.sourcePosition = 500;
            compare(clock.position, 500);
        }
        function test_hidden_window_stops_tick_and_resumes_from_source() {
            const clock = newClock();
            clock.playing = true;
            tryVerify(() => clock.position > 1020);
            clock.displayed = false;
            verify(!clock.ticker.running);
            const hidden = clock.position;
            wait(100);
            compare(clock.position, hidden);
            clock.sourcePosition = 2500;
            clock.displayed = true;
            compare(clock.position, 2500);
            tryVerify(() => clock.position > 2520);
            clock.playing = false;
        }
        function test_interpolation_is_bounded_at_normal_ui_rate() {
            const clock = newClock();
            compare(clock.ticker.interval, 33);
            clock.duration = 1050;
            clock.playing = true;
            tryCompare(clock, "position", 1050);
            clock.duration = 10000;
            tryCompare(clock, "position", 1700);
            wait(70);
            compare(clock.position, 1700);
            clock.sourcePosition = 3000;
            compare(clock.position, 3000);
            clock.playing = false;
        }
    }
}
