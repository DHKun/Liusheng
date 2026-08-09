import QtQuick

Rectangle {
    id: row

    property string trackNumber
    property string trackTitle
    property string trackArtist
    property int durationMs
    property color foregroundColor: "#e8edf0"
    property color mutedColor: "#829198"

    function durationText() {
        const totalSeconds = Math.floor(durationMs / 1000)
        const minutes = Math.floor(totalSeconds / 60)
        const seconds = totalSeconds % 60
        return minutes + ":" + (seconds < 10 ? "0" : "") + seconds
    }

    color: "transparent"
    height: 62

    Text {
        width: 36
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        text: row.trackNumber
        color: row.mutedColor
        font.family: "JetBrains Mono"
        font.pixelSize: 11
    }

    Column {
        anchors.left: parent.left
        anchors.leftMargin: 48
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
            text: row.trackArtist
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
                       0.07)
    }
}
