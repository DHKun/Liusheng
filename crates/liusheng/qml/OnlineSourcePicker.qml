import QtQuick

QuietComboBox {
    id: picker
    property string kind: "lyrics"
    readonly property var sourceIds: kind === "cover" ? ["all", "qqmusic", "netease", "deezer", "primary"] : ["all", "qqmusic", "netease", "primary"]
    readonly property string sourceId: sourceIds[currentIndex] || "all"
    model: kind === "cover" ? [qsTr("全部来源"), qsTr("QQ 音乐"), qsTr("网易云音乐"), "Deezer", "MusicBrainz"] : [qsTr("全部来源"), qsTr("QQ 音乐"), qsTr("网易云音乐"), "LRCLIB"]
    Accessible.name: qsTr("资料来源")
    onKindChanged: currentIndex = 0
}
