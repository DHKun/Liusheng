pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: card

    property string albumTitle
    property string albumArtist
    property url coverSource
    property int trackCount
    property int albumYear
    property color surfaceColor: "#151d22"
    property color foregroundColor: "#e8edf0"
    property color mutedColor: "#829198"
    property color accentColor: "#b85f4a"
    property color surroundingColor: "#0b1114"
    property bool interactive: true

    signal activated

    activeFocusOnTab: interactive
    scale: interactive && hoverHandler.hovered && Application.styleHints.useHoverEffects ? 1.018 : 1
    Accessible.role: Accessible.Button
    Accessible.name: qsTr("打开专辑 %1").arg(albumTitle)
    Accessible.ignored: !interactive
    Keys.onReturnPressed: if (card.interactive) card.activated()
    Keys.onEnterPressed: if (card.interactive) card.activated()

    Behavior on scale {
        NumberAnimation { duration: 220; easing.type: Easing.OutCubic }
    }

    HoverHandler {
        id: hoverHandler
        enabled: card.interactive
    }

    TapHandler {
        enabled: card.interactive
        onTapped: {
            card.forceActiveFocus()
            card.activated()
        }
    }

    CoverArt {
        id: artwork

        width: parent.width
        height: width
        source: card.coverSource
        title: card.albumTitle
        surfaceColor: card.surfaceColor
        foregroundColor: card.foregroundColor
        accentColor: card.accentColor
        surroundingColor: card.surroundingColor
        cornerRadius: 16
        frameWidth: card.interactive && card.activeFocus ? 2 : 0
        frameColor: card.accentColor
    }

    Column {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: artwork.bottom
        anchors.topMargin: 12
        spacing: 4

        Text {
            width: parent.width
            text: card.albumTitle
            color: card.foregroundColor
            elide: Text.ElideRight
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 14
            font.weight: Font.DemiBold
        }

        Text {
            width: parent.width
            text: card.albumArtist
            color: card.mutedColor
            elide: Text.ElideRight
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 12
        }

        Text {
            width: parent.width
            text: card.albumYear > 0
                  ? qsTr("%1，%2 首").arg(card.albumYear).arg(card.trackCount)
                  : qsTr("%1 首").arg(card.trackCount)
            color: card.mutedColor
            font.family: "JetBrains Mono"
            font.pixelSize: 10
        }
    }
}
