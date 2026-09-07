pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs
import io.github.dhkun.Liusheng 1.0

QuietDialog {
    id: dialog
    required property var controller
    property var draft: ({})
    property string initialTab: "library"
    property string tab: initialTab
    property alias folderView: folder
    property alias rootsField: roots
    property alias appearanceControl: appearance
    property alias densityControl: density
    property string localError: ""
    readonly property bool validDraft: acceptEnabled
    title: qsTr("设置")
    width: Math.min(620, parent ? parent.width - 40 : 620)
    height: Math.min(620, parent ? parent.height - 40 : 620)
    acceptEnabled: roots.text.trim().length > 0 && validPaths(roots.text) && validPaths(excludes.text) && device.text.trim().length > 0 && mixer.text.trim().length > 0 && element.text.trim().length > 0
    onOpened: {
        tab = initialTab;
        localError = "";
        controller.refreshDevices();
        try {
            draft = JSON.parse(controller.settingsJson);
        } catch (error) {
            draft = ({});
        }
        roots.text = (draft.music_roots || []).join("\n");
        excludes.text = (draft.excluded_directories || []).join("\n");
        device.text = draft.exclusive_device || "hw:Hybrid,0";
        mixer.text = draft.mixer_device || "hw:Hybrid";
        element.text = draft.mixer_element || "PCM";
        tray.checked = draft.close_to_tray !== false;
        restore.checked = draft.restore_session !== false;
        exclusive.checked = draft.prefer_exclusive === true;
        appearance.currentIndex = Math.max(0, ["system", "light", "dark"].indexOf(draft.appearance || "system"));
        motion.checked = draft.reduced_motion === true;
        density.currentIndex = draft.compact_grid === false ? 1 : 0;
    }
    function lines(text) {
        return text.split(/\r?\n/).map(s => s.trim()).filter(s => s.length > 0);
    }
    function validPaths(text) {
        return lines(text).every(s => s.charAt(0) === "/");
    }
    onAccepted: {
        const settings = Object.assign({}, draft);
        settings.version = 1;
        settings.music_roots = lines(roots.text);
        settings.excluded_directories = lines(excludes.text);
        settings.exclusive_device = device.text.trim();
        settings.mixer_device = mixer.text.trim();
        settings.mixer_element = element.text.trim();
        settings.close_to_tray = tray.checked;
        settings.restore_session = restore.checked;
        settings.prefer_exclusive = exclusive.checked;
        settings.appearance = ["system", "light", "dark"][appearance.currentIndex];
        settings.reduced_motion = motion.checked;
        settings.compact_grid = density.currentIndex === 0;
        controller.applySettings(JSON.stringify(settings));
    }
    FolderDialog {
        id: folder
        parentWindow: dialog.parent ? dialog.parent.Window.window : null
        title: qsTr("添加音乐目录")
        onAccepted: {
            const path = DesktopBridge.localPath(selectedFolder);
            const previous = dialog.lines(roots.text);
            if (previous.indexOf(path) < 0)
                previous.push(path);
            roots.text = previous.join("\n");
        }
    }
    contentItem: ColumnLayout {
        spacing: 20
        RowLayout {
            Layout.fillWidth: true
            spacing: 6
            QuietButton {
                text: qsTr("曲库")
                selected: dialog.tab === "library"
                onClicked: dialog.tab = "library"
                Layout.fillWidth: true
            }
            QuietButton {
                text: qsTr("播放")
                selected: dialog.tab === "playback"
                onClicked: dialog.tab = "playback"
                Layout.fillWidth: true
            }
            QuietButton {
                text: qsTr("外观与行为")
                selected: dialog.tab === "appearance"
                onClicked: dialog.tab = "appearance"
                Layout.fillWidth: true
            }
        }
        Rectangle {
            height: 1
            color: Theme.line
            Layout.fillWidth: true
        }
        ScrollView {
            id: scroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: availableWidth
            ColumnLayout {
                width: scroll.availableWidth
                spacing: 14
                ColumnLayout {
                    visible: dialog.tab === "library"
                    Layout.fillWidth: true
                    spacing: 12
                    RowLayout {
                        Layout.fillWidth: true
                        Text {
                            text: qsTr("音乐目录")
                            color: Theme.text
                            font.pixelSize: Theme.bodySize
                            font.weight: Font.Medium
                            Layout.fillWidth: true
                        }
                        QuietButton {
                            text: qsTr("添加目录")
                            glyph: "plus"
                            compact: true
                            onClicked: folder.open()
                        }
                    }
                    Text {
                        text: qsTr("每行一个完整路径。离线目录会保留曲库缓存。")
                        color: Theme.secondary
                        font.pixelSize: Theme.captionSize
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                    QuietTextArea {
                        id: roots
                        objectName: "musicDirectories"
                        Layout.fillWidth: true
                        Layout.minimumHeight: 90
                        placeholderText: "/data/Music"
                        Accessible.name: qsTr("音乐目录")
                    }
                    Text {
                        text: qsTr("排除目录")
                        color: Theme.text
                        font.pixelSize: Theme.bodySize
                        Layout.topMargin: 10
                    }
                    QuietTextArea {
                        id: excludes
                        Layout.fillWidth: true
                        Layout.minimumHeight: 65
                        placeholderText: qsTr("可选，每行一个完整路径")
                        Accessible.name: qsTr("排除目录")
                    }
                    Text {
                        text: dialog.controller.scanErrors
                        visible: text.length > 0
                        color: Theme.danger
                        font.pixelSize: Theme.captionSize
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                }
                ColumnLayout {
                    visible: dialog.tab === "playback"
                    Layout.fillWidth: true
                    spacing: 12
                    Text {
                        text: qsTr("输出设备")
                        color: Theme.text
                        font.pixelSize: Theme.bodySize
                        font.weight: Font.Medium
                    }
                    Text {
                        text: Qt.platform.os === "linux" ? qsTr("共享模式使用系统输出，独占模式直接连接所选设备。") : qsTr("使用系统默认的 CoreAudio 输出设备。")
                        color: Theme.secondary
                        font.pixelSize: Theme.captionSize
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                    QuietComboBox {
                        visible: Qt.platform.os === "linux"
                        Layout.fillWidth: true
                        property var devices: {
                            try {
                                return JSON.parse(dialog.controller.devicesJson);
                            } catch (e) {
                                return [];
                            }
                        }
                        model: devices
                        textRole: "name"
                        displayText: currentIndex < 0 ? qsTr("选择检测到的设备") : currentText
                        onActivated: {
                            device.text = devices[currentIndex].device;
                            mixer.text = devices[currentIndex].mixer;
                        }
                    }
                    GridLayout {
                        visible: Qt.platform.os === "linux"
                        columns: 2
                        columnSpacing: 20
                        rowSpacing: 12
                        Layout.fillWidth: true
                        Text {
                            text: qsTr("ALSA 设备")
                            color: Theme.secondary
                            font.pixelSize: Theme.captionSize
                        }
                        QuietField {
                            id: device
                            Layout.fillWidth: true
                            Accessible.name: qsTr("ALSA 设备")
                        }
                        Text {
                            text: qsTr("混音器设备")
                            color: Theme.secondary
                            font.pixelSize: Theme.captionSize
                        }
                        QuietField {
                            id: mixer
                            Layout.fillWidth: true
                            Accessible.name: qsTr("混音器设备")
                        }
                        Text {
                            text: qsTr("音量控件")
                            color: Theme.secondary
                            font.pixelSize: Theme.captionSize
                        }
                        QuietField {
                            id: element
                            Layout.fillWidth: true
                            Accessible.name: qsTr("硬件音量控件")
                        }
                    }
                    QuietCheckBox {
                        id: exclusive
                        text: qsTr("下次播放优先使用独占模式")
                        visible: Qt.platform.os === "linux"
                        Layout.fillWidth: true
                    }
                    Text {
                        text: qsTr("音量由硬件控制。设备未提供混音器时，使用耳机或音箱按键调节。")
                        color: Theme.secondary
                        font.pixelSize: Theme.captionSize
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                }
                ColumnLayout {
                    visible: dialog.tab === "appearance"
                    Layout.fillWidth: true
                    spacing: 14
                    Text {
                        text: qsTr("外观")
                        color: Theme.text
                        font.pixelSize: Theme.bodySize
                        font.weight: Font.Medium
                    }
                    QuietComboBox {
                        id: appearance
                        objectName: "appearanceChoice"
                        Layout.fillWidth: true
                        model: [qsTr("跟随系统"), qsTr("浅色"), qsTr("深色")]
                        Accessible.name: qsTr("界面主题")
                    }
                    Text {
                        text: qsTr("封面布局")
                        color: Theme.text
                        font.pixelSize: Theme.labelSize
                    }
                    QuietComboBox {
                        id: density
                        objectName: "gridDensity"
                        Layout.fillWidth: true
                        model: [qsTr("紧凑 · 显示更多专辑"), qsTr("舒适 · 更大封面")]
                        Accessible.name: qsTr("封面布局")
                    }
                    QuietCheckBox {
                        id: motion
                        text: qsTr("减少动画")
                        Layout.fillWidth: true
                    }
                    Rectangle {
                        height: 1
                        color: Theme.line
                        Layout.fillWidth: true
                        Layout.topMargin: 6
                        Layout.bottomMargin: 6
                    }
                    Text {
                        text: qsTr("窗口与播放")
                        color: Theme.text
                        font.pixelSize: Theme.bodySize
                        font.weight: Font.Medium
                    }
                    QuietCheckBox {
                        id: tray
                        objectName: "closeToTrayChoice"
                        text: qsTr("关闭窗口后留在托盘")
                        Layout.fillWidth: true
                    }
                    Text {
                        text: qsTr("Wayland 初次使用默认关闭即退出；勾选后会在托盘可用时隐藏窗口。托盘右键操作会在主窗口中显示。")
                        visible: DesktopBridge.wayland
                        color: Theme.secondary
                        font.pixelSize: Theme.captionSize
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                    QuietCheckBox {
                        id: restore
                        text: qsTr("启动时恢复队列与浏览位置")
                        Layout.fillWidth: true
                    }
                    Text {
                        text: qsTr("恢复后的播放保持暂停，准备好后再继续。")
                        color: Theme.secondary
                        font.pixelSize: Theme.captionSize
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                    }
                }
                Text {
                    text: qsTr("请填写以 / 开头的完整目录路径。")
                    visible: !dialog.validPaths(roots.text) || !dialog.validPaths(excludes.text)
                    color: Theme.danger
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                }
            }
        }
    }
}
