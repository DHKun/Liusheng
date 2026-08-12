pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: row

    property string sequence
    property string artistName
    property url coverSource
    property int trackCount
    property int albumCount
    property color backgroundColor: "#0b1114"
    property color surfaceColor: "#151d22"
    property color foregroundColor: "#e8edf0"
    property color mutedColor: "#829198"
    property color accentColor: "#d9a15f"
    readonly property color activeSurface: hoverHandler.hovered
                                           ? Qt.rgba(foregroundColor.r,
                                                     foregroundColor.g,
                                                     foregroundColor.b,
                                                     0.045)
                                           : "transparent"

    signal activated

    height: 108
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: qsTr("打开艺术家 %1").arg(artistName)
    Keys.onReturnPressed: row.activated()
    Keys.onEnterPressed: row.activated()

    HoverHandler {
        id: hoverHandler
    }

    TapHandler {
        onTapped: {
            row.forceActiveFocus()
            row.activated()
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: 14
        color: row.activeSurface
        border.width: row.activeFocus ? 1 : 0
        border.color: row.accentColor

        Behavior on color {
            ColorAnimation { duration: 160; easing.type: Easing.OutCubic }
        }
    }

    Text {
        width: 52
        anchors.left: parent.left
        anchors.leftMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        text: row.sequence
        color: Qt.rgba(row.mutedColor.r,
                       row.mutedColor.g,
                       row.mutedColor.b,
                       0.7)
        font.family: "JetBrains Mono"
        font.pixelSize: 22
        font.weight: Font.Light
    }

    CoverArt {
        id: portrait

        width: 72
        height: 72
        anchors.left: parent.left
        anchors.leftMargin: 70
        anchors.verticalCenter: parent.verticalCenter
        source: row.coverSource
        title: row.artistName
        surfaceColor: row.surfaceColor
        foregroundColor: row.foregroundColor
        accentColor: row.accentColor
        surroundingColor: row.backgroundColor
        cornerRadius: width / 2
        frameWidth: 1
        frameColor: Qt.rgba(row.foregroundColor.r,
                            row.foregroundColor.g,
                            row.foregroundColor.b,
                            0.1)
        scale: hoverHandler.hovered && Application.styleHints.useHoverEffects ? 1.045 : 1

        Behavior on scale {
            NumberAnimation { duration: 240; easing.type: Easing.OutBack }
        }
    }

    Column {
        anchors.left: portrait.right
        anchors.leftMargin: 22
        anchors.right: count.left
        anchors.rightMargin: 28
        anchors.verticalCenter: parent.verticalCenter
        spacing: 7

        Text {
            width: parent.width
            text: row.artistName
            color: row.foregroundColor
            elide: Text.ElideRight
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 23
            font.weight: Font.Bold
            font.letterSpacing: -0.4
        }

        Text {
            width: parent.width
            text: qsTr("%1 张专辑").arg(row.albumCount)
            color: row.mutedColor
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 11
        }
    }

    Text {
        id: count

        width: 84
        anchors.right: parent.right
        anchors.rightMargin: 18
        anchors.verticalCenter: parent.verticalCenter
        text: qsTr("%1 首").arg(row.trackCount)
        color: row.accentColor
        horizontalAlignment: Text.AlignRight
        font.family: "JetBrains Mono"
        font.pixelSize: 12
        font.weight: Font.DemiBold
    }

    Rectangle {
        height: 1
        anchors.left: portrait.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        color: Qt.rgba(row.foregroundColor.r,
                       row.foregroundColor.g,
                       row.foregroundColor.b,
                       0.07)
    }
}
