pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: vinyl

    property color discColor: "#11181c"
    property color grooveColor: "#405057"
    property color labelColor: "#b85f4a"
    property color labelTextColor: "#f4e9dc"

    implicitWidth: 360
    implicitHeight: 360

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        color: vinyl.discColor
        border.width: 1
        border.color: Qt.lighter(vinyl.discColor, 1.35)
    }

    Repeater {
        model: 11

        Rectangle {
            required property int index

            width: vinyl.width - 24 - index * 20
            height: width
            radius: width / 2
            color: "transparent"
            border.width: 1
            border.color: Qt.rgba(vinyl.grooveColor.r,
                                  vinyl.grooveColor.g,
                                  vinyl.grooveColor.b,
                                  0.22 + index * 0.012)
            anchors.centerIn: vinyl
        }
    }

    Rectangle {
        width: vinyl.width * 0.34
        height: width
        radius: width / 2
        color: vinyl.labelColor
        anchors.centerIn: parent

        Text {
            anchors.centerIn: parent
            text: qsTr("留声")
            color: vinyl.labelTextColor
            font.family: "Noto Sans CJK SC"
            font.pixelSize: Math.max(16, vinyl.width * 0.065)
            font.weight: Font.DemiBold
            font.letterSpacing: 4
        }

        Rectangle {
            width: 10
            height: 10
            radius: 5
            color: vinyl.discColor
            anchors.centerIn: parent
        }
    }

    Rectangle {
        width: parent.width * 0.12
        height: parent.height * 0.48
        radius: width / 2
        color: Qt.rgba(1, 1, 1, 0.045)
        rotation: 32
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.horizontalCenterOffset: -parent.width * 0.18
        anchors.verticalCenter: parent.verticalCenter
    }
}
