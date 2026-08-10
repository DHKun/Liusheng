pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: control

    property bool exclusive: false
    property bool busy: false
    property string statusText
    property string errorText
    property color surfaceColor: "#151d22"
    property color foregroundColor: "#e8edf0"
    property color mutedColor: "#829198"
    property color accentColor: "#d9a15f"
    property color errorColor: "#b85f4a"

    signal modeRequested(bool exclusive)

    implicitHeight: errorText.length > 0 ? 128 : 106
    radius: 14
    color: Qt.rgba(surfaceColor.r, surfaceColor.g, surfaceColor.b, 0.68)
    border.width: 1
    border.color: Qt.rgba(foregroundColor.r,
                          foregroundColor.g,
                          foregroundColor.b,
                          0.08)

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 7

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Text {
                text: qsTr("音频输出")
                color: control.foregroundColor
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 12
                font.weight: Font.DemiBold
            }

            Item { Layout.fillWidth: true }

            Text {
                text: control.busy ? qsTr("切换中") : control.statusText
                color: control.busy ? control.accentColor : control.mutedColor
                elide: Text.ElideRight
                font.family: "Noto Sans CJK SC"
                font.pixelSize: 10
                Layout.maximumWidth: 92
            }
        }

        Rectangle {
            id: modePicker

            Layout.fillWidth: true
            Layout.preferredHeight: 34
            radius: 10
            color: Qt.rgba(control.foregroundColor.r,
                           control.foregroundColor.g,
                           control.foregroundColor.b,
                           0.05)

            Rectangle {
                id: selection

                x: control.exclusive ? 2 + width : 2
                y: 2
                width: (modePicker.width - 4) / 2
                height: modePicker.height - 4
                radius: 8
                color: Qt.rgba(control.accentColor.r,
                               control.accentColor.g,
                               control.accentColor.b,
                               0.18)
                border.width: 1
                border.color: Qt.rgba(control.accentColor.r,
                                      control.accentColor.g,
                                      control.accentColor.b,
                                      0.38)

                Behavior on x {
                    NumberAnimation { duration: 180; easing.type: Easing.OutCubic }
                }
            }

            Row {
                anchors.fill: parent

                Repeater {
                    model: [qsTr("共享"), qsTr("独占")]

                    Button {
                        id: modeButton

                        required property int index
                        required property string modelData

                        width: modePicker.width / 2
                        height: modePicker.height
                        text: modelData
                        enabled: !control.busy
                        focusPolicy: Qt.StrongFocus
                        Accessible.name: index === 0
                                         ? qsTr("使用共享音频输出")
                                         : qsTr("使用独占音频输出")
                        onClicked: {
                            const requestedExclusive = index === 1
                            if (requestedExclusive !== control.exclusive)
                                control.modeRequested(requestedExclusive)
                        }

                        contentItem: Text {
                            text: modeButton.text
                            color: modeButton.index === (control.exclusive ? 1 : 0)
                                   ? control.accentColor
                                   : control.mutedColor
                            font.family: "Noto Sans CJK SC"
                            font.pixelSize: 11
                            font.weight: modeButton.index === (control.exclusive ? 1 : 0)
                                         ? Font.DemiBold
                                         : Font.Medium
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }

                        background: Rectangle {
                            color: "transparent"
                            radius: 8
                            border.width: modeButton.activeFocus ? 1 : 0
                            border.color: control.foregroundColor
                        }
                    }
                }
            }
        }

        Text {
            id: outputError

            visible: control.errorText.length > 0
            Layout.fillWidth: true
            text: control.errorText
            color: control.errorColor
            elide: Text.ElideRight
            font.family: "Noto Sans CJK SC"
            font.pixelSize: 10

            HoverHandler { id: errorHover }

            ToolTip.visible: errorHover.hovered
            ToolTip.text: control.errorText
            ToolTip.delay: 500
        }
    }

    Rectangle {
        visible: control.busy
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 2
        radius: 1
        color: control.accentColor
        opacity: 0.78

        SequentialAnimation on opacity {
            running: control.busy
            loops: Animation.Infinite
            NumberAnimation { to: 0.24; duration: 520; easing.type: Easing.InOutSine }
            NumberAnimation { to: 0.78; duration: 520; easing.type: Easing.InOutSine }
        }
    }
}
