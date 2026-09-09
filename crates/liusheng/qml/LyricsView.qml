pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: view
    required property var controller
    required property var colors
    property real positionMs: 0
    property bool displayed: visible
    property bool showSecondary: true
    property bool autoFollow: true
    property bool previousDark: colors.dark
    property int lyricSize: width >= 520 ? 32 : 27
    property real lastPosition: 0
    property var cues: []
    readonly property int activeIndex: cueAt(positionMs - controller.lyricsOffsetMs)
    readonly property bool instrumental: activeIndex >= 0 && cues[activeIndex].text.length === 0
    property alias listView: list

    function reload() {
        try {
            const parsed = JSON.parse(controller.lyricCuesJson || "[]");
            cues = Array.isArray(parsed) ? parsed : [];
        } catch (error) {
            cues = [];
        }
        autoFollow = true;
        scrollAnimation.stop();
        Qt.callLater(function () {
            follow(false);
        });
    }
    function cueAt(position) {
        if (cues.length === 0 || cues[0].time === null)
            return -1;
        let lo = 0, hi = cues.length;
        while (lo < hi) {
            const mid = (lo + hi) >>> 1;
            if (cues[mid].time <= position)
                lo = mid + 1;
            else
                hi = mid;
        }
        return lo - 1;
    }
    function suspendFollow() {
        scrollAnimation.stop();
        autoFollow = false;
    }
    function resumeFollow() {
        autoFollow = true;
        follow(true);
    }
    function follow(animated) {
        if (!displayed || !autoFollow || list.moving || list.count === 0)
            return;
        scrollAnimation.stop();
        const oldY = list.contentY;
        if (activeIndex < 0)
            list.positionViewAtBeginning();
        else
            list.positionViewAtIndex(activeIndex, ListView.Center);
        const targetY = list.contentY;
        if (animated && !Theme.reducedMotion && Math.abs(targetY - oldY) < list.height * 1.5) {
            list.contentY = oldY;
            scrollAnimation.from = oldY;
            scrollAnimation.to = targetY;
            scrollAnimation.start();
        }
    }
    function activate(index) {
        const cue = cues[index];
        if (cue && cue.time !== null && controller.seekable) {
            autoFollow = true;
            controller.seekTo(Math.max(0, cue.time + controller.lyricsOffsetMs));
        }
    }
    onActiveIndexChanged: follow(true)
    onShowSecondaryChanged: Qt.callLater(function () {
        follow(false);
    })
    onDisplayedChanged: {
        if (displayed)
            Qt.callLater(function () {
                follow(false);
            });
        else
            scrollAnimation.stop();
    }
    onWidthChanged: Qt.callLater(function () {
        follow(false);
    })
    onHeightChanged: Qt.callLater(function () {
        follow(false);
    })
    onPositionMsChanged: {
        if (Math.abs(positionMs - lastPosition) > 1500 || positionMs < lastPosition - 150) {
            autoFollow = true;
            Qt.callLater(function () {
                follow(false);
            });
        }
        lastPosition = positionMs;
    }
    Component.onCompleted: {
        previousDark = colors.dark;
        reload();
    }
    Connections {
        target: view.colors
        function onDarkChanged() {
            Qt.callLater(function () {
                view.previousDark = view.colors.dark;
            });
        }
    }
    Connections {
        target: view.controller
        function onLyricCuesJsonChanged() {
            view.reload();
        }
        function onCurrentTrackPathChanged() {
            view.autoFollow = true;
        }
    }
    NumberAnimation {
        id: scrollAnimation
        target: list
        property: "contentY"
        duration: 260
        easing.type: Easing.OutCubic
    }
    ListView {
        id: list
        objectName: "lyricsList"
        anchors.fill: parent
        anchors.bottomMargin: resume.visible ? 42 : 0
        model: view.cues
        clip: true
        spacing: 24
        reuseItems: true
        cacheBuffer: height
        boundsBehavior: Flickable.StopAtBounds
        currentIndex: -1
        highlightRangeMode: ListView.NoHighlightRange
        onMovementStarted: view.suspendFollow()
        onMovementEnded: {
            if (view.autoFollow)
                view.follow(false);
        }
        Keys.onUpPressed: {
            view.suspendFollow();
            decrementCurrentIndex();
        }
        Keys.onDownPressed: {
            view.suspendFollow();
            incrementCurrentIndex();
        }
        Keys.onReturnPressed: view.activate(currentIndex)
        Keys.onEnterPressed: view.activate(currentIndex)
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Home || event.key === Qt.Key_End) {
                view.suspendFollow();
                if (event.key === Qt.Key_Home)
                    list.positionViewAtBeginning();
                else
                    list.positionViewAtEnd();
                event.accepted = true;
            }
        }
        header: Item {
            width: list.width
            height: list.height * 0.34
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 24
                text: qsTr("前奏")
                visible: view.activeIndex < 0 && view.cues.length > 0 && view.cues[0].time !== null
                color: view.colors.secondary
                font.pixelSize: Theme.captionSize
                font.letterSpacing: 4
            }
        }
        footer: Item {
            height: list.height * 0.46
        }
        ScrollBar.vertical: QuietScrollBar {
            onPressedChanged: {
                if (pressed)
                    view.suspendFollow();
            }
        }
        delegate: ItemDelegate {
            id: row
            required property var modelData
            required property int index
            readonly property bool current: index === view.activeIndex
            readonly property bool keyboardSelected: list.activeFocus && list.currentIndex === index
            width: list.width - 12
            implicitHeight: words.implicitHeight + 20
            padding: 10
            focusPolicy: Qt.StrongFocus
            hoverEnabled: true
            Accessible.name: (modelData.text || qsTr("间奏")) + (modelData.secondary ? "\n" + modelData.secondary : "")
            background: Rectangle {
                radius: Theme.radius
                color: row.hovered ? view.colors.surface : "transparent"
                border.width: row.visualFocus || row.keyboardSelected ? 1 : 0
                border.color: view.colors.accent
            }
            contentItem: Column {
                id: words
                spacing: 10
                Text {
                    objectName: "lyricText"
                    width: parent.width
                    text: row.modelData.text || "·  ·  ·"
                    textFormat: Text.PlainText
                    color: row.current || (view.cues.length > 0 && view.cues[0].time === null) ? view.colors.text : view.colors.secondary
                    font.family: Theme.fontFamily
                    font.pixelSize: row.modelData.text ? view.lyricSize : 21
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    lineHeight: 1.12
                    Behavior on color {
                        enabled: !Theme.reducedMotion && view.previousDark === view.colors.dark
                        ColorAnimation {
                            duration: 140
                        }
                    }
                }
                Text {
                    objectName: "lyricSecondary"
                    width: parent.width
                    visible: view.showSecondary && text.length > 0
                    text: row.modelData.secondary
                    textFormat: Text.PlainText
                    color: view.colors.secondary
                    font.family: Theme.fontFamily
                    font.pixelSize: Math.max(16, view.lyricSize * 0.58)
                    lineHeight: 1.18
                    wrapMode: Text.Wrap
                }
            }
            onClicked: view.activate(index)
        }
    }
    QuietButton {
        id: resume
        objectName: "resumeLyricsFollow"
        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        text: qsTr("回到当前句")
        glyph: "down"
        visible: !view.autoFollow && view.activeIndex >= 0
        onClicked: view.resumeFollow()
    }
    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(340, parent.width - 32)
        spacing: 14
        visible: view.cues.length === 0
        Icon {
            name: "lyrics"
            color: view.colors.secondary
            size: 28
            Layout.alignment: Qt.AlignHCenter
        }
        Text {
            text: view.controller.lyricsLoading ? qsTr("正在读取歌词") : qsTr("让音乐自己说话")
            color: view.colors.text
            font.pixelSize: 21
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
        }
        Text {
            text: view.controller.lyricsError || qsTr("同名 LRC 与内嵌歌词会在这里显示。")
            textFormat: Text.PlainText
            color: view.colors.secondary
            font.pixelSize: Theme.captionSize
            wrapMode: Text.Wrap
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
        }
    }
}
