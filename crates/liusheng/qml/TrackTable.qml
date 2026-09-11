pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: table
    required property var controller
    required property var trackModel
    property string mode: "tracks"
    property bool condensed: false
    property alias contentY: list.contentY
    readonly property int count: list.count
    readonly property bool queue: mode === "queue"
    readonly property bool columns: width >= 760 && !condensed
    property alias listView: list
    property int rowHeight: condensed ? 64 : Theme.rowHeight
    signal activated(int sourceIndex)
    function play(index) {
        if (queue)
            controller.playQueueTrack(index);
        else if (mode === "selected")
            controller.playSelectedTrack(index);
        else
            controller.playAllTrack(index);
        activated(index);
    }
    function enqueue(index, next) {
        if (mode === "selected")
            controller.enqueueSelectedTrack(index, next);
        else
            controller.enqueueAllTrack(index, next);
    }
    function restoreScroll(value) {
        list.contentY = Math.max(list.originY, Math.min(list.originY + Math.max(0, list.contentHeight - list.height), value));
    }
    RowLayout {
        id: heading
        visible: !table.condensed
        width: parent.width - 10
        height: 36
        spacing: 12
        Item {
            Layout.preferredWidth: table.queue ? 64 : 40
        }
        Text {
            text: qsTr("歌曲")
            color: Theme.muted
            font.pixelSize: Theme.captionSize
            Layout.fillWidth: true
        }
        Text {
            text: qsTr("艺术家")
            color: Theme.muted
            font.pixelSize: Theme.captionSize
            visible: table.columns
            Layout.preferredWidth: Math.floor(table.width * 0.19)
        }
        Text {
            text: qsTr("专辑")
            color: Theme.muted
            font.pixelSize: Theme.captionSize
            visible: table.columns
            Layout.preferredWidth: Math.floor(table.width * 0.22)
        }
        Icon {
            name: "clock"
            color: Theme.muted
            size: 13
            Layout.preferredWidth: 46
            Layout.alignment: Qt.AlignHCenter
        }
        Item {
            Layout.preferredWidth: 36
        }
    }
    Rectangle {
        anchors.bottom: heading.bottom
        width: parent.width
        height: 1
        color: Theme.line
        visible: heading.visible
    }
    ListView {
        id: list
        objectName: "trackTable"
        anchors.fill: parent
        anchors.topMargin: heading.visible ? heading.height + 6 : 0
        clip: true
        model: table.trackModel
        reuseItems: true
        boundsBehavior: Flickable.StopAtBounds
        currentIndex: -1
        keyNavigationEnabled: true
        keyNavigationWraps: false
        highlightMoveDuration: 0
        Keys.onReturnPressed: {
            if (currentItem)
                table.play(currentItem.model.sourceIndex);
        }
        Keys.onEnterPressed: {
            if (currentItem)
                table.play(currentItem.model.sourceIndex);
        }
        Keys.onPressed: event => {
            if (currentItem && (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && event.modifiers & Qt.ShiftModifier))) {
                currentItem.openActions();
                event.accepted = true;
            }
        }
        ScrollBar.vertical: QuietScrollBar {}
        delegate: Item {
            id: row
            required property var model
            required property int index
            readonly property bool current: table.queue ? model.sourceIndex === table.controller.currentQueueIndex : model.rowPath === table.controller.currentTrackPath
            readonly property bool selected: list.activeFocus && list.currentIndex === index
            readonly property bool actionsVisible: hover.hovered || selected || actions.opened
            width: list.width - 10
            height: table.rowHeight
            property alias actionMenu: actions
            function openActions() {
                actions.openBelow(more);
            }
            Accessible.role: Accessible.ListItem
            Accessible.name: model.rowTitle + ", " + model.rowArtist
            Accessible.onPressAction: table.play(model.sourceIndex)
            ListView.onPooled: actions.close()
            Rectangle {
                anchors.fill: parent
                radius: 5
                color: row.selected ? Theme.accentWash : hover.hovered ? Theme.subtle : "transparent"
                border.color: Theme.accent
                border.width: row.selected ? 1 : 0
            }
            HoverHandler {
                id: hover
            }
            TapHandler {
                acceptedButtons: Qt.LeftButton
                onTapped: {
                    list.currentIndex = row.index;
                    list.forceActiveFocus();
                }
                onDoubleTapped: table.play(row.model.sourceIndex)
            }
            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: point => actions.openAt(row, point.position.x, point.position.y)
            }
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && event.modifiers & Qt.ShiftModifier)) {
                    actions.openBelow(more);
                    event.accepted = true;
                }
            }
            RowLayout {
                anchors.fill: parent
                spacing: 12
                Item {
                    Layout.preferredWidth: table.queue ? 64 : 40
                    Layout.fillHeight: true
                    Text {
                        anchors.centerIn: parent
                        anchors.horizontalCenterOffset: table.queue ? 12 : 0
                        text: row.model.rowNumber
                        visible: !row.actionsVisible && !row.current
                        color: Theme.muted
                        font.pixelSize: Theme.captionSize
                    }
                    Icon {
                        anchors.centerIn: parent
                        anchors.horizontalCenterOffset: table.queue ? 12 : 0
                        name: "music"
                        color: Theme.accent
                        size: 15
                        visible: row.current && !row.actionsVisible
                    }
                    QuietButton {
                        anchors.centerIn: parent
                        anchors.horizontalCenterOffset: table.queue ? 12 : 0
                        glyph: "play"
                        compact: true
                        hint: qsTr("播放 %1").arg(row.model.rowTitle)
                        visible: row.actionsVisible
                        enabled: !table.controller.playbackInitializing
                        onClicked: table.play(row.model.sourceIndex)
                    }
                }
                Column {
                    Layout.fillWidth: true
                    spacing: 3
                    Text {
                        width: parent.width
                        textFormat: Text.PlainText
                        text: row.model.rowTitle
                        elide: Text.ElideRight
                        color: row.current ? Theme.accent : Theme.text
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.bodySize
                        font.weight: row.current ? Font.DemiBold : Font.Normal
                    }
                    Text {
                        width: parent.width
                        visible: !table.columns
                        textFormat: Text.PlainText
                        text: row.model.rowArtist
                        elide: Text.ElideRight
                        color: Theme.secondary
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.captionSize
                    }
                }
                Text {
                    visible: table.columns
                    Layout.preferredWidth: Math.floor(table.width * 0.19)
                    textFormat: Text.PlainText
                    text: row.model.rowArtist
                    elide: Text.ElideRight
                    color: Theme.secondary
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.labelSize
                }
                Text {
                    visible: table.columns
                    Layout.preferredWidth: Math.floor(table.width * 0.22)
                    textFormat: Text.PlainText
                    text: row.model.rowAlbum
                    elide: Text.ElideRight
                    color: Theme.secondary
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.labelSize
                }
                Text {
                    Layout.preferredWidth: 46
                    text: Theme.time(row.model.rowDuration)
                    color: Theme.muted
                    font.pixelSize: Theme.captionSize
                    horizontalAlignment: Text.AlignRight
                }
                QuietButton {
                    id: more
                    objectName: "trackMore"
                    glyph: "more"
                    compact: true
                    hint: qsTr("%1 的更多操作").arg(row.model.rowTitle)
                    Layout.preferredWidth: 36
                    opacity: row.actionsVisible ? 1 : 0.35
                    onClicked: actions.openBelow(more)
                }
            }
            QuietMenu {
                id: actions
                objectName: "trackActions"
                onClosed: list.forceActiveFocus()
                QuietMenuItem {
                    text: qsTr("查找在线封面…")
                    onTriggered: table.controller.requestOnlineDetails(table.mode, row.model.sourceIndex, "cover")
                }
                QuietMenuItem {
                    text: qsTr("查找在线歌词…")
                    onTriggered: table.controller.requestOnlineDetails(table.mode, row.model.sourceIndex, "lyrics")
                }
                QuietMenuItem {
                    text: qsTr("播放")
                    enabled: !table.controller.playbackInitializing
                    onTriggered: table.play(row.model.sourceIndex)
                }
                QuietMenuItem {
                    text: qsTr("下一首播放")
                    visible: !table.queue
                    height: visible ? implicitHeight : 0
                    onTriggered: table.enqueue(row.model.sourceIndex, true)
                }
                QuietMenuItem {
                    text: qsTr("加入播放队列")
                    visible: !table.queue
                    height: visible ? implicitHeight : 0
                    onTriggered: table.enqueue(row.model.sourceIndex, false)
                }
                QuietMenuItem {
                    text: qsTr("上移")
                    visible: table.queue
                    height: visible ? implicitHeight : 0
                    enabled: row.model.sourceIndex > 0
                    onTriggered: table.controller.moveQueueTrack(row.model.sourceIndex, row.model.sourceIndex - 1)
                }
                QuietMenuItem {
                    text: qsTr("下移")
                    visible: table.queue
                    height: visible ? implicitHeight : 0
                    enabled: row.model.sourceIndex + 1 < table.controller.queueCount
                    onTriggered: table.controller.moveQueueTrack(row.model.sourceIndex, row.model.sourceIndex + 1)
                }
                QuietMenuItem {
                    text: qsTr("从队列移除")
                    destructive: true
                    visible: table.queue
                    height: visible ? implicitHeight : 0
                    onTriggered: table.controller.removeQueueTrack(row.model.sourceIndex)
                }
            }
            // A dedicated handle keeps drag-to-reorder separate from track activation.
            MouseArea {
                visible: table.queue
                opacity: row.actionsVisible ? 1 : 0.35
                x: 0
                y: 0
                objectName: "queueDragHandle"
                width: 24
                height: row.height
                cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                preventStealing: true
                property real origin: 0
                property int sourceIndex: -1
                onPressed: mouse => {
                    origin = mouse.y;
                    sourceIndex = row.model.sourceIndex;
                }
                onReleased: mouse => {
                    const to = Math.max(0, Math.min(table.controller.queueCount - 1, sourceIndex + Math.round((mouse.y - origin) / row.height)));
                    if (sourceIndex !== to)
                        table.controller.moveQueueTrack(sourceIndex, to);
                }
                Icon {
                    anchors.centerIn: parent
                    name: "grip"
                    size: 12
                    color: Theme.muted
                }
            }
        }
    }
}
