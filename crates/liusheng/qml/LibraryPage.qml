pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import io.github.dhkun.Liusheng 1.0

Item {
    id: page
    required property var controller
    property string section: "albums"
    property bool artistsVisited: false
    property bool tracksVisited: false
    property bool detailsVisited: false
    property bool changingSection: false
    property string albumSearch: ""
    property string artistSearch: ""
    readonly property bool detailOpen: section === "albums" ? controller.albumOpen : section === "artists" && controller.artistOpen
    readonly property string heading: section === "albums" ? qsTr("专辑") : section === "artists" ? qsTr("艺术家") : qsTr("歌曲")
    readonly property real albumScroll: albums.contentY
    readonly property real trackScroll: songLoader.item ? songLoader.item.contentY : 0
    property real pendingTrackScroll: 0
    signal checkUpdatesRequested
    signal settingsRequested
    signal filesRequested
    signal quitRequested
    onSectionChanged: {
        changingSection = true;
        if (section === "artists")
            artistsVisited = true;
        if (section === "allTracks")
            tracksVisited = true;
        search.text = section === "albums" ? albumSearch : section === "artists" ? artistSearch : controller.trackFilter;
        searchTimer.stop();
        changingSection = false;
    }
    onDetailOpenChanged: {
        if (detailOpen)
            detailsVisited = true;
    }
    function focusSearch() {
        controller.closeAlbum();
        controller.closeArtist();
        Qt.callLater(function () {
            search.forceActiveFocus();
            search.selectAll();
        });
    }
    function searchFor(query) {
        search.text = query;
        searchTimer.stop();
        applySearch();
    }
    property alias moreMenu: libraryMenu
    property alias filtersPopup: filters
    property alias sortControl: sortBox
    property alias formatControl: formatBox
    property alias albumView: albums
    property alias detailView: details
    property alias songView: songLoader
    property alias searchControl: search
    readonly property int resultCount: section === "albums" ? albums.count : section === "artists" ? (artistLoader.item ? artistLoader.item.count : 0) : controller.visibleTrackCount
    function restoreScroll(albumY, trackY) {
        albums.restoreScroll(albumY);
        pendingTrackScroll = trackY;
        if (songLoader.item)
            songLoader.item.restoreScroll(trackY);
    }
    function applySearch() {
        if (section === "albums") {
            albumSearch = search.text;
            albumModel.query = search.text;
        } else if (section === "artists") {
            artistSearch = search.text;
            artistModel.query = search.text;
        } else
            controller.filterTracks(search.text);
    }
    UiModel {
        id: albumModel
        kind: "albums"
        onQueryChanged: refresh()
        Component.onCompleted: refresh()
    }
    UiModel {
        id: artistModel
        kind: "artists"
        onQueryChanged: refresh()
        Component.onCompleted: refresh()
    }
    UiModel {
        id: songModel
        kind: "tracks"
        Component.onCompleted: refresh()
    }
    Connections {
        target: page.controller
        function onLibraryRevisionChanged() {
            albumModel.refresh();
            artistModel.refresh();
            songModel.refresh();
        }
        function onArtworkRevisionChanged() {
            albumModel.refresh();
            artistModel.refresh();
        }
    }
    RowLayout {
        id: toolbar
        width: parent.width
        height: 64
        spacing: 12
        QuietButton {
            glyph: "back"
            hint: qsTr("返回 %1").arg(page.heading)
            visible: page.detailOpen
            onClicked: {
                page.controller.closeAlbum();
                page.controller.closeArtist();
            }
        }
        Column {
            spacing: 5
            Layout.fillWidth: true
            Text {
                text: page.heading
                font.family: Theme.fontFamily
                font.pixelSize: page.detailOpen ? 18 : 26
                font.weight: Font.DemiBold
                color: Theme.text
            }
            Text {
                visible: !page.detailOpen
                font.pixelSize: Theme.noteSize
                color: Theme.secondary
                text: page.section === "albums" ? qsTr("%1 张专辑").arg(page.controller.albumCount) : page.section === "artists" ? qsTr("%1 位艺术家").arg(page.controller.artistCount) : qsTr("%1 首歌曲").arg(page.controller.trackCount)
            }
        }
        QuietField {
            id: search
            objectName: "librarySearch"
            visible: !page.detailOpen
            Layout.preferredWidth: page.width > 720 ? 240 : 190
            Layout.fillWidth: false
            search: true
            placeholderText: qsTr("搜索%1").arg(page.heading)
            onTextEdited: searchTimer.restart()
            onTextChanged: {
                if (text.length === 0 && !page.changingSection) {
                    searchTimer.stop();
                    page.applySearch();
                }
            }
            onAccepted: {
                searchTimer.stop();
                page.applySearch();
            }
            Keys.onEscapePressed: {
                clear();
                focus = false;
            }
        }
        QuietButton {
            id: filterButton
            objectName: "filterButton"
            glyph: "filter"
            hint: qsTr("排序与格式筛选")
            visible: page.section === "allTracks" && !page.detailOpen
            selected: filters.opened
            onClicked: filters.openBelow(filterButton)
            QuietPopover {
                id: filters
                width: 260
                padding: 20
                implicitHeight: filterOptions.implicitHeight + 40

                ColumnLayout {
                    id: filterOptions
                    width: parent.width
                    spacing: 12
                    Text {
                        text: qsTr("排序与筛选")
                        color: Theme.text
                        font.pixelSize: Theme.bodySize
                        font.weight: Font.Medium
                    }
                    QuietComboBox {
                        id: sortBox
                        Layout.fillWidth: true
                        model: [qsTr("专辑顺序"), qsTr("歌曲名称"), qsTr("艺术家"), qsTr("发行年份"), qsTr("时长")]
                        currentIndex: page.controller.sortOrder
                        onActivated: page.controller.sortTracks(currentIndex, formatBox.currentIndex === 0 ? "" : formatBox.currentText)
                    }
                    QuietComboBox {
                        id: formatBox
                        Layout.fillWidth: true
                        model: [qsTr("所有格式"), "flac", "mp3", "m4a", "ogg", "wav", "aiff"]
                        onActivated: page.controller.sortTracks(sortBox.currentIndex, currentIndex === 0 ? "" : currentText)
                    }
                }
            }
        }
        QuietButton {
            id: libraryMore
            objectName: "libraryMore"
            glyph: "more"
            hint: qsTr("曲库操作")
            onClicked: libraryMenu.openBelow(libraryMore)
            QuietMenu {
                id: libraryMenu
                objectName: "libraryMenu"
                QuietMenuItem {
                    text: qsTr("打开文件…")
                    onTriggered: page.filesRequested()
                }
                QuietMenuItem {
                    objectName: "scanLibraryAction"
                    text: page.controller.scanning ? qsTr("取消扫描") : qsTr("重新扫描曲库")
                    onTriggered: page.controller.scanning ? page.controller.cancelScan() : page.controller.scanLibrary()
                }
                QuietMenuItem {
                    text: qsTr("检查更新…")
                    onTriggered: page.checkUpdatesRequested()
                }
                QuietMenuItem {
                    text: qsTr("管理音乐目录…")
                    onTriggered: page.settingsRequested()
                }
                QuietMenuItem {
                    text: qsTr("退出留声")
                    onTriggered: page.quitRequested()
                }
            }
        }
    }
    Timer {
        id: searchTimer
        interval: 180
        onTriggered: page.applySearch()
    }
    Item {
        id: body
        anchors.top: toolbar.bottom
        anchors.topMargin: 24
        anchors.bottom: parent.bottom
        width: parent.width
        CollectionView {
            id: albums
            anchors.fill: parent
            controller: page.controller
            collectionModel: albumModel
            visible: page.section === "albums" && !page.detailOpen
        }
        Loader {
            id: artistLoader
            anchors.fill: parent
            active: page.artistsVisited
            visible: page.section === "artists" && !page.detailOpen
            sourceComponent: Component {
                CollectionView {
                    controller: page.controller
                    collectionModel: artistModel
                    artists: true
                }
            }
        }
        Loader {
            id: songLoader
            anchors.fill: parent
            active: page.tracksVisited
            visible: page.section === "allTracks"
            sourceComponent: Component {
                TrackTable {
                    controller: page.controller
                    trackModel: songModel
                }
            }
            onLoaded: Qt.callLater(function () {
                if (songLoader.item)
                    songLoader.item.restoreScroll(page.pendingTrackScroll);
            })
        }
        Loader {
            id: details
            anchors.fill: parent
            active: page.detailsVisited
            visible: page.detailOpen
            sourceComponent: Component {
                DetailPage {
                    controller: page.controller
                    artist: page.section === "artists"
                }
            }
        }
        ColumnLayout {
            anchors.centerIn: parent
            width: Math.min(380, parent.width - 48)
            spacing: 16
            visible: !page.detailOpen && (page.section === "albums" ? albums.count === 0 : page.section === "artists" ? !artistLoader.item || artistLoader.item.count === 0 : page.controller.visibleTrackCount === 0)
            Icon {
                name: search.text.length ? "search" : "disc"
                size: 36
                color: Theme.muted
                Layout.alignment: Qt.AlignHCenter
            }
            Text {
                text: search.text.length ? qsTr("没有匹配的音乐") : page.controller.scanning ? qsTr("正在整理你的音乐") : qsTr("把喜欢的音乐留在这里")
                color: Theme.text
                font.pixelSize: Theme.headingSize
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            Text {
                text: search.text.length ? qsTr("试试歌曲名、艺术家或拼音。") : page.controller.scanning ? qsTr("曲库将随着扫描逐步显示。") : qsTr("添加本地音乐目录，开始浏览专辑与歌曲。")
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
            }
            QuietButton {
                text: search.text.length ? qsTr("清除搜索") : qsTr("添加音乐目录")
                primary: true
                visible: !page.controller.scanning
                Layout.alignment: Qt.AlignHCenter
                onClicked: {
                    if (search.text.length)
                        search.clear();
                    else
                        page.settingsRequested();
                }
            }
        }
    }
}
