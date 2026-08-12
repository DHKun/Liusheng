import QtQuick
import QtQuick.Controls

TrackListRow {
    id: row

    property color dangerColor: "#b85f4a"
    signal removeRequested

    rightInset: 74

    Button {
        id: removeButton

        width: 58
        height: 30
        anchors.right: parent.right
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        text: qsTr("移除")
        focusPolicy: Qt.StrongFocus
        Accessible.name: qsTr("从队列移除 %1").arg(row.trackTitle)
        onClicked: row.removeRequested()

        contentItem: Text {
            text: removeButton.text
            color: removeButton.hovered || removeButton.activeFocus
                   ? row.dangerColor
                   : row.mutedColor
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 11
            font.weight: Font.Medium
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }

        background: Rectangle {
            color: removeButton.hovered
                   ? Qt.rgba(row.dangerColor.r, row.dangerColor.g, row.dangerColor.b, 0.1)
                   : "transparent"
            radius: 15
            border.width: removeButton.activeFocus ? 1 : 0
            border.color: row.dangerColor

            Behavior on color {
                ColorAnimation { duration: 120 }
            }
        }
    }
}
