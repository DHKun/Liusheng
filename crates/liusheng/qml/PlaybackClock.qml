import QtQuick
import io.github.dhkun.Liusheng 1.0

QtObject {
    id: clock
    property int sourcePosition: 0
    property int duration: 0
    property bool playing: false
    property bool displayed: true
    property real anchorTime: 0
    property real position: sourcePosition
    function align() {
        anchorTime = DesktopBridge.monotonicMs();
        position = sourcePosition;
    }
    onSourcePositionChanged: align()
    onPlayingChanged: {
        // Freeze the last visible position immediately on pause. A subsequent
        // engine position snapshot remains authoritative through align().
        if (playing)
            align();
        else
            anchorTime = DesktopBridge.monotonicMs();
    }
    onDisplayedChanged: align()
    property Timer ticker: Timer {
        interval: 33
        repeat: true
        running: clock.playing && clock.displayed
        onTriggered: {
            const estimate = clock.sourcePosition + Math.min(700, Math.max(0, DesktopBridge.monotonicMs() - clock.anchorTime));
            clock.position = clock.duration > 0 ? Math.min(clock.duration, estimate) : estimate;
        }
    }
}
