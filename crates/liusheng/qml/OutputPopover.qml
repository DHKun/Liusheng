pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

QuietPopover {
    id: popup
    required property var controller
    property bool detailsVisible: false
    signal settingsRequested
    width: 340
    padding: 22
    implicitHeight: Math.min(column.implicitHeight + 44, parent ? parent.height - 128 : 480)
    focus: true
    background: QuietSurface {}
    onOpened: controller.refreshHardwareVolume()
    contentItem: ScrollView {
        id: outputScroll
        contentWidth: availableWidth
        clip: true
        ColumnLayout {
            id: column
            width: outputScroll.availableWidth
            spacing: 16
            RowLayout {
                Layout.fillWidth: true
                Text {
                    text: qsTr("声音输出")
                    color: Theme.text
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }
                QuietButton {
                    glyph: "settings"
                    compact: true
                    hint: qsTr("音频设置")
                    onClicked: {
                        popup.close();
                        popup.settingsRequested();
                    }
                }
            }
            Text {
                text: popup.controller.outputStatus
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            RowLayout {
                visible: Qt.platform.os === "linux"
                spacing: 4
                Layout.fillWidth: true
                QuietButton {
                    text: qsTr("共享")
                    selected: !popup.controller.exclusiveOutput
                    Layout.fillWidth: true
                    enabled: !popup.controller.outputSwitching
                    onClicked: popup.controller.requestExclusiveOutput(false)
                }
                QuietButton {
                    text: qsTr("独占")
                    selected: popup.controller.exclusiveOutput
                    Layout.fillWidth: true
                    enabled: !popup.controller.outputSwitching
                    onClicked: popup.controller.requestExclusiveOutput(true)
                }
            }
            Text {
                visible: popup.controller.outputError.length > 0
                text: popup.controller.outputError
                color: Theme.danger
                font.pixelSize: Theme.captionSize
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            Rectangle {
                height: 1
                color: Theme.line
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.fillWidth: true
                Text {
                    text: qsTr("硬件音量")
                    color: Theme.text
                    font.pixelSize: Theme.captionSize
                    Layout.fillWidth: true
                }
                Text {
                    text: popup.controller.hardwareVolumeAvailable ? popup.controller.hardwareVolumePercent + "%" : qsTr("设备按键控制")
                    color: Theme.secondary
                    font.pixelSize: Theme.noteSize
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                QuietButton {
                    glyph: popup.controller.hardwareMuted ? "mute" : "volume"
                    hint: qsTr("硬件静音")
                    compact: true
                    enabled: popup.controller.hardwareMuteAvailable
                    onClicked: popup.controller.toggleHardwareMute()
                }
                QuietSlider {
                    Layout.fillWidth: true
                    from: 0
                    to: 100
                    stepSize: 1
                    value: popup.controller.hardwareVolumePercent
                    enabled: popup.controller.hardwareVolumeAvailable
                    Accessible.name: qsTr("硬件音量")
                    onMoved: popup.controller.requestHardwareVolume(Math.round(value))
                }
            }
            QuietButton {
                text: popup.detailsVisible ? qsTr("收起音频详情") : qsTr("查看音频详情")
                compact: true
                glyph: popup.detailsVisible ? "up" : "down"
                onClicked: popup.detailsVisible = !popup.detailsVisible
            }
            Text {
                visible: popup.detailsVisible
                text: popup.controller.audioDetails || qsTr("播放后显示来源格式与输出协商结果。")
                color: Theme.secondary
                font.pixelSize: Theme.captionSize
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
        }
    }

    Timer {
        interval: popup.controller.hardwareVolumeAvailable ? 1000 : 5000
        repeat: true
        running: popup.opened
        onTriggered: popup.controller.refreshHardwareVolume()
    }
}
