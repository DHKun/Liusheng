pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Shapes

Rectangle {
    id: cover

    property url source
    property string title
    property color surfaceColor: "#151d22"
    property color foregroundColor: "#e8edf0"
    property color accentColor: "#b85f4a"
    property color surroundingColor: surfaceColor
    property real cornerRadius: 16
    property int frameWidth: 0
    property color frameColor: "transparent"
    readonly property bool imageReady: coverImage.status === Image.Ready

    implicitWidth: 180
    implicitHeight: 180
    radius: cornerRadius
    color: surfaceColor
    antialiasing: true
    clip: true

    gradient: Gradient {
        orientation: Gradient.Horizontal
        GradientStop {
            position: 0
            color: Qt.tint(cover.surfaceColor,
                           Qt.rgba(cover.accentColor.r,
                                   cover.accentColor.g,
                                   cover.accentColor.b,
                                   0.38))
        }
        GradientStop { position: 1; color: cover.surfaceColor }
    }

    Rectangle {
        id: fallbackDisc

        width: cover.width * 0.72
        height: width
        radius: width / 2
        anchors.right: parent.right
        anchors.rightMargin: -width * 0.16
        anchors.verticalCenter: parent.verticalCenter
        color: Qt.darker(cover.surfaceColor, 1.32)
        border.width: 1
        border.color: Qt.rgba(cover.foregroundColor.r,
                              cover.foregroundColor.g,
                              cover.foregroundColor.b,
                              0.08)

        Repeater {
            model: 7

            Rectangle {
                required property int index

                width: fallbackDisc.width - 18 - index * 12
                height: width
                radius: width / 2
                color: "transparent"
                border.width: 1
                border.color: Qt.rgba(cover.foregroundColor.r,
                                      cover.foregroundColor.g,
                                      cover.foregroundColor.b,
                                      0.07)
                anchors.centerIn: fallbackDisc
            }
        }

        Rectangle {
            width: fallbackDisc.width * 0.3
            height: width
            radius: width / 2
            color: cover.accentColor
            anchors.centerIn: parent
        }
    }

    Text {
        anchors.left: parent.left
        anchors.bottom: parent.bottom
        anchors.leftMargin: Math.max(8, cover.width * 0.1)
        anchors.bottomMargin: Math.max(6, cover.height * 0.055)
        text: cover.title.length > 0 ? cover.title.slice(0, 1) : ""
        color: Qt.rgba(cover.foregroundColor.r,
                       cover.foregroundColor.g,
                       cover.foregroundColor.b,
                       0.92)
        font.family: "Noto Sans CJK SC"
        font.pixelSize: Math.max(18, cover.width * 0.28)
        font.weight: Font.Black
    }

    Image {
        id: coverImage

        anchors.fill: parent
        source: cover.source
        asynchronous: true
        cache: true
        autoTransform: true
        fillMode: Image.PreserveAspectCrop
        mipmap: true
        sourceSize.width: Math.min(720, Math.max(96, Math.ceil(cover.width * 1.5)))
        sourceSize.height: Math.min(720, Math.max(96, Math.ceil(cover.height * 1.5)))
        visible: cover.imageReady
        opacity: cover.imageReady ? 1 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: Application.styleHints.useHoverEffects ? 180 : 0
                easing.type: Easing.OutCubic
            }
        }
    }

    Shape {
        anchors.fill: parent
        visible: cover.imageReady && cover.cornerRadius > 0

        ShapePath {
            fillColor: cover.surroundingColor
            strokeColor: "transparent"
            startX: 0
            startY: cover.cornerRadius
            PathLine { x: 0; y: 0 }
            PathLine { x: cover.cornerRadius; y: 0 }
            PathArc {
                x: 0
                y: cover.cornerRadius
                radiusX: cover.cornerRadius
                radiusY: cover.cornerRadius
                direction: PathArc.Counterclockwise
            }
        }

        ShapePath {
            fillColor: cover.surroundingColor
            strokeColor: "transparent"
            startX: cover.width - cover.cornerRadius
            startY: 0
            PathLine { x: cover.width; y: 0 }
            PathLine { x: cover.width; y: cover.cornerRadius }
            PathArc {
                x: cover.width - cover.cornerRadius
                y: 0
                radiusX: cover.cornerRadius
                radiusY: cover.cornerRadius
                direction: PathArc.Counterclockwise
            }
        }

        ShapePath {
            fillColor: cover.surroundingColor
            strokeColor: "transparent"
            startX: cover.width
            startY: cover.height - cover.cornerRadius
            PathLine { x: cover.width; y: cover.height }
            PathLine { x: cover.width - cover.cornerRadius; y: cover.height }
            PathArc {
                x: cover.width
                y: cover.height - cover.cornerRadius
                radiusX: cover.cornerRadius
                radiusY: cover.cornerRadius
                direction: PathArc.Counterclockwise
            }
        }

        ShapePath {
            fillColor: cover.surroundingColor
            strokeColor: "transparent"
            startX: cover.cornerRadius
            startY: cover.height
            PathLine { x: 0; y: cover.height }
            PathLine { x: 0; y: cover.height - cover.cornerRadius }
            PathArc {
                x: cover.cornerRadius
                y: cover.height
                radiusX: cover.cornerRadius
                radiusY: cover.cornerRadius
                direction: PathArc.Counterclockwise
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: cover.cornerRadius
        color: "transparent"
        border.width: cover.frameWidth
        border.color: cover.frameColor
        antialiasing: true
    }
}
