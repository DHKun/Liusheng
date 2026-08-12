import QtQuick

Rectangle {
    id: row

    property string trackNumber
    property string trackTitle
    property string trackArtist
    property string trackAlbum
    property int durationMs
    property color foregroundColor: "#e8edf0"
    property color mutedColor: "#829198"
    property color accentColor: "#d9a15f"
    property bool current: false
    property bool interactive: true
    property int rightInset: 0

    signal activated

    function durationText() {
        const totalSeconds = Math.floor(durationMs / 1000)
        const minutes = Math.floor(totalSeconds / 60)
        const seconds = totalSeconds % 60
        return minutes + ":" + (seconds < 10 ? "0" : "") + seconds
    }

    color: current
           ? Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.12)
           : hoverHandler.hovered
             ? Qt.rgba(foregroundColor.r, foregroundColor.g, foregroundColor.b, 0.045)
             : "transparent"
    height: 62
    radius: 8
    activeFocusOnTab: interactive || activeFocus
    Accessible.role: Accessible.Button
    Accessible.name: qsTr("播放 %1").arg(trackTitle)
    Accessible.ignored: !interactive
    Keys.onReturnPressed: {
        if (row.interactive)
            row.activated()
    }
    Keys.onEnterPressed: {
        if (row.interactive)
            row.activated()
    }

    Behavior on color {
        ColorAnimation { duration: 120 }
    }

    HoverHandler {
        id: hoverHandler
        enabled: row.interactive
    }

    TapHandler {
        enabled: row.interactive
        onTapped: {
            row.forceActiveFocus()
            row.activated()
        }
    }

    Rectangle {
        visible: row.current
        width: 3
        height: 28
        radius: 2
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        color: row.accentColor
    }

    Text {
        width: 36
        anchors.left: parent.left
        anchors.leftMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        text: row.trackNumber
        color: row.mutedColor
        font.family: "JetBrains Mono"
        font.pixelSize: 11
    }

    Column {
        anchors.left: parent.left
        anchors.leftMargin: 52
        anchors.right: duration.left
        anchors.rightMargin: 20
        anchors.verticalCenter: parent.verticalCenter
        spacing: 3

        Text {
            width: parent.width
            text: row.trackTitle
            color: row.foregroundColor
            elide: Text.ElideRight
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 13
            font.weight: Font.Medium
        }

        Text {
            width: parent.width
            text: row.trackAlbum.length > 0
                  ? qsTr("%1，%2").arg(row.trackArtist).arg(row.trackAlbum)
                  : row.trackArtist
            color: row.mutedColor
            elide: Text.ElideRight
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 11
        }
    }

    Text {
        id: duration

        width: 46
        anchors.right: parent.right
        anchors.rightMargin: row.rightInset
        anchors.verticalCenter: parent.verticalCenter
        text: row.durationText()
        color: row.mutedColor
        horizontalAlignment: Text.AlignRight
        font.family: "JetBrains Mono"
        font.pixelSize: 10
    }

    Rectangle {
        height: 1
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        color: Qt.rgba(row.foregroundColor.r,
                       row.foregroundColor.g,
                       row.foregroundColor.b,
                       row.current ? 0 : 0.07)
    }
}
