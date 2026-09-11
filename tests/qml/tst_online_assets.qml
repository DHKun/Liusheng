import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "ui" as UI

Item {
    id: stage
    width: 820
    height: 560
    QtObject {
        id: testService
        property var state: ({})
        property int searches: 0
        property string searchedSource: ""
        property int choices: 0
        property int previews: 0
        property int closed: 0
        property int batchCommands: 0
        signal changed
        signal applied(string identity, string key, string kind, string message)
        function update(values) {
            state = Object.assign({}, state, values);
            changed();
        }
        function search(title, artist, album) {
            searches++;
            update({
                busy: true,
                message: "搜索中"
            });
        }
        function searchWithSource(title, artist, album, source) {
            searchedSource = source;
            search(title, artist, album);
        }
        function preview(index) {
            previews++;
            update({
                selected: index,
                previewText: "[00:01.00]测试歌词"
            });
        }
        function choose(album) {
            choices++;
            update({
                busy: true,
                status: "saving"
            });
        }
        function close() {
            closed++;
        }
        function cancel() {
            update({
                busy: false
            });
        }
        function restoreDefault(album) {
        }
        function importLocal(file, album) {
        }
        function openSource() {
        }
        function inspectBatch() {
        }
        function pauseBatch() {
            batchCommands++;
            update({
                batchPaused: true
            });
        }
        function resumeBatch() {
            batchCommands++;
            update({
                batchPaused: false
            });
        }
        function cancelBatch() {
            batchCommands++;
            update({
                batchCancelled: state.batchRemaining,
                batchRetryable: state.batchRemaining,
                batchRemaining: 0,
                batchPaused: true,
                batchPhase: "cancelled",
                batchRetryAt: 0
            });
        }
        function retryBatchFailures() {
            batchCommands++;
            update({
                batchRemaining: state.batchRetryable,
                batchRetryable: 0,
                batchCancelled: 0,
                batchPaused: false,
                batchPhase: "running"
            });
        }
        function clearSearchCache() {
        }
    }
    QtObject {
        id: testController
        property bool onlinePreparing: false
        property int selectedTrackCount: 0
        property int started: 0
        function prepareOnlineBatch(kind, selected) {
            started++;
        }
        function cancelOnlinePreparation() {
            onlinePreparing = false;
        }
    }
    Component {
        id: searchDialog
        UI.OnlineAssetsDialog {
            service: testService
            x: 16
            y: 16
        }
    }
    Component {
        id: batchDialog
        UI.OnlineBatchDialog {
            service: testService
            controller: testController
            x: 16
            y: 16
        }
    }
    TestCase {
        name: "OnlineMusicResources"
        when: windowShown
        function init() {
            UI.Theme.reducedMotion = true;
            UI.Theme.previewAppearance = "light";
            testService.state = {
                busy: false,
                kind: "lyrics",
                status: "idle",
                context: {
                    trackKey: "abc",
                    identity: "song",
                    albumKey: "",
                    title: "Title",
                    artist: "Artist",
                    album: "",
                    scope: "track"
                },
                candidates: [],
                selected: -1,
                previewText: "",
                previewUrl: "",
                source: "",
                message: "点击查找",
                batchTotal: 0,
                batchDone: 0,
                batchRemaining: 0,
                batchPaused: true,
                batchResults: []
            };
            testService.searches = 0;
            testService.searchedSource = "";
            testService.choices = 0;
            testService.previews = 0;
            testService.closed = 0;
            testService.batchCommands = 0;
            testController.onlinePreparing = false;
            testController.started = 0;
        }
        function open(component) {
            const dialog = createTemporaryObject(component, stage);
            verify(dialog);
            dialog.open();
            tryCompare(dialog, "opened", true);
            return dialog;
        }
        function test_source_picker_routes_explicit_search_and_resets_by_kind() {
            const dialog = open(searchDialog);
            const picker = findChild(dialog, "onlineSourcePicker");
            compare(picker.sourceId, "all");
            compare(testService.searches, 0);
            picker.currentIndex = 1;
            compare(picker.sourceId, "qqmusic");
            mouseClick(findChild(dialog, "onlineSearch"));
            compare(testService.searchedSource, "qqmusic");
            verify(!picker.enabled);
            mouseClick(findChild(dialog, "onlineCancelRequest"));
            testService.update({
                kind: "cover"
            });
            compare(picker.sourceId, "all");
            picker.currentIndex = 3;
            compare(picker.sourceId, "deezer");
            dialog.close();
        }
        function test_provider_lyric_preview_is_required_before_apply() {
            testService.update({
                candidates: [
                    {
                        id: "qq-mid",
                        title: "Title",
                        artist: "Artist",
                        album: "",
                        note: "",
                        lyricsPending: true,
                        delta: 0,
                        provider: "QQ 音乐"
                    }
                ],
                selected: 0
            });
            const dialog = open(searchDialog);
            verify(!findChild(dialog, "onlineApply").enabled);
            testService.update({
                candidates: [Object.assign({}, testService.state.candidates[0], {
                        lyricsPending: false,
                        synced: true
                    })],
                status: "preview"
            });
            verify(findChild(dialog, "onlineApply").enabled);
            verify(!findChild(dialog, "onlineMessage").visible);
            dialog.close();
        }
        function test_source_details_are_collapsed_and_escape_preserves_search() {
            testService.update({
                sourceReports: [
                    {
                        provider: "MusicBrainz",
                        status: "error",
                        count: 0,
                        message: "HTTP 503"
                    }
                ],
                queryHint: "Title Artist"
            });
            const dialog = open(searchDialog);
            const info = findChild(dialog, "onlineSourceInfo");
            verify(info && !info.opened);
            verify(!findChild(dialog, "onlineMessage").visible);
            mouseClick(findChild(dialog, "onlineSourceInfoButton"));
            tryCompare(info, "opened", true);
            compare(info.popupType, Popup.Item);
            keyClick(Qt.Key_Escape);
            tryCompare(info, "opened", false);
            verify(dialog.opened);
            compare(findChild(dialog, "onlineTitle").text, "Title");
            dialog.close();
        }
        function test_batch_captures_source_before_async_preparation() {
            const dialog = open(batchDialog);
            const picker = findChild(dialog, "batchSourcePicker");
            picker.currentIndex = 2;
            compare(picker.sourceId, "netease");
            mouseClick(findChild(dialog, "batchStart"));
            compare(testController.started, 1);
            compare(dialog.requestedSource, "netease");
            picker.currentIndex = 0;
            compare(dialog.requestedSource, "netease");
            dialog.close();
        }
        function test_partial_results_remain_visible_with_optional_diagnostics() {
            testService.update({
                status: "candidates",
                partialResults: true,
                candidates: [
                    {
                        title: "Title",
                        artist: "Artist",
                        provider: "QQ 音乐",
                        note: "",
                        delta: 0
                    }
                ],
                sourceReports: [
                    {
                        provider: "LRCLIB",
                        status: "error",
                        message: "HTTP 503",
                        count: 0
                    }
                ]
            });
            const dialog = open(searchDialog);
            verify(findChild(dialog, "onlineMessage").visible);
            verify(findChild(dialog, "onlineMessage").text.indexOf("已保留可用结果") >= 0);
            verify(!findChild(dialog, "onlineSourceInfo").opened);
            compare(findChild(dialog, "onlineCandidates").count, 1);
            dialog.close();
        }
        function test_manual_search_requires_click_and_preserves_target() {
            const dialog = open(searchDialog);
            compare(testService.searches, 0);
            const title = findChild(dialog, "onlineTitle");
            compare(title.text, "Title");
            title.text = "Edited search";
            mouseClick(findChild(dialog, "onlineSearch"));
            compare(testService.searches, 1);
            compare(testService.state.context.title, "Title");
            verify(!findChild(dialog, "onlineSearch").enabled);
            mouseClick(findChild(dialog, "onlineCancelRequest"));
            verify(findChild(dialog, "onlineSearch").enabled);
            dialog.close();
        }
        function test_compact_light_dark_dialogs_use_window_items() {
            for (const theme of ["light", "dark"]) {
                UI.Theme.previewAppearance = theme;
                const dialog = open(searchDialog);
                compare(dialog.popupType, Popup.Item);
                verify(dialog.x >= 0 && dialog.y >= 0);
                verify(dialog.width + dialog.x <= stage.width);
                verify(dialog.height + dialog.y <= stage.height);
                verify(dialog.contentItem.height > 200);
                verify(findChild(dialog, "onlineSearch").visible);
                verify(!findChild(dialog, "onlineApply").enabled);
                keyClick(Qt.Key_Escape);
                tryCompare(dialog, "opened", false);
            }
        }
        function test_candidate_preview_and_application_are_separate() {
            testService.update({
                candidates: [
                    {
                        title: "Title",
                        artist: "Artist",
                        album: "Album",
                        note: "核对版本",
                        synced: true,
                        delta: 0,
                        provider: "LRCLIB"
                    }
                ]
            });
            const dialog = open(searchDialog);
            const list = findChild(dialog, "onlineCandidates");
            tryCompare(list, "count", 1);
            tryVerify(() => list.itemAtIndex(0) !== null);
            mouseClick(list.itemAtIndex(0));
            compare(testService.previews, 1);
            compare(testService.choices, 0);
            verify(findChild(dialog, "onlineApply").enabled);
            mouseClick(findChild(dialog, "onlineApply"));
            compare(testService.choices, 1);
            dialog.close();
        }
        function test_apply_waits_for_acknowledgement_and_blocks_duplicate_clicks() {
            testService.update({
                candidates: [
                    {
                        id: "1",
                        title: "Title",
                        artist: "Artist",
                        synced: true,
                        note: "",
                        delta: 0
                    }
                ],
                selected: 0,
                status: "preview"
            });
            const dialog = open(searchDialog);
            const button = findChild(dialog, "onlineApply");
            compare(button.text, "应用并关闭");
            mouseClick(button);
            compare(testService.choices, 1);
            compare(button.text, "正在应用…");
            verify(!button.enabled && dialog.opened);
            mouseClick(button);
            compare(testService.choices, 1);
            testService.applied("other-song", "abc", "lyrics", "歌词已应用");
            verify(dialog.opened);
            testService.update({
                busy: false,
                status: "saved"
            });
            verify(dialog.opened);
            testService.applied("song", "abc", "lyrics", "歌词已应用");
            tryCompare(dialog, "opened", false);
        }
        function test_apply_failure_keeps_dialog_and_allows_retry() {
            testService.update({
                candidates: [
                    {
                        id: "1",
                        title: "Title",
                        artist: "Artist",
                        note: "",
                        synced: true,
                        delta: 0
                    }
                ],
                selected: 0,
                status: "preview"
            });
            const dialog = open(searchDialog);
            const button = findChild(dialog, "onlineApply");
            mouseClick(button);
            testService.update({
                busy: false,
                status: "error",
                message: "磁盘空间不足"
            });
            verify(dialog.opened);
            verify(button.enabled);
            compare(button.text, "应用并关闭");
            mouseClick(button);
            compare(testService.choices, 2);
            dialog.close();
        }
        function test_empty_results_and_completed_batch_have_actionable_feedback() {
            testService.update({
                status: "missing"
            });
            const search = open(searchDialog);
            verify(findChild(search, "onlineEmptyState").text.indexOf("导入本地") >= 0);
            search.close();
            testService.update({
                batchTotal: 2,
                batchDone: 2,
                batchRemaining: 0,
                batchPaused: true,
                batchKind: "lyrics",
                batchCounts: {
                    review: 1,
                    missing: 1
                }
            });
            const batch = open(batchDialog);
            compare(findChild(batch, "onlineBatchClose").text, "关闭");
            verify(findChild(batch, "batchOutcomeSummary").text.indexOf("待确认 1") >= 0);
            verify(findChild(batch, "batchOutcomeSummary").text.indexOf("暂无资料 1") >= 0);
            batch.close();
        }
        function test_waiting_keeps_remaining_and_cancelled_items_can_be_retried() {
            testService.update({
                batchTotal: 1,
                batchDone: 0,
                batchRemaining: 1,
                batchCancelled: 0,
                batchRetryable: 0,
                batchPaused: false,
                batchPhase: "waiting",
                batchRetryAt: Date.now() + 60000,
                batchMessage: "musicbrainz.org 暂时繁忙 · 等待后自动重试",
                batchKind: "cover"
            });
            const dialog = open(batchDialog);
            const status = findChild(dialog, "batchStatusText");
            verify(status.text.indexOf("自动重试") >= 0);
            verify(findChild(dialog, "batchProgressSummary").text.indexOf("剩余 1") >= 0);
            verify(findChild(dialog, "batchPauseResume").enabled);
            verify(findChild(dialog, "batchCancel").enabled);
            verify(!findChild(dialog, "batchStart").enabled);
            mouseClick(findChild(dialog, "batchCancel"));
            verify(findChild(dialog, "batchProgressSummary").text.indexOf("已取消 1") >= 0);
            const retry = findChild(dialog, "batchRetryFailed");
            verify(retry.visible && retry.enabled);
            mouseClick(retry);
            compare(testService.state.batchRemaining, 1);
            compare(testService.state.batchCancelled, 0);
            compare(testService.batchCommands, 2);
            dialog.close();
        }
        function test_manual_wait_is_cancellable_and_displays_provider_countdown() {
            testService.update({
                status: "waiting",
                busy: true,
                retryAt: Date.now() + 30000,
                message: "musicbrainz.org 等待后自动重试"
            });
            const dialog = open(searchDialog);
            verify(findChild(dialog, "onlineMessage").text.indexOf("秒后自动重试") >= 0);
            verify(findChild(dialog, "onlineCancelRequest").visible);
            verify(!findChild(dialog, "onlineSearch").enabled);
            mouseClick(findChild(dialog, "onlineCancelRequest"));
            verify(findChild(dialog, "onlineSearch").enabled);
            dialog.close();
        }
        function test_waiting_and_cancelled_batch_layout_remains_bounded_in_both_themes() {
            for (const theme of ["light", "dark"]) {
                UI.Theme.previewAppearance = theme;
                testService.update({
                    batchTotal: 1,
                    batchDone: 0,
                    batchRemaining: 0,
                    batchCancelled: 1,
                    batchRetryable: 1,
                    batchPaused: true,
                    batchPhase: "cancelled",
                    batchMessage: "任务已取消，已保存的资料继续保留",
                    batchResults: [
                        {
                            status: "cancelled",
                            title: "起风了 (Live)",
                            artist: "林俊杰",
                            message: "本次已取消，可重新查询",
                            context: testService.state.context,
                            kind: "cover"
                        }
                    ]
                });
                const dialog = open(batchDialog);
                wait(20);
                const retry = findChild(dialog, "batchRetryFailed");
                const list = findChild(dialog, "batchResultList");
                verify(list.height >= 80);
                const pos = retry.mapToItem(dialog.contentItem, 0, 0);
                verify(pos.x >= 0 && pos.x + retry.width <= dialog.contentItem.width);
                verify(pos.y >= 0 && pos.y + retry.height <= dialog.contentItem.height);
                compare(findChild(dialog, "onlineBatchClose").text, "关闭");
                keyClick(Qt.Key_Escape);
                tryCompare(dialog, "opened", false);
            }
        }
        function test_unknown_album_scope_is_hidden() {
            testService.update({
                kind: "cover"
            });
            const dialog = open(searchDialog);
            verify(!findChild(dialog, "onlineAlbumScope").visible);
            testService.update({
                context: Object.assign({}, testService.state.context, {
                    identity: "known",
                    albumKey: "known-album",
                    scope: "album"
                })
            });
            tryCompare(findChild(dialog, "onlineAlbumScope"), "visible", true);
            verify(findChild(dialog, "onlineAlbumScope").checked);
            dialog.close();
        }
        function test_cover_apply_requires_decoded_preview() {
            testService.update({
                kind: "cover",
                candidates: [
                    {
                        title: "Album",
                        artist: "Artist",
                        date: "2000",
                        album: "Album",
                        note: "",
                        provider: "Cover Art Archive"
                    }
                ],
                selected: 0
            });
            const dialog = open(searchDialog);
            verify(!findChild(dialog, "onlineApply").enabled);
            // Empty or failed image downloads keep the selection uncommitted.
            testService.update({
                busy: true
            });
            verify(!findChild(dialog, "onlineApply").enabled);
            dialog.close();
        }
        function test_batch_pause_resume_and_cancel_have_distinct_actions() {
            testService.update({
                batchTotal: 3,
                batchDone: 1,
                batchRemaining: 2,
                batchPaused: true
            });
            const dialog = open(batchDialog);
            compare(dialog.popupType, Popup.Item);
            verify(!findChild(dialog, "batchStart").enabled);
            mouseClick(findChild(dialog, "batchPauseResume"));
            verify(!testService.state.batchPaused);
            mouseClick(findChild(dialog, "batchPauseResume"));
            verify(testService.state.batchPaused);
            mouseClick(findChild(dialog, "batchCancel"));
            compare(testService.state.batchRemaining, 0);
            compare(testService.batchCommands, 3);
            dialog.close();
        }
    }
}
