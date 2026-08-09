import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import io.github.dhkun.Liusheng 1.0

ApplicationWindow {
    id: root

    readonly property bool darkMode: palette.window.hslLightness < 0.5
    readonly property color ink: darkMode ? "#0b1114" : "#eef2f3"
    readonly property color graphite: darkMode ? "#151d22" : "#ffffff"
    readonly property color fog: darkMode ? "#e8edf0" : "#172126"
    readonly property color muted: darkMode ? "#829198" : "#607078"
    readonly property color amber: darkMode ? "#d9a15f" : "#a56422"
    readonly property color rust: darkMode ? "#b85f4a" : "#a64736"
    readonly property color teal: darkMode ? "#6f9d99" : "#3f7773"

    width: 1240
    height: 760
    minimumWidth: 960
    minimumHeight: 620
    visible: true
    title: qsTr("留声")
    color: ink

    AppController {
        id: controller
    }

    Component.onCompleted: {
        Application.name = "Liusheng"
        Application.displayName = qsTr("留声")
        Application.version = "0.1.0"
        root.raise()
        root.requestActivate()
        if (Application.arguments.indexOf("--smoke-test") >= 0) {
            Qt.callLater(Qt.quit)
        } else {
            controller.scanLibrary()
        }
    }

    Rectangle {
        anchors.fill: parent
        color: root.ink

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: root.ink }
            GradientStop {
                position: 0.72
                color: Qt.tint(root.ink,
                               Qt.rgba(root.teal.r, root.teal.g, root.teal.b, 0.08))
            }
            GradientStop {
                position: 1.0
                color: Qt.tint(root.ink,
                               Qt.rgba(root.rust.r, root.rust.g, root.rust.b, 0.1))
            }
        }
    }

    Rectangle {
        id: sidebar
        width: 224
        anchors.top: parent.top
        anchors.bottom: playerBar.top
        anchors.left: parent.left
        color: Qt.rgba(root.graphite.r, root.graphite.g, root.graphite.b, 0.72)
        border.width: 1
        border.color: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 0

            Text {
                text: qsTr("留声")
                color: root.fog
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 26
                font.weight: Font.Bold
                font.letterSpacing: 6
                Layout.bottomMargin: 8
            }

            Text {
                text: qsTr("本地曲库")
                color: root.muted
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 11
                font.letterSpacing: 2
                Layout.bottomMargin: 42
            }

            NavButton {
                text: qsTr("专辑")
                selected: true
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
            }
            NavButton {
                text: qsTr("艺术家")
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
            }
            NavButton {
                text: qsTr("全部歌曲")
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
            }
            NavButton {
                text: qsTr("歌单")
                accentColor: root.amber
                foregroundColor: root.fog
                hoverColor: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.06)
            }

            Item { Layout.fillHeight: true }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 74
                radius: 14
                color: Qt.rgba(root.teal.r, root.teal.g, root.teal.b, 0.1)
                border.width: 1
                border.color: Qt.rgba(root.teal.r, root.teal.g, root.teal.b, 0.18)

                Column {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    spacing: 5

                    Text {
                        text: controller.status
                        color: root.fog
                        font.family: "Noto Sans CJK SC"
                        font.pixelSize: 12
                        font.weight: Font.Medium
                    }
                    Text {
                        width: parent.width
                        text: "/data/Music"
                        color: root.muted
                        elide: Text.ElideMiddle
                        font.family: "JetBrains Mono"
                        font.pixelSize: 10
                    }
                }
            }
        }
    }

    Item {
        id: content
        anchors.left: sidebar.right
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: playerBar.top
        clip: true

        Column {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.leftMargin: 48
            anchors.topMargin: 48
            spacing: 8

            Text {
                text: qsTr("专辑")
                color: root.fog
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 62
                font.weight: Font.Black
                font.letterSpacing: -2
            }
            Rectangle {
                width: 52
                height: 3
                radius: 2
                color: root.amber
            }
        }

        Item {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.leftMargin: 48
            anchors.rightMargin: 48
            anchors.topMargin: 150
            anchors.bottomMargin: 36

            Column {
                width: Math.min(360, parent.width * 0.44)
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                spacing: 14

                Text {
                    width: parent.width
                    text: controller.status
                    color: root.fog
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: 28
                    font.weight: Font.DemiBold
                }
                Text {
                    width: parent.width
                    text: controller.trackCount > 0
                          ? qsTr("曲库索引已保存，搜索和专辑视图共用这份数据。")
                          : qsTr("扫描完成后，这里会按专辑整理本地音乐。")
                    color: root.muted
                    wrapMode: Text.WordWrap
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: 14
                    lineHeight: 1.55
                }
                Rectangle {
                    width: parent.width
                    height: 1
                    color: Qt.rgba(root.fog.r, root.fog.g, root.fog.b, 0.1)
                }
                Text {
                    text: qsTr("音乐目录  /data/Music")
                    color: root.teal
                    font.family: "JetBrains Mono"
                    font.pixelSize: 11
                }

                Button {
                    id: scanButton

                    text: controller.scanning ? qsTr("扫描中") : qsTr("重新扫描")
                    enabled: !controller.scanning
                    focusPolicy: Qt.StrongFocus
                    implicitWidth: 104
                    implicitHeight: 38
                    topPadding: 0
                    bottomPadding: 0
                    leftPadding: 16
                    rightPadding: 16
                    onClicked: controller.scanLibrary()

                    contentItem: Text {
                        text: scanButton.text
                        color: root.darkMode ? "#12181b" : "#ffffff"
                        font.family: "Noto Sans CJK SC"
                        font.pixelSize: 12
                        font.weight: Font.DemiBold
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        radius: 19
                        color: scanButton.enabled
                               ? scanButton.hovered ? Qt.lighter(root.amber, 1.08) : root.amber
                               : root.muted
                        border.width: scanButton.activeFocus ? 2 : 0
                        border.color: root.fog

                        Behavior on color {
                            ColorAnimation { duration: 160; easing.type: Easing.OutCubic }
                        }
                    }
                }
            }

            VinylMark {
                width: Math.min(parent.height * 0.8, parent.width * 0.46)
                height: width
                anchors.right: parent.right
                anchors.rightMargin: -width * 0.15
                anchors.verticalCenter: parent.verticalCenter
                discColor: root.darkMode ? "#11181c" : "#d9dfe1"
                grooveColor: root.darkMode ? "#536168" : "#7e8b91"
                labelColor: root.rust
                labelTextColor: root.darkMode ? "#f4e9dc" : "#fff8f1"
            }
        }
    }

    PlayerBar {
        id: playerBar
        height: 92
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        surfaceColor: Qt.rgba(root.graphite.r, root.graphite.g, root.graphite.b, 0.96)
        foregroundColor: root.fog
        mutedColor: root.muted
        accentColor: root.amber
    }
}
