pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: control

    property bool available: false
    property bool muted: false
    property bool canMute: false
    property int percent: 100
    property string errorText
    property color foregroundColor: "#e8edf0"
    property color mutedColor: "#829198"
    property color accentColor: "#d9a15f"

    signal volumeRequested(int percent)
    signal muteRequested
    signal refreshRequested

    implicitWidth: 202
    implicitHeight: 52

    RowLayout {
        anchors.fill: parent
        spacing: 10

        Button {
            id: muteButton

            Layout.preferredWidth: 50
            Layout.preferredHeight: 32
            text: control.muted ? qsTr("恢复") : qsTr("静音")
            enabled: control.available && control.canMute
            focusPolicy: Qt.StrongFocus
            Accessible.name: control.muted ? qsTr("恢复硬件声音") : qsTr("硬件静音")
            onClicked: control.muteRequested()

            contentItem: Text {
                text: muteButton.text
                color: muteButton.enabled
                       ? control.muted ? control.accentColor : control.foregroundColor
                       : control.mutedColor
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 10
                font.weight: Font.Medium
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }

            background: Rectangle {
                radius: 16
                color: control.muted
                       ? Qt.rgba(control.accentColor.r,
                                 control.accentColor.g,
                                 control.accentColor.b,
                                 0.14)
                       : "transparent"
                border.width: muteButton.activeFocus ? 2 : 1
                border.color: muteButton.activeFocus
                              ? control.accentColor
                              : Qt.rgba(control.foregroundColor.r,
                                        control.foregroundColor.g,
                                        control.foregroundColor.b,
                                        0.12)

                Behavior on color {
                    ColorAnimation { duration: 160; easing.type: Easing.OutCubic }
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Text {
                    text: qsTr("硬件音量")
                    color: control.available ? control.mutedColor : Qt.rgba(
                               control.mutedColor.r,
                               control.mutedColor.g,
                               control.mutedColor.b,
                               0.58)
                    font.family: "Noto Sans CJK SC"
                    font.pixelSize: 10
                }

                Item { Layout.fillWidth: true }

                Text {
                    text: control.muted
                          ? qsTr("静音")
                          : qsTr("%1%").arg(control.available ? control.percent : 100)
                    color: control.muted ? control.accentColor : control.mutedColor
                    font.family: "JetBrains Mono"
                    font.pixelSize: 10
                }
            }

            Slider {
                id: volumeSlider

                Layout.fillWidth: true
                Layout.preferredHeight: 18
                from: 0
                to: 100
                stepSize: 1
                enabled: control.available
                focusPolicy: Qt.StrongFocus
                hoverEnabled: true
                Accessible.name: qsTr("硬件音量")
                Accessible.description: control.errorText
                onMoved: control.volumeRequested(Math.round(value))

                Binding {
                    target: volumeSlider
                    property: "value"
                    value: control.available ? control.percent : 100
                    when: !volumeSlider.pressed
                    restoreMode: Binding.RestoreBindingOrValue
                }

                background: Rectangle {
                    x: volumeSlider.leftPadding
                    y: volumeSlider.topPadding
                       + volumeSlider.availableHeight / 2 - height / 2
                    width: volumeSlider.availableWidth
                    height: 3
                    radius: 2
                    color: Qt.rgba(control.foregroundColor.r,
                                   control.foregroundColor.g,
                                   control.foregroundColor.b,
                                   0.11)

                    Rectangle {
                        width: volumeSlider.visualPosition * parent.width
                        height: parent.height
                        radius: parent.radius
                        color: control.accentColor
                        opacity: control.available && !control.muted ? 1 : 0.42
                    }
                }

                handle: Rectangle {
                    x: volumeSlider.leftPadding
                       + volumeSlider.visualPosition
                         * (volumeSlider.availableWidth - width)
                    y: volumeSlider.topPadding
                       + volumeSlider.availableHeight / 2 - height / 2
                    width: volumeSlider.pressed || volumeSlider.hovered ? 11 : 8
                    height: width
                    radius: width / 2
                    color: control.accentColor
                    border.width: volumeSlider.activeFocus ? 2 : 0
                    border.color: control.foregroundColor
                    opacity: control.available ? 1 : 0.32

                    Behavior on width {
                        NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
                    }
                }
            }
        }
    }

    HoverHandler { id: unavailableHover }

    ToolTip.visible: !control.available
                     && control.errorText.length > 0
                     && unavailableHover.hovered
    ToolTip.text: control.errorText
    ToolTip.delay: 500

    Timer {
        interval: control.available ? 1000 : 5000
        repeat: true
        running: control.visible
        onTriggered: control.refreshRequested()
    }
}
