pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import io.github.dhkun.Liusheng 1.0

Dialog {
    id: dialog
    required property var controller
    property var draft: ({})
    title: qsTr("设置")
    modal: true
    width: Math.min(700, parent ? parent.width - 48 : 700)
    height: Math.min(680, parent ? parent.height - 48 : 680)
    standardButtons: Dialog.Save | Dialog.Cancel
    onOpened: {
        controller.refreshDevices()
        try { draft = JSON.parse(controller.settingsJson) } catch (error) { draft = ({}) }
        roots.text = (draft.music_roots || []).join("\n")
        excludes.text = (draft.excluded_directories || []).join("\n")
        device.text = draft.exclusive_device || "hw:Hybrid,0"
        mixer.text = draft.mixer_device || "hw:Hybrid"
        element.text = draft.mixer_element || "PCM"
        tray.checked = draft.close_to_tray !== false
        restore.checked = draft.restore_session !== false
        exclusive.checked = draft.prefer_exclusive === true
    }
    function lines(text) { return text.split(/\r?\n/).map(s => s.trim()).filter(s => s.length > 0) }
    onAccepted: {
        const settings = Object.assign({}, draft)
        settings.version = 1
        settings.music_roots = lines(roots.text)
        settings.excluded_directories = lines(excludes.text)
        settings.exclusive_device = device.text.trim()
        settings.mixer_device = mixer.text.trim()
        settings.mixer_element = element.text.trim()
        settings.close_to_tray = tray.checked
        settings.restore_session = restore.checked
        settings.prefer_exclusive = exclusive.checked
        controller.applySettings(JSON.stringify(settings))
    }
    FolderDialog {
        id: folder
        title: qsTr("添加音乐目录")
        onAccepted: {
            const path = DesktopBridge.localPath(selectedFolder)
            const previous = dialog.lines(roots.text)
            if (previous.indexOf(path) < 0) previous.push(path)
            roots.text = previous.join("\n")
        }
    }
    ScrollView {
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth
        ColumnLayout {
            width: parent.width
            spacing: 12
            Label { text: qsTr("音乐目录"); font.bold: true }
            Label { text: qsTr("每行一个绝对路径。离线目录保留曲库缓存。"); wrapMode: Text.Wrap; Layout.fillWidth: true }
            TextArea { id: roots; Layout.fillWidth: true; Layout.preferredHeight: 85; placeholderText: "/data/Music\n/home/user/Music"; selectByMouse: true }
            Button { text: qsTr("选择目录…"); onClicked: folder.open() }
            Label { text: qsTr("排除目录"); font.bold: true }
            TextArea { id: excludes; Layout.fillWidth: true; Layout.preferredHeight: 65; placeholderText: qsTr("每行一个绝对路径"); selectByMouse: true }
            Label { text: qsTr("独占输出与硬件音量"); font.bold: true; visible: Qt.platform.os === "linux" }
            ComboBox {
                visible: Qt.platform.os === "linux"
                Layout.fillWidth: true
                property var devices: { try { return JSON.parse(dialog.controller.devicesJson) } catch (e) { return [] } }
                model: devices
                textRole: "name"
                displayText: currentIndex < 0 ? qsTr("检测到的输出设备") : currentText
                onActivated: { device.text = devices[currentIndex].device; mixer.text = devices[currentIndex].mixer }
            }
            GridLayout {
                visible: Qt.platform.os === "linux"
                columns: 2; Layout.fillWidth: true
                Label { text: qsTr("ALSA 设备") }
                TextField { id: device; Layout.fillWidth: true; selectByMouse: true }
                Label { text: qsTr("Mixer 设备") }
                TextField { id: mixer; Layout.fillWidth: true; selectByMouse: true }
                Label { text: qsTr("音量控件") }
                TextField { id: element; Layout.fillWidth: true; selectByMouse: true }
            }
            CheckBox { id: exclusive; text: qsTr("下次播放优先使用独占输出"); visible: Qt.platform.os === "linux" }
            CheckBox { id: tray; text: qsTr("关闭窗口时留在托盘") }
            CheckBox { id: restore; text: qsTr("启动恢复队列、位置与界面（暂停状态）") }
            Label { text: qsTr("曲库扫描问题"); font.bold: true; visible: dialog.controller.scanErrors.length > 0 }
            TextArea { text: dialog.controller.scanErrors; readOnly: true; selectByMouse: true; wrapMode: Text.Wrap; Layout.fillWidth: true; visible: text.length > 0 }
        }
    }
}
