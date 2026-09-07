pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic

GridView {
    id: grid
    required property var controller
    required property var collectionModel
    property bool artists: false
    readonly property int preferredCover: Theme.compactGrid ? 208 : 256
    readonly property int columns: Math.max(1, Math.min(Math.ceil((width - 12 + gap) / (preferredCover + gap)), Math.floor((width - 12 + gap) / 184)))
    readonly property int coverWidth: Math.max(1, Math.min(preferredCover, cellWidth - gap))
    readonly property int gap: 24
    property alias savedY: grid.contentY
    model: collectionModel
    cellWidth: Math.floor(width / columns)
    cellHeight: coverWidth + 64
    bottomMargin: 16
    objectName: artists ? "artistGrid" : "albumGrid"
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    cacheBuffer: cellHeight
    keyNavigationEnabled: true
    currentIndex: -1
    highlightMoveDuration: 0
    function restoreScroll(value) {
        contentY = Math.max(originY, Math.min(originY + Math.max(0, contentHeight - height), value));
    }
    Keys.onReturnPressed: {
        if (currentItem)
            currentItem.open();
    }
    Keys.onEnterPressed: {
        if (currentItem)
            currentItem.open();
    }
    ScrollBar.vertical: QuietScrollBar {}
    delegate: Item {
        id: card
        required property var model
        required property int index
        readonly property bool inViewport: grid.visible && y + height >= grid.contentY && y <= grid.contentY + grid.height
        readonly property bool selected: grid.activeFocus && grid.currentIndex === index
        width: grid.cellWidth
        height: grid.cellHeight
        function open() {
            if (grid.artists)
                grid.controller.openArtist(model.sourceIndex);
            else
                grid.controller.openAlbum(model.sourceIndex);
        }
        function requestCover() {
            if (!inViewport)
                return;
            if (grid.artists)
                grid.controller.requestArtistCover(model.sourceIndex);
            else
                grid.controller.requestAlbumCover(model.sourceIndex);
        }
        onInViewportChanged: requestCover()
        Component.onCompleted: requestCover()
        Connections {
            target: grid.controller
            function onArtworkRevisionChanged() {
                card.requestCover();
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: model.rowTitle + (grid.artists ? "" : ", " + model.rowArtist)
        Accessible.onPressAction: open()
        CoverArt {
            id: cover
            width: grid.coverWidth
            height: width
            source: card.model.rowCover
            title: card.model.rowTitle
        }
        Rectangle {
            anchors.fill: cover
            color: "transparent"
            border.color: Theme.accent
            border.width: card.selected ? 2 : 0
        }
        Rectangle {
            anchors.fill: cover
            color: Theme.dark ? "#0cffffff" : "#10000000"
            visible: hover.hovered
        }
        Column {
            y: cover.height + 12
            width: cover.width
            spacing: 4
            Text {
                id: albumTitle
                height: 20
                width: parent.width
                textFormat: Text.PlainText
                text: card.model.rowTitle
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: Theme.bodySize
                font.weight: Font.Medium
                elide: Text.ElideRight
            }
            Text {
                id: albumSubtitle
                height: 18
                width: parent.width
                textFormat: Text.PlainText
                text: grid.artists ? qsTr("%1 张专辑 · %2 首").arg(card.model.rowAlbumCount).arg(card.model.rowTrackCount) : card.model.rowArtist
                color: Theme.secondary
                font.family: Theme.fontFamily
                font.pixelSize: Theme.captionSize
                elide: Text.ElideRight
            }
        }
        ToolTip {
            popupType: Popup.Item
            visible: hover.hovered && (albumTitle.truncated || albumSubtitle.truncated)
            delay: 700
            text: card.model.rowTitle + "\n" + (grid.artists ? albumSubtitle.text : card.model.rowArtist)
            width: Math.min(360, implicitWidth)
        }
        HoverHandler {
            id: hover
        }
        TapHandler {
            onTapped: {
                grid.currentIndex = card.index;
                grid.forceActiveFocus();
                card.open();
            }
        }
    }
}
