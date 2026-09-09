import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "ui" as UI

Item {
    id: stage
    width: 820
    height: 560
    QtObject {
        id: updates
        property string status: "available"
        property string message: "发现新版本 v0.4.0"
        property string currentVersion: "0.3.3"
        property string latestVersion: "0.4.0"
        property string notes: ""
        property string publishedAt: "2026-09-09T10:00:00Z"
        property string lastChecked: "2026-09-09T10:00:00Z"
        property string skippedVersion: ""
        property string actionError: ""
        property string installHint: "下载后先退出留声，沿用原安装方式更新程序。"
        property bool busy: false
        property bool ignored: false
        property bool notifyAvailable: true
        property var assets: []
        property int recommendedIndex: 0
        property int downloaded: -1
        property int skipped: 0
        property int later: 0
        property int checks: 0
        property int openedPage: 0
        signal changed
        function check(manual) {
            checks++;
            busy = true;
            status = "checking";
            changed();
        }
        function openAsset(index) {
            downloaded = index;
            return true;
        }
        function openReleasePage() {
            openedPage++;
            return true;
        }
        function skipVersion() {
            skipped++;
            ignored = true;
            notifyAvailable = false;
            changed();
            return true;
        }
        function remindLater() {
            later++;
            notifyAvailable = false;
            changed();
        }
    }
    Component {
        id: dialogComponent
        UI.UpdateDialog {
            updater: updates
            x: 110
            y: 20
        }
    }
    Component {
        id: noticeComponent
        UI.UpdateNotice {
            updater: updates
            width: 560
            height: 52
            property int details: 0
            onDetailsRequested: details++
        }
    }
    TestCase {
        name: "SoftwareUpdates"
        when: windowShown
        function init() {
            UI.Theme.previewAppearance = "light";
            UI.Theme.reducedMotion = true;
            updates.status = "available";
            updates.busy = false;
            updates.ignored = false;
            updates.message = "发现新版本 v0.4.0";
            updates.notes = "<img src='https://invalid.test/tracker.png'>\n# 更新内容\n" + "这是很长的中文 Japanese English 混合发布说明。\n".repeat(150);
            updates.assets = [
                {
                    label: "DEB · Linux x86_64",
                    name: "liusheng_0.4.0_amd64.deb",
                    size: 12345678
                },
                {
                    label: "AppImage · Linux x86_64",
                    name: "liusheng-0.4.0-linux-x86_64.AppImage",
                    size: 60000000
                }
            ];
            updates.recommendedIndex = 1;
            updates.downloaded = -1;
            updates.skipped = 0;
            updates.later = 0;
            updates.checks = 0;
            updates.openedPage = 0;
            updates.actionError = "";
            updates.notifyAvailable = true;
        }
        function openDialog() {
            const dialog = createTemporaryObject(dialogComponent, stage);
            dialog.open();
            tryCompare(dialog, "opened", true);
            return dialog;
        }
        function test_notes_are_plain_text_and_compact_dialog_is_bounded() {
            const dialog = openDialog();
            compare(dialog.popupType, Popup.Item);
            const text = findChild(dialog, "updateReleaseNotes");
            verify(text);
            compare(text.textFormat, Text.PlainText);
            verify(text.text.indexOf("<img") >= 0);
            verify(dialog.width <= stage.width - 40);
            verify(dialog.height <= stage.height - 40);
            verify(dialog.contentItem.height > 100);
            const download = findChild(dialog, "updateDownloadButton");
            verify(download.visible && download.enabled);
            dialog.close();
        }
        function test_package_selection_and_download_require_click() {
            const dialog = openDialog();
            compare(updates.downloaded, -1);
            const choices = findChild(dialog, "updatePackageChoice");
            compare(choices.currentIndex, 1);
            choices.currentIndex = 0;
            mouseClick(findChild(dialog, "updateDownloadButton"));
            compare(updates.downloaded, 0);
            verify(dialog.opened);
            dialog.close();
        }
        function test_skip_and_later_have_separate_actions() {
            let dialog = openDialog();
            mouseClick(findChild(dialog, "skipUpdateVersion"));
            compare(updates.skipped, 1);
            compare(updates.later, 0);
            tryCompare(dialog, "opened", false);
            updates.ignored = false;
            dialog = openDialog();
            keyClick(Qt.Key_Escape);
            tryCompare(dialog, "opened", false);
            compare(updates.later, 1);
        }
        function test_ignored_version_can_still_download_manually() {
            updates.ignored = true;
            const dialog = openDialog();
            verify(!findChild(dialog, "skipUpdateVersion").visible);
            mouseClick(findChild(dialog, "updateDownloadButton"));
            compare(updates.downloaded, 1);
            dialog.close();
        }
        function test_missing_assets_offer_release_page() {
            updates.assets = [];
            updates.recommendedIndex = -1;
            const dialog = openDialog();
            verify(!findChild(dialog, "updateDownloadButton").visible);
            mouseClick(findChild(dialog, "updateReleasePage"));
            compare(updates.openedPage, 1);
            dialog.close();
        }
        function test_error_retry_and_busy_states() {
            updates.status = "error";
            updates.message = "连接超时，请重试。";
            const dialog = openDialog();
            verify(!findChild(dialog, "updateDownloadButton").visible);
            const retry = findChild(dialog, "updateCheckAgain");
            mouseClick(retry);
            compare(updates.checks, 1);
            verify(!retry.enabled);
            mouseClick(retry);
            compare(updates.checks, 1);
            dialog.close();
        }
        function test_nonmodal_notice_never_grabs_focus() {
            const field = Qt.createQmlObject('import QtQuick.Controls.Basic; TextField { x: 30; y: 100; width: 200; height: 40 }', stage);
            field.forceActiveFocus();
            const notice = createTemporaryObject(noticeComponent, stage);
            verify(field.activeFocus);
            mouseClick(findChild(notice, "updateNoticeDetails"));
            compare(notice.details, 1);
            mouseClick(findChild(notice, "updateNoticeLater"));
            compare(updates.later, 1);
            field.destroy();
        }
    }
}
