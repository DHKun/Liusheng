pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: card

    property string albumTitle
    property string albumArtist
    property int trackCount
    property int albumYear
    property color surfaceColor: "#151d22"
    property color foregroundColor: "#e8edf0"
    property color mutedColor: "#829198"
    property color accentColor: "#b85f4a"
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

    Rectangle {
        id: artwork

        width: parent.width
        height: width
        radius: 16
        clip: true
        color: card.surfaceColor
        border.width: card.interactive && card.activeFocus ? 2 : 0
        border.color: card.accentColor

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop {
                position: 0
                color: Qt.tint(card.surfaceColor,
                               Qt.rgba(card.accentColor.r,
                                       card.accentColor.g,
                                       card.accentColor.b,
                                       0.38))
            }
            GradientStop { position: 1; color: card.surfaceColor }
        }

        Rectangle {
            id: disc

            width: artwork.width * 0.72
            height: width
            radius: width / 2
            anchors.right: parent.right
            anchors.rightMargin: -width * 0.16
            anchors.verticalCenter: parent.verticalCenter
            color: Qt.darker(card.surfaceColor, 1.32)
            border.width: 1
            border.color: Qt.rgba(card.foregroundColor.r,
                                  card.foregroundColor.g,
                                  card.foregroundColor.b,
                                  0.08)

            Repeater {
                model: 7

                Rectangle {
                    required property int index

                    width: disc.width - 18 - index * 12
                    height: width
                    radius: width / 2
                    color: "transparent"
                    border.width: 1
                    border.color: Qt.rgba(card.foregroundColor.r,
                                          card.foregroundColor.g,
                                          card.foregroundColor.b,
                                          0.07)
                    anchors.centerIn: disc
                }
            }

            Rectangle {
                width: disc.width * 0.3
                height: width
                radius: width / 2
                color: card.accentColor
                anchors.centerIn: parent
            }
        }

        Text {
            anchors.left: parent.left
            anchors.bottom: parent.bottom
            anchors.leftMargin: 18
            anchors.bottomMargin: 10
            text: card.albumTitle.length > 0 ? card.albumTitle.slice(0, 1) : ""
            color: Qt.rgba(card.foregroundColor.r,
                           card.foregroundColor.g,
                           card.foregroundColor.b,
                           0.92)
            font.family: "Noto Sans CJK SC"
            font.pixelSize: Math.max(42, artwork.width * 0.28)
            font.weight: Font.Black
        }
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
