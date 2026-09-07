pragma ComponentBehavior: Bound
import QtQuick

Rectangle {
    id: art
    property url source
    property string title: ""
    readonly property bool imageReady: image.status === Image.Ready
    property int resolution: width <= 180 ? 256 : 768
    implicitWidth: 180
    implicitHeight: 180
    color: Theme.subtle
    clip: true
    Icon {
        anchors.centerIn: parent
        name: "disc"
        color: Theme.muted
        size: Math.max(20, Math.min(56, art.width * 0.28))
        opacity: 0.4
        visible: !art.imageReady
    }
    Image {
        id: image
        anchors.fill: parent
        source: art.source
        asynchronous: true
        cache: true
        autoTransform: true
        fillMode: Image.PreserveAspectCrop
        sourceSize: Qt.size(art.resolution, art.resolution)
        visible: art.imageReady
    }
    Rectangle {
        anchors.fill: parent
        color: "transparent"
        border.width: 1
        border.color: Theme.dark ? "#10ffffff" : "#0c000000"
    }
    Accessible.role: Accessible.Graphic
    Accessible.name: art.title
}
