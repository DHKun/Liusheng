#include "online_service.h"
#include "online/worker.h"
#include <QtCore/QCoreApplication>
#include <QtCore/QDir>
#include <QtCore/QFileInfo>
#include <QtCore/QUuid>
#include <QtCore/QTimer>
#include <QtGui/QDesktopServices>

namespace liusheng {
OnlineService::OnlineService(QObject *parent) : QObject(parent) {
    state_ = {{"busy", false}, {"status", "idle"}, {"message", QStringLiteral("选择歌曲或专辑，查找在线资料")},
        {"kind", "lyrics"}, {"sourceReports", QVariantList()}, {"partialResults", false}, {"context", QVariantMap()}, {"candidates", QVariantList()}, {"selected", -1},
        {"previewUrl", QString()}, {"previewText", QString()}, {"source", QString()},
        {"batchTotal", 0}, {"batchDone", 0}, {"batchRemaining", 0}, {"batchPaused", true}, {"batchResults", QVariantList()},
        {"batchPhase", "idle"}, {"batchCancelled", 0}, {"batchRetryable", 0}, {"batchRetryAt", qint64(0)}, {"batchMessage", QString()}, {"retryAt", qint64(0)},
        {"automaticMessage", QString()}, {"automaticStatus", QString()}};
    const auto arguments = QCoreApplication::arguments();
    for (const auto &flag : {"--no-online-metadata", "--smoke-test", "--output-smoke-test", "--startup-benchmark", "--ui-test", "--functional-test"})
        if (arguments.contains(flag)) blocked_ = true;
}
OnlineService::~OnlineService() {
    if (worker_) {
        QMetaObject::invokeMethod(worker_, [worker = worker_] {worker->shutdown();}, Qt::BlockingQueuedConnection);
        thread_.quit(); thread_.wait();
    }
}
void OnlineService::setStorageRoot(const QString &root) {
    if (root_ == root || worker_) return;
    if (!QDir::isAbsolutePath(root)) return;
    root_ = root; emit storageRootChanged();
}
void OnlineService::setAutoCovers(bool value) {
    if (covers_ == value) return;
    covers_ = value; emit preferencesChanged();
    if (worker_) dispatch({{"action", "preferences"}, {"covers", covers_}, {"lyrics", lyrics_}, {"extraSources", extras_}});
}
void OnlineService::setAutoLyrics(bool value) {
    if (lyrics_ == value) return;
    lyrics_ = value; emit preferencesChanged();
    if (worker_) dispatch({{"action", "preferences"}, {"covers", covers_}, {"lyrics", lyrics_}, {"extraSources", extras_}});
}
void OnlineService::setAutoExtraSources(bool value) {
    if (extras_ == value) return;
    extras_ = value; emit preferencesChanged();
    if (worker_) dispatch({{"action", "preferences"}, {"covers", covers_}, {"lyrics", lyrics_}, {"extraSources", extras_}});
}
void OnlineService::dispatch(QVariantMap command) {
    if (!worker_) {
        if (!QDir::isAbsolutePath(root_)) {receive({{"message", QStringLiteral("资料目录尚未准备完成")}}); return;}
        worker_ = new online::Worker(this, root_, blocked_);
#ifdef LIUSHENG_ONLINE_TEST
        worker_->testServer = testServer_; worker_->deadlineMs = testDeadline_;
#endif
        worker_->moveToThread(&thread_);
        connect(&thread_, &QThread::finished, worker_, &QObject::deleteLater);
        thread_.setObjectName("liusheng-online-assets"); thread_.start();
        const QVariantMap preferences{{"action", "preferences"}, {"covers", covers_}, {"lyrics", lyrics_}, {"extraSources", extras_}};
        QMetaObject::invokeMethod(worker_, [worker = worker_, preferences] {worker->command(preferences);}, Qt::QueuedConnection);
    }
    QMetaObject::invokeMethod(worker_, [worker = worker_, command] {worker->command(command);}, Qt::QueuedConnection);
}
void OnlineService::receive(const QVariantMap &state) {
    bool different = false;
    for (auto it = state.begin(); it != state.end(); ++it) if (state_.value(it.key()) != it.value()) {
        state_[it.key()] = it.value(); different = true;
    }
    if (different) emit changed();
}
void OnlineService::open(const QString &contextJson, const QString &kind) {pendingImport_.clear(); dispatch({{"action", "open"}, {"context", contextJson}, {"kind", kind}});}
void OnlineService::close() {
    if (!pendingImport_.isEmpty()) {
        pendingImport_.clear();
        receive({{"busy", false}, {"status", "idle"}});
    }
    if (worker_) dispatch({{"action", "close"}});
}
void OnlineService::search(const QString &title, const QString &artist, const QString &album) {pendingImport_.clear(); dispatch({{"action", "search"}, {"title", title}, {"artist", artist}, {"album", album}});}
void OnlineService::searchWithSource(const QString &title, const QString &artist, const QString &album, const QString &source) {
    pendingImport_.clear(); dispatch({{"action", "search"}, {"title", title}, {"artist", artist}, {"album", album}, {"source", source}});
}
void OnlineService::startBatchWithSource(const QString &contextsJson, const QString &kind, const QString &source) {
    dispatch({{"action", "batchStart"}, {"contexts", contextsJson}, {"kind", kind}, {"source", source}});
}
void OnlineService::preview(int index) {dispatch({{"action", "preview"}, {"index", index}});}
void OnlineService::choose(bool albumScope) {
    if (state_["busy"].toBool() || state_["selected"].toInt() < 0) return;
    receive({{"busy", true}, {"status", "saving"}, {"message", QStringLiteral("正在应用选定资料…")}});
    dispatch({{"action", "choose"}, {"albumScope", albumScope}});
}
void OnlineService::restoreDefault(bool albumScope) {pendingImport_.clear(); dispatch({{"action", "clear"}, {"albumScope", albumScope}});}
void OnlineService::importLocal(const QUrl &file, bool albumScope) {
    if (state_["busy"].toBool()) return;
    if (!file.isLocalFile() || !QDir::isAbsolutePath(file.toLocalFile())) {
        receive({{"busy", false}, {"status", "error"}, {"message", QStringLiteral("请选择本地资料文件")}});
        return;
    }
    receive({{"busy", true}, {"status", "saving"}, {"message", QStringLiteral("正在导入资料…")}});
    const auto suffix = QFileInfo(file.toLocalFile()).suffix().toLower();
    if (state_["kind"].toString() == "lyrics" && QStringList{"ttml", "qrc", "yrc", "alrc"}.contains(suffix)) {
        importContext_ = state_["context"].toMap();
        pendingImport_ = QUuid::createUuid().toString(QUuid::WithoutBraces);
        const auto id = pendingImport_;
        QTimer::singleShot(10000, this, [this, id] {
            if (pendingImport_ != id) return;
            pendingImport_.clear();
            receive({{"busy", false}, {"status", "error"}, {"message", QStringLiteral("歌词文件读取超时，请确认文件所在磁盘可用")}});
        });
        emit lyricImportRequested(id, file.toString());
        return;
    }
    dispatch({{"action", "import"}, {"file", file.toString()}, {"albumScope", albumScope}});
}
void OnlineService::completeLyricImport(const QString &requestId, const QString &text, bool wordTimed, const QString &error) {
    if (pendingImport_.isEmpty() || requestId != pendingImport_) return;
    pendingImport_.clear();
    if (state_["kind"].toString() != "lyrics" || state_["context"].toMap() != importContext_) return;
    if (!error.isEmpty() || text.toUtf8().size() > online::LyricLimit || text.isEmpty()) {
        receive({{"busy", false}, {"status", "error"}, {"message", error.isEmpty() ? QStringLiteral("歌词文件超出限制") : error}});
        return;
    }
    // The text is accepted only as the result of the requested Rust parser job.
    dispatch({{"action", "verifiedLyricImport"}, {"context", importContext_}, {"text", text}, {"wordTimed", wordTimed}});
}
void OnlineService::cancel() {pendingImport_.clear(); if (worker_) dispatch({{"action", "cancel"}});}
void OnlineService::requestAutomatic(const QString &contextsJson) {
    if (blocked_ || (!covers_ && !lyrics_)) return;
    dispatch({{"action", "automatic"}, {"contexts", contextsJson}});
}
void OnlineService::startBatch(const QString &contextsJson, const QString &kind) {dispatch({{"action", "batchStart"}, {"contexts", contextsJson}, {"kind", kind}});}
void OnlineService::inspectBatch() {dispatch({{"action", "batchInspect"}});}
void OnlineService::pauseBatch() {dispatch({{"action", "batchPause"}});}
void OnlineService::resumeBatch() {dispatch({{"action", "batchResume"}});}
void OnlineService::retryBatchFailures() {dispatch({{"action", "batchRetryFailed"}});}
void OnlineService::cancelBatch() {dispatch({{"action", "batchCancel"}});}
void OnlineService::clearSearchCache() {dispatch({{"action", "clearCache"}});}
void OnlineService::openSource() {
    const auto candidates = state_["candidates"].toList();
    const auto selected = state_["selected"].toInt();
    if (selected < 0 || selected >= candidates.size()) return;
    const QUrl url(candidates[selected].toMap()["sourceUrl"].toString());
    if (online::safeSourcePage(url)) QDesktopServices::openUrl(url);
}
#ifdef LIUSHENG_ONLINE_TEST
void OnlineService::configureTest(const QUrl &server, int deadlineMs) {
    if (worker_) return;
    testServer_ = server; testDeadline_ = deadlineMs; blocked_ = false;
}
#endif
}
