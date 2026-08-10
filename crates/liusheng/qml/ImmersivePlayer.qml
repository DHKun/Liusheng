pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls

Item {
    id: immersive

    property color backgroundColor: "#0b1114"
    property color surfaceColor: "#151d22"
    property color foregroundColor: "#e8edf0"
    property color mutedColor: "#829198"
    property color accentColor: "#d9a15f"
    property color secondaryColor: "#6f9d99"
    property color warmColor: "#b85f4a"
    property string trackTitle
    property string trackArtist
    property string lyricsError
    property int positionMs
    property int durationMs
    property int lyricLineCount
    property int currentLyricIndex
    property int lyricsRevision
    property bool hasTrack: false
    property bool seekable: false
    property bool playing: false
    property bool lyricsLoading: false
    property bool lyricsSynced: false
    property bool motionEnabled: Application.styleHints.useHoverEffects
    property var lyricTextProvider: function(index) { return "" }
    property var lyricTimeProvider: function(index) { return -1 }

    signal closeRequested
    signal previousRequested
    signal toggleRequested
    signal nextRequested
    signal seekRequested(int positionMs)

    function timeText(milliseconds) {
        const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000))
        const minutes = Math.floor(totalSeconds / 60)
        const seconds = totalSeconds % 60
        return minutes + ":" + (seconds < 10 ? "0" : "") + seconds
    }

    function lyricText(index) {
        return lyricsRevision >= 0 ? lyricTextProvider(index) : ""
    }

    function lyricTime(index) {
        return lyricsRevision >= 0 ? lyricTimeProvider(index) : -1
    }

    focus: visible
    Keys.onEscapePressed: closeRequested()
    onVisibleChanged: {
        if (visible)
            closeButton.forceActiveFocus()
    }

    Rectangle {
        anchors.fill: parent
        color: immersive.backgroundColor

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0; color: immersive.backgroundColor }
            GradientStop {
                position: 0.44
                color: Qt.tint(immersive.backgroundColor,
                               Qt.rgba(immersive.warmColor.r,
                                       immersive.warmColor.g,
                                       immersive.warmColor.b,
                                       0.15))
            }
            GradientStop {
                position: 1
                color: Qt.tint(immersive.backgroundColor,
                               Qt.rgba(immersive.secondaryColor.r,
                                       immersive.secondaryColor.g,
                                       immersive.secondaryColor.b,
                                       0.12))
            }
        }
    }

    Rectangle {
        width: Math.min(parent.width * 0.48, parent.height * 0.86)
        height: width
        radius: width / 2
        anchors.left: parent.left
        anchors.leftMargin: -width * 0.3
        anchors.verticalCenter: parent.verticalCenter
        color: "transparent"
        border.width: Math.max(44, width * 0.15)
        border.color: Qt.rgba(immersive.warmColor.r,
                              immersive.warmColor.g,
                              immersive.warmColor.b,
                              0.055)
    }

    Button {
        id: closeButton

        anchors.left: parent.left
        anchors.top: parent.top
        anchors.leftMargin: 34
        anchors.topMargin: 28
        text: qsTr("返回曲库")
        focusPolicy: Qt.StrongFocus
        implicitWidth: 96
        implicitHeight: 38
        Accessible.name: qsTr("关闭沉浸播放页")
        onClicked: immersive.closeRequested()

        contentItem: Text {
            text: closeButton.text
            color: immersive.foregroundColor
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 12
            font.weight: Font.Medium
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }

        background: Rectangle {
            radius: 19
            color: closeButton.hovered
                   ? Qt.rgba(immersive.foregroundColor.r,
                             immersive.foregroundColor.g,
                             immersive.foregroundColor.b,
                             0.08)
                   : Qt.rgba(immersive.surfaceColor.r,
                             immersive.surfaceColor.g,
                             immersive.surfaceColor.b,
                             0.52)
            border.width: closeButton.activeFocus ? 2 : 1
            border.color: closeButton.activeFocus
                          ? immersive.accentColor
                          : Qt.rgba(immersive.foregroundColor.r,
                                    immersive.foregroundColor.g,
                                    immersive.foregroundColor.b,
                                    0.1)
        }
    }

    Text {
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.rightMargin: 38
        anchors.topMargin: 38
        text: immersive.lyricsSynced ? qsTr("同步歌词") : qsTr("本地歌词")
        color: immersive.mutedColor
        font.family: "JetBrains Mono"
        font.pixelSize: 10
        font.letterSpacing: 2
    }

    Item {
        id: recordPanel

        width: parent.width * 0.39
        anchors.left: parent.left
        anchors.top: closeButton.bottom
        anchors.bottom: parent.bottom
        anchors.leftMargin: 34
        anchors.topMargin: 18
        anchors.bottomMargin: 34

        Item {
            id: recordStage

            width: Math.min(parent.width * 0.82, parent.height * 0.52)
            height: width
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.topMargin: 4

            Rectangle {
                width: parent.width * 0.86
                height: width
                radius: width / 2
                anchors.centerIn: parent
                color: Qt.rgba(immersive.accentColor.r,
                               immersive.accentColor.g,
                               immersive.accentColor.b,
                               0.08)
            }

            VinylMark {
                id: spinningRecord

                width: parent.width * 0.76
                height: width
                anchors.centerIn: parent
                discColor: Qt.darker(immersive.surfaceColor, 1.3)
                grooveColor: immersive.mutedColor
                labelColor: immersive.warmColor
                labelTextColor: immersive.foregroundColor

                RotationAnimator on rotation {
                    from: 0
                    to: 360
                    duration: 22000
                    loops: Animation.Infinite
                    running: immersive.visible && immersive.playing && immersive.motionEnabled
                }
            }

            Rectangle {
                width: 10
                height: 10
                radius: 5
                anchors.centerIn: parent
                color: immersive.accentColor
            }
        }

        Column {
            id: trackMeta

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: recordStage.bottom
            anchors.topMargin: 12
            spacing: 5

            Text {
                width: parent.width
                text: immersive.trackTitle
                color: immersive.foregroundColor
                elide: Text.ElideRight
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 28
                font.weight: Font.Black
                font.letterSpacing: -1
            }

            Text {
                width: parent.width
                text: immersive.trackArtist
                color: immersive.mutedColor
                elide: Text.ElideRight
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 13
            }
        }

        Slider {
            id: progress

            property real pendingSeekMs: immersive.positionMs

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: trackMeta.bottom
            anchors.topMargin: 18
            from: 0
            to: Math.max(1, immersive.durationMs)
            enabled: immersive.seekable && immersive.durationMs > 0
            Accessible.name: qsTr("播放进度")
            onPressedChanged: {
                if (pressed) {
                    pendingSeekMs = value
                } else if (enabled) {
                    immersive.seekRequested(Math.round(pendingSeekMs))
                }
            }
            onMoved: pendingSeekMs = value

            Binding {
                target: progress
                property: "value"
                value: immersive.positionMs
                when: !progress.pressed
                restoreMode: Binding.RestoreBindingOrValue
            }

            background: Rectangle {
                x: progress.leftPadding
                y: progress.topPadding + progress.availableHeight / 2 - height / 2
                width: progress.availableWidth
                height: 3
                radius: 2
                color: Qt.rgba(immersive.foregroundColor.r,
                               immersive.foregroundColor.g,
                               immersive.foregroundColor.b,
                               0.12)

                Rectangle {
                    width: progress.visualPosition * parent.width
                    height: parent.height
                    radius: parent.radius
                    color: immersive.accentColor
                }
            }

            handle: Rectangle {
                x: progress.leftPadding
                   + progress.visualPosition * (progress.availableWidth - width)
                y: progress.topPadding + progress.availableHeight / 2 - height / 2
                width: progress.pressed || progress.hovered ? 13 : 9
                height: width
                radius: width / 2
                color: immersive.accentColor

                Behavior on width {
                    NumberAnimation {
                        duration: immersive.motionEnabled ? 140 : 0
                        easing.type: Easing.OutCubic
                    }
                }
            }
        }

        Text {
            anchors.left: parent.left
            anchors.top: progress.bottom
            text: immersive.timeText(immersive.positionMs)
            color: immersive.mutedColor
            font.family: "JetBrains Mono"
            font.pixelSize: 10
        }

        Text {
            anchors.right: parent.right
            anchors.top: progress.bottom
            text: immersive.timeText(immersive.durationMs)
            color: immersive.mutedColor
            font.family: "JetBrains Mono"
            font.pixelSize: 10
        }

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            spacing: 10

            Repeater {
                model: [qsTr("上一曲"), immersive.playing ? qsTr("暂停") : qsTr("播放"), qsTr("下一曲")]

                Button {
                    id: transport

                    required property int index
                    required property string modelData

                    text: modelData
                    enabled: immersive.hasTrack
                    focusPolicy: Qt.StrongFocus
                    implicitWidth: index === 1 ? 78 : 68
                    implicitHeight: 40
                    opacity: enabled ? 1 : 0.4
                    onClicked: {
                        if (index === 0)
                            immersive.previousRequested()
                        else if (index === 1)
                            immersive.toggleRequested()
                        else
                            immersive.nextRequested()
                    }

                    contentItem: Text {
                        text: transport.text
                        color: immersive.foregroundColor
                        font.family: "Noto Sans CJK SC"
                        font.pixelSize: 12
                        font.weight: transport.index === 1 ? Font.DemiBold : Font.Normal
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }

                    background: Rectangle {
                        radius: 20
                        color: transport.index === 1
                               ? Qt.rgba(immersive.accentColor.r,
                                         immersive.accentColor.g,
                                         immersive.accentColor.b,
                                         transport.hovered ? 0.3 : 0.2)
                               : transport.hovered
                                 ? Qt.rgba(immersive.foregroundColor.r,
                                           immersive.foregroundColor.g,
                                           immersive.foregroundColor.b,
                                           0.07)
                                 : "transparent"
                        border.width: transport.activeFocus ? 2 : 1
                        border.color: transport.activeFocus
                                      ? immersive.accentColor
                                      : Qt.rgba(immersive.foregroundColor.r,
                                                immersive.foregroundColor.g,
                                                immersive.foregroundColor.b,
                                                0.12)
                    }
                }
            }
        }
    }

    Item {
        id: lyricsPanel

        anchors.left: recordPanel.right
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.leftMargin: 42
        anchors.rightMargin: 36
        anchors.topMargin: 72
        anchors.bottomMargin: 28
        clip: true

        Rectangle {
            visible: immersive.lyricsSynced && immersive.lyricLineCount > 0
            width: parent.width
            height: 1
            y: parent.height * 0.46
            color: Qt.rgba(immersive.accentColor.r,
                           immersive.accentColor.g,
                           immersive.accentColor.b,
                           0.32)

            Rectangle {
                width: 44
                height: 3
                radius: 2
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                color: immersive.accentColor
            }
        }

        ListView {
            id: lyricsView

            visible: immersive.lyricLineCount > 0
            anchors.fill: parent
            clip: true
            model: immersive.lyricLineCount
            currentIndex: immersive.lyricsSynced ? immersive.currentLyricIndex : -1
            boundsBehavior: Flickable.StopAtBounds
            preferredHighlightBegin: height * 0.41
            preferredHighlightEnd: height * 0.51
            highlightRangeMode: immersive.lyricsSynced && currentIndex >= 0
                                ? ListView.StrictlyEnforceRange
                                : ListView.NoHighlightRange
            highlightMoveDuration: immersive.motionEnabled ? 680 : 0
            highlightResizeDuration: immersive.motionEnabled ? 360 : 0
            header: Item { width: lyricsView.width; height: lyricsView.height * 0.4 }
            footer: Item { width: lyricsView.width; height: lyricsView.height * 0.4 }

            delegate: Item {
                id: lyricRow

                required property int index
                readonly property bool active: immersive.lyricsSynced
                                                    && index === immersive.currentLyricIndex
                readonly property int distance: immersive.currentLyricIndex >= 0
                                                ? Math.abs(index - immersive.currentLyricIndex)
                                                : 0
                readonly property int timestamp: immersive.lyricTime(index)

                width: lyricsView.width
                height: Math.max(64, lyricText.implicitHeight + 28)
                opacity: active ? 1 : Math.max(0.26, 0.72 - distance * 0.1)
                scale: active ? 1 : 0.965
                transformOrigin: Item.Left
                Accessible.role: timestamp >= 0 && immersive.seekable
                                 ? Accessible.Button
                                 : Accessible.StaticText
                Accessible.name: lyricText.text

                Behavior on opacity {
                    NumberAnimation {
                        duration: immersive.motionEnabled ? 260 : 0
                        easing.type: Easing.OutCubic
                    }
                }

                Behavior on scale {
                    enabled: immersive.motionEnabled
                    SpringAnimation {
                        spring: 3.2
                        damping: 0.32
                        epsilon: 0.002
                    }
                }

                Text {
                    id: lyricText

                    width: parent.width - 24
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    text: immersive.lyricText(lyricRow.index)
                    color: lyricRow.active
                           ? immersive.foregroundColor
                           : immersive.mutedColor
                    wrapMode: Text.Wrap
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: lyricRow.active ? 27 : 22
                    font.weight: lyricRow.active ? Font.Black : Font.DemiBold
                    lineHeight: 1.22

                    Behavior on color {
                        ColorAnimation { duration: immersive.motionEnabled ? 240 : 0 }
                    }

                    Behavior on font.pixelSize {
                        NumberAnimation {
                            duration: immersive.motionEnabled ? 300 : 0
                            easing.type: Easing.OutCubic
                        }
                    }
                }

                TapHandler {
                    enabled: lyricRow.timestamp >= 0 && immersive.seekable
                    onTapped: immersive.seekRequested(lyricRow.timestamp)
                }
            }

            ScrollBar.vertical: ScrollBar {
                policy: immersive.lyricsSynced ? ScrollBar.AlwaysOff : ScrollBar.AsNeeded
            }
        }

        Column {
            visible: immersive.lyricLineCount === 0
            width: Math.min(parent.width * 0.72, 420)
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10

            Text {
                width: parent.width
                text: immersive.lyricsLoading
                      ? qsTr("正在读取歌词")
                      : immersive.lyricsError.length > 0
                        ? immersive.lyricsError
                        : qsTr("未找到本地歌词")
                color: immersive.foregroundColor
                wrapMode: Text.WordWrap
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 28
                font.weight: Font.Black
            }

            Text {
                visible: !immersive.lyricsLoading
                width: parent.width
                text: immersive.lyricsError.length > 0
                      ? qsTr("检查歌词文件编码和读取权限。")
                      : qsTr("将同名 LRC 文件放到音频所在目录。")
                color: immersive.mutedColor
                wrapMode: Text.WordWrap
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 13
            }
        }
    }
}
