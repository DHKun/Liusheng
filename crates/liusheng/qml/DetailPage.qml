pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import io.github.dhkun.Liusheng 1.0

Item {
    id: page
    required property var controller
    property bool artist: false
    readonly property int selection: artist ? controller.selectedArtistIndex : controller.selectedAlbumIndex
    readonly property string name: {
        controller.libraryRevision;
        return artist ? controller.artistName(selection) : controller.albumTitle(selection);
    }
    readonly property string subtitle: {
        controller.libraryRevision;
        return artist ? qsTr("%1 张专辑").arg(controller.artistAlbumCount(selection)) : controller.albumArtist(selection);
    }
    readonly property url artwork: {
        controller.artworkRevision;
        return artist ? controller.artistCoverUrl(selection) : controller.albumCoverUrl(selection);
    }
    readonly property int year: {
        controller.libraryRevision;
        return artist ? 0 : controller.albumYear(selection);
    }
    UiModel {
        id: tracks
        kind: "selected"
        Component.onCompleted: refresh()
    }
    Connections {
        target: page.controller
        function onLibraryRevisionChanged() {
            tracks.refresh();
        }
    }
    RowLayout {
        id: hero
        width: parent.width
        height: Math.max(page.height < 460 ? 144 : 188, detailText.implicitHeight)
        spacing: 24
        CoverArt {
            Layout.preferredWidth: page.height < 460 ? 144 : 188
            Layout.preferredHeight: page.height < 460 ? 144 : 188
            source: page.artwork
            title: page.name
            resolution: 768
        }
        ColumnLayout {
            id: detailText
            spacing: 8
            Layout.fillWidth: true
            Text {
                text: page.artist ? qsTr("艺术家") : qsTr("专辑")
                color: Theme.muted
                font.pixelSize: Theme.captionSize
            }
            Text {
                textFormat: Text.PlainText
                text: page.name
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: page.width < 750 ? 24 : 30
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
                maximumLineCount: 2
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                textFormat: Text.PlainText
                text: page.subtitle + (page.year > 0 ? " · " + page.year : "") + " · " + qsTr("%1 首").arg(page.controller.selectedTrackCount)
                color: Theme.secondary
                font.pixelSize: Theme.labelSize
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
            QuietButton {
                text: qsTr("查找专辑封面…")
                compact: true
                visible: !page.artist
                enabled: page.controller.selectedTrackCount > 0
                onClicked: page.controller.requestOnlineDetails("album", 0, "cover")
            }
            QuietButton {
                text: qsTr("播放")
                glyph: "play"
                primary: true
                Layout.topMargin: 8
                enabled: page.controller.selectedTrackCount > 0 && !page.controller.playbackInitializing
                onClicked: page.controller.playSelectedTrack(0)
            }
        }
    }
    TrackTable {
        anchors.top: hero.bottom
        anchors.topMargin: 24
        anchors.bottom: parent.bottom
        width: parent.width
        controller: page.controller
        trackModel: tracks
        mode: "selected"
    }
}
