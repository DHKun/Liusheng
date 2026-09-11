#include "worker.h"
#include <stdexcept>
#include "../online_service.h"
#include <QtCore/QStringDecoder>
#include <QtCore/QFileInfo>

namespace liusheng::online {
Worker::Worker(OnlineService *owner, QString root, bool blocked) : owner_(owner), store_(std::move(root)), blocked_(blocked) {}
void Worker::post(QVariantMap state) {
    QMetaObject::invokeMethod(owner_, [owner = owner_, state] { owner->receive(state); }, Qt::QueuedConnection);
}
void Worker::notifyResource(const QJsonObject &binding) {
    const auto id = binding["key"].toString(), kind = binding["kind"].toString();
    QMetaObject::invokeMethod(owner_, [owner = owner_, id, kind] { emit owner->resourceChanged(id, kind); }, Qt::QueuedConnection);
}
void Worker::initialize() {
    if (ready_) return;
    store_.initialize();
    batch_ = store_.readJson("batch.json", 32 * 1024 * 1024);
    if (batch_["version"].toInt() != 1) batch_ = {{"version", 1}, {"total", 0}, {"done", 0}, {"pending", QJsonArray()}, {"results", QJsonArray()}};
    normalizeBatch();
    const auto cooldowns = store_.readJson("cooldowns.json");
    for (auto it = cooldowns.begin(); it != cooldowns.end(); ++it) {
        const qint64 time = it.value().toInteger();
        if (time > QDateTime::currentMSecsSinceEpoch() && time < QDateTime::currentMSecsSinceEpoch() + 86400000) retryAt_[it.key()] = time;
    }
    batchPaused_ = true; ready_ = true; batchState();
}
void Worker::shutdown() {
    stopped_ = true; abortRequest();
    if (ready_) {
        try { persistBatch(); } catch (const std::exception &) {}
    }
}
void Worker::cancelActive() {
    abortRequest(); multi_.reset();
    if (active_ && active_->mode == "batch") { batch_["retryAt"] = 0; batch_["message"] = QString(); }
    active_.reset(); post({{"busy", false}, {"retryAt", qint64(0)}});
}
void Worker::command(const QVariantMap &message) {
    try { initialize(); dispatch(message); }
    catch (const std::exception &error) { finish("error", QString::fromUtf8(error.what())); }
}
void Worker::dispatch(const QVariantMap &message) {
    const auto action = message["action"].toString();
    if (action == "preferences") {
        covers_ = message["covers"].toBool(); lyrics_ = message["lyrics"].toBool();
        const bool extra = message["extraSources"].toBool();
        if (extras_ && !extra) {
            automatic_.clear();
            if (active_ && active_->mode == "auto") cancelActive();
        }
        extras_ = extra;
        automatic_.erase(std::remove_if(automatic_.begin(), automatic_.end(), [this](const Job &job) {return job.kind == "cover" ? !covers_ : !lyrics_;}), automatic_.end());
        if (active_ && active_->mode == "auto" && (active_->kind == "cover" ? !covers_ : !lyrics_)) cancelActive();
        pump(); return;
    }
    if (action == "open") {
        cancelActive(); manualOpen_ = true; batchState();
        context_ = QJsonDocument::fromJson(message["context"].toString().toUtf8()).object();
        kind_ = message["kind"].toString() == "cover" ? "cover" : "lyrics";
        candidates_ = {}; selected_ = -1;
        if (!validContext(context_)) {post({{"message", QStringLiteral("请选择一首歌曲或专辑")}}); return;}
        const auto saved = store_.binding(context_, kind_);
        post({{"status", saved["unavailable"].toBool() ? "warning" : "idle"}, {"context", context_.toVariantMap()}, {"kind", kind_}, {"candidates", QVariantList()}, {"selected", -1},
            {"previewUrl", QString()}, {"previewText", QString()}, {"queryHint", QString()}, {"sourceReports", QVariantList()}, {"partialResults", false}, {"attempts", 0}, {"source", saved["provider"].toString()},
            {"message", saved["unavailable"].toBool() ? QStringLiteral("选定资源缺失或损坏，请重新查找或恢复默认") : QStringLiteral("修改检索词后点击查找；结果保存到留声资料库")}});
        return;
    }
    if (action == "close") {manualOpen_ = false; batchState(); if (active_ && active_->mode == "manual") cancelActive(); pump(); return;}
    if (action == "cancel") {cancelActive(); post({{"status", "idle"}, {"message", QStringLiteral("已取消本次查找")}}); return;}
    if (action == "search") {
        cancelActive(); candidates_ = {}; selected_ = -1;
        auto query = context_;
        query["title"] = message["title"].toString().trimmed().left(512);
        query["artist"] = message["artist"].toString().trimmed().left(512);
        query["albumArtist"] = query["artist"];
        query["album"] = message["album"].toString().trimmed().left(512);
        query["_source"] = sourceSelection(message["source"].toString(), kind_);
        post({{"sourceReports", QVariantList()}, {"partialResults", false}});
        if (!validContext(query) || !known(query[kind_ == "cover" && known(query["album"].toString()) ? "album" : "title"].toString())) {
            post({{"message", QStringLiteral("请填写有效的歌曲名或专辑名")}}); return;
        }
        publishCandidates();
        lookup({context_, query, kind_, "manual"}, true); return;
    }
    if (action == "preview") {
        if (active_) cancelActive();
        selected_ = message["index"].toInt();
        if (selected_ < 0 || selected_ >= candidates_.size()) return;
        const auto candidate = candidates_[selected_].toObject();
        post({{"status", kind_ == "cover" || candidate["lyricsPending"].toBool() ? "previewing" : "preview"}, {"selected", selected_}, {"source", candidate["provider"].toString()}, {"previewUrl", QString()},
            {"previewText", candidate["instrumental"].toBool() ? QStringLiteral("来源将此曲目标记为纯音乐") : candidate["text"].toString().left(24000)}, {"message", candidate["note"].toString()}});
        if (kind_ == "lyrics" && candidate["lyricsPending"].toBool()) {
            active_ = Job{context_, context_, kind_, "manual"}; post({{"busy", true}}); fetchProviderLyrics(candidate, false);
        }
        if (kind_ == "cover") {
            active_ = Job{context_, context_, kind_, "manual"}; post({{"busy", true}}); fetchCover(candidate, false);
        }
        return;
    }
    if (action == "verifiedLyricImport") {
        const auto context = QJsonObject::fromVariantMap(message["context"].toMap());
        if (kind_ != "lyrics" || !manualOpen_ || context != context_ || !validContext(context)) return;
        abortRequest(); active_.reset();
        QJsonObject candidate{{"provider", QStringLiteral("手动导入")}, {"id", "local"},
            {"title", context["title"]}, {"text", message["text"].toString()},
            {"verifiedLyrics", true}, {"wordTimed", message["wordTimed"].toBool()}};
        save(context, "lyrics", candidate, {}, true); return;
    }
    if (action == "clear" || action == "choose" || action == "import") {
        if (!validContext(context_)) { finish("error", QStringLiteral("资料目标已失效，请重新选择歌曲")); return; }
        abortRequest(); active_.reset(); auto context = context_;
        post({{"busy", action != "clear"}, {"status", action == "clear" ? "idle" : "saving"}});
        context["scope"] = message["albumScope"].toBool() ? "album" : "track";
        if (action == "clear") {
            notifyResource(store_.clear(context, kind_));
            post({{"message", QStringLiteral("已恢复本地优先，并暂停此项自动补全")}, {"source", QString()}}); return;
        }
        if (action == "import") {
            const QUrl url(message["file"].toString());
            if (!url.isLocalFile()) { finish("error", QStringLiteral("请选择本地资料文件")); return; }
            QFile file(url.toLocalFile());
            const auto limit = kind_ == "cover" ? ImageLimit : LyricLimit;
            if (!QFileInfo(file).isFile() || !file.open(QIODevice::ReadOnly) || file.size() > limit) throw std::runtime_error("本地文件无法读取或超过大小限制");
            auto bytes = file.read(limit + 1);
            if (bytes.size() > limit) throw std::runtime_error("本地文件超过大小限制");
            QJsonObject candidate{{"provider", QStringLiteral("手动导入")}, {"id", "local"}, {"title", context["title"]}};
            if (kind_ == "lyrics") {
                QStringDecoder decoder(QStringDecoder::encodingForData(bytes).value_or(QStringDecoder::Utf8));
                const QString text = decoder(bytes);
                if (decoder.hasError()) throw std::runtime_error("歌词编码错误，请使用 UTF-8 或带 BOM 的 UTF-16");
                candidate["text"] = text;
            }
            save(context, kind_, candidate, bytes, true); return;
        }
        if (selected_ < 0 || selected_ >= candidates_.size()) { finish("error", QStringLiteral("请选择并预览资料")); return; }
        const auto candidate = candidates_[selected_].toObject();
        if (kind_ == "lyrics" && candidate["lyricsPending"].toBool()) { finish("error", QStringLiteral("请先预览歌词")); return; }
        QByteArray image;
        if (kind_ == "cover") {
            QFile file(QUrl(candidate["previewUrl"].toString()).toLocalFile());
            if (!file.fileName().startsWith(store_.root() + "/previews/") || !file.open(QIODevice::ReadOnly)) throw std::runtime_error("请先选择并预览封面");
            image = file.read(ImageLimit + 1);
        }
        save(context, kind_, candidate, image, true); return;
    }
    // Batch and automatic operations share the same bounded scheduler.
    dispatchQueue(message);
}
}
