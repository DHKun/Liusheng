pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.impl as Impl

Item {
    id: glyph
    property string name: "music"
    property color color: Theme.text
    property int size: 20
    readonly property bool ready: image.status === Image.Ready
    // The only dependency on Qt Controls' image implementation is isolated here.
    // It tints cached SVG pixels on both software and GPU scene-graph backends.
    readonly property var names: ["album", "artist", "music", "playlist", "queue", "play", "pause", "previous", "next", "shuffle", "repeat", "repeat-one", "search", "filter", "back", "down", "up", "more", "settings", "folder", "import", "export", "trash", "lyrics", "output", "volume", "mute", "close", "check", "plus", "grip", "refresh", "clock", "brand", "disc"]
    implicitWidth: size
    implicitHeight: size
    Accessible.ignored: true
    Impl.IconImage {
        id: image
        objectName: "glyphImage"
        anchors.centerIn: parent
        width: Math.min(glyph.width, glyph.height)
        height: width
        color: glyph.color
        source: Qt.resolvedUrl("assets/icons/" + (glyph.names.indexOf(glyph.name) >= 0 ? glyph.name : "music") + ".svg")
        sourceSize: Qt.size(Math.ceil(width * Screen.devicePixelRatio), Math.ceil(height * Screen.devicePixelRatio))
        fillMode: Image.PreserveAspectFit
        cache: true
    }
}
