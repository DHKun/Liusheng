import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

TrackListRow {
    id: row

    property color dangerColor: "#b85f4a"
    signal removeRequested

    property bool canMoveUp: false
    property bool canMoveDown: false
    signal moveUpRequested
    signal moveDownRequested
    signal reorderRequested(int displacement)
    rightInset: 182
    Rectangle {
        width: 24; height: 36; color: "transparent"
        anchors.right: parent.right; anchors.rightMargin: 148; anchors.verticalCenter: parent.verticalCenter
        Text { anchors.centerIn: parent; text: "≡"; color: row.mutedColor; font.pixelSize: 20 }
        DragHandler {
            id: dragHandle
            target: null
            xAxis.enabled: false
            property real initialTranslation: 0
            property real latestDelta: 0
            onTranslationChanged: { if (active) latestDelta = translation.y - initialTranslation }
            onActiveChanged: {
                if (active) { initialTranslation = translation.y; latestDelta = 0 }
                else { const steps = Math.round(latestDelta / row.height); if (steps !== 0) row.reorderRequested(steps) }
            }
        }
        ToolTip.visible: dragHandle.active
        ToolTip.text: qsTr("移动 %1 行").arg(Math.round(dragHandle.latestDelta / row.height))
    }
    Row {
        anchors.right: parent.right; anchors.rightMargin: 76; anchors.verticalCenter: parent.verticalCenter; spacing: 2
        Button { width: 32; height: 30; text: "↑"; enabled: row.canMoveUp; Accessible.name: qsTr("上移曲目"); onClicked: row.moveUpRequested() }
        Button { width: 32; height: 30; text: "↓"; enabled: row.canMoveDown; Accessible.name: qsTr("下移曲目"); onClicked: row.moveDownRequested() }
    }

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
