#include "worker.h"
#include <stdexcept>

namespace liusheng::online {
namespace {
QString itemKey(const QJsonObject &item) {
    const auto kind = item["kind"].toString();
    const auto context = item["context"].toObject();
    if (!validContext(context) || (kind != "lyrics" && kind != "cover")) return {};
    return kind + ':' + bindingKey(context, kind);
}
QJsonArray uniqueItems(const QJsonArray &items, int limit = 5000) {
    QSet<QString> seen; QJsonArray result;
    for (const auto &value : items) {
        const auto id = itemKey(value.toObject());
        if (id.isEmpty() || seen.contains(id)) continue;
        seen.insert(id); result.append(value);
        if (result.size() >= limit) break;
    }
    return result;
}
}
void Worker::normalizeBatch() {
    batch_["pending"] = uniqueItems(batch_["pending"].toArray());
    batch_["results"] = uniqueItems(batch_["results"].toArray(), 200);
    const int pending = batch_["pending"].toArray().size();
    const int done = std::clamp(batch_["done"].toInt(), 0, 5000 - pending);
    const int total = std::clamp(batch_["total"].toInt(), done + pending, 5000);
    batch_["total"] = total; batch_["done"] = done;
    batch_["cancelled"] = total - done - pending;
    auto retryable = batch_["retryable"].toArray();
    // Older canceled batches kept retry history but dropped pending records.
    // Offer explicit recovery; restarting the app keeps every task paused.
    if (batch_["cancelled"].toInt() > 0 && retryable.isEmpty()) {
        for (const auto &result : batch_["results"].toArray())
            if (result.toObject()["status"].toString() == "retry") retryable.append(result);
    }
    batch_["retryable"] = uniqueItems(retryable);
    batch_["retryAt"] = 0;
}
void Worker::batchState() {
    const int remaining = batch_["pending"].toArray().size();
    QString phase = "idle";
    if (remaining > 0) phase = batchPaused_ ? "paused" : manualOpen_ ? "manual" : batch_["retryAt"].toInteger() > QDateTime::currentMSecsSinceEpoch() ? "waiting" : "running";
    else if (batch_["cancelled"].toInt() > 0) phase = "cancelled";
    else if (batch_["total"].toInt() > 0) phase = "completed";
    post({{"batchTotal", batch_["total"].toInt()}, {"batchDone", batch_["done"].toInt()},
        {"batchRemaining", remaining}, {"batchPaused", batchPaused_}, {"batchPhase", phase},
        {"batchCancelled", batch_["cancelled"].toInt()}, {"batchRetryable", batch_["retryable"].toArray().size()},
        {"batchRetryAt", batch_["retryAt"].toInteger()}, {"batchSourceReports", batch_["sourceReports"].toArray().toVariantList()}, {"batchMessage", batch_["message"].toString()},
        {"batchResults", batch_["results"].toArray().toVariantList()},
        {"batchCounts", batch_["counts"].toObject().toVariantMap()}, {"batchKind", batch_["kind"].toString()}});
}
void Worker::recordBatchResult(const Job &job, const QString &status, const QString &message) {
    const QJsonObject row{{"title", job.context["title"]}, {"artist", job.context["artist"]},
        {"status", status}, {"message", message}, {"context", job.context}, {"kind", job.kind}};
    QJsonArray results{row};
    for (const auto &previous : batch_["results"].toArray()) {
        if (itemKey(previous.toObject()) != itemKey(row)) results.append(previous);
        if (results.size() >= 200) break;
    }
    batch_["results"] = results;
}
void Worker::persistBatch() {
    store_.writeJson("batch.json", batch_);
    QJsonObject cooldowns;
    const auto now = QDateTime::currentMSecsSinceEpoch();
    for (auto it = retryAt_.begin(); it != retryAt_.end(); ++it)
        if (it.value() > now) cooldowns[it.key()] = it.value();
    store_.writeJson("cooldowns.json", cooldowns);
}
void Worker::dispatchQueue(const QVariantMap &message) {
    const auto action = message["action"].toString();
    if (action == "automatic") {
        automatic_.clear();
        if (active_ && active_->mode == "auto") cancelActive();
        const auto raw = message["contexts"].toString().toUtf8();
        if (raw.size() > 32 * 1024 * 1024) throw std::runtime_error("资料批次超过容量限制，请选择较小的范围");
        const auto contexts = QJsonDocument::fromJson(raw).array();
        for (const auto &value : contexts) {
            const auto context = value.toObject();
            if (!validContext(context)) continue;
            for (const auto &kind : {QStringLiteral("lyrics"), QStringLiteral("cover")}) {
                if ((kind == "lyrics" ? !lyrics_ || context["localLyrics"].toBool() : !covers_ || context["localCover"].toBool())) continue;
                if (!store_.binding(context, kind).isEmpty()) continue;
                auto query = context; query["_source"] = extras_ ? "all" : "primary";
                if (automatic_.size() < 4) automatic_.push_back({context, query, kind, "auto"});
            }
        }
        pump(); return;
    }
    if (action == "batchRetryFailed") {
        if (!batchPaused_) return;
        auto items = batch_["pending"].toArray();
        for (const auto &value : batch_["retryable"].toArray()) items.append(value);
        QJsonArray contexts;
        for (const auto &item : uniqueItems(items)) contexts.append(item.toObject()["context"]);
        if (!contexts.isEmpty()) dispatchQueue({{"action", "batchStart"}, {"kind", batch_["kind"].toString()}, {"source", batch_["source"].toString()},
            {"contexts", QString::fromUtf8(QJsonDocument(contexts).toJson(QJsonDocument::Compact))}});
        return;
    }
    if (action == "batchStart") {
        if (active_ && active_->mode == "batch") cancelActive();
        const auto raw = message["contexts"].toString().toUtf8();
        if (raw.size() > 32 * 1024 * 1024) throw std::runtime_error("资料批次超过容量限制，请选择较小的范围");
        const auto contexts = QJsonDocument::fromJson(raw).array();
        const auto kind = message["kind"].toString();
        if (kind != "cover" && kind != "lyrics") return;
        QJsonArray pending;
        QSet<QString> seen;
        for (const auto &value : contexts) {
            const auto context = value.toObject();
            if (!validContext(context)) continue;
            const auto id = bindingKey(context, kind);
            if (seen.contains(id) || context[kind == "cover" ? "localCover" : "localLyrics"].toBool() || !store_.binding(context, kind).isEmpty()) continue;
            seen.insert(id); pending.append(QJsonObject{{"context", context}, {"kind", kind}});
            if (pending.size() >= 5000) break;
        }
        batch_ = {{"version", 1}, {"kind", kind}, {"source", sourceSelection(message["source"].toString(), kind)}, {"counts", QJsonObject()}, {"total", pending.size()}, {"done", 0},
            {"cancelled", 0}, {"retryAt", 0}, {"retryable", QJsonArray()}, {"pending", pending}, {"results", QJsonArray()}};
        batchPaused_ = false; persistBatch(); batchState(); pump(); return;
    }
    if (action == "batchInspect") { batchState(); return; }
    if (action == "batchPause") {
        batchPaused_ = true;
        if (active_ && active_->mode == "batch") cancelActive();
        batch_["retryAt"] = 0; batch_["message"] = QStringLiteral("任务已暂停，剩余条目已保留");
        persistBatch(); batchState(); return;
    }
    if (action == "batchResume") { batchPaused_ = false; batchState(); pump(); return; }
    if (action == "batchCancel") {
        batchPaused_ = true;
        if (active_ && active_->mode == "batch") cancelActive();
        auto retryable = batch_["retryable"].toArray();
        for (const auto &value : batch_["pending"].toArray()) {
            retryable.append(value);
            const auto row = value.toObject();
            const auto context = row["context"].toObject();
            recordBatchResult({context, context, row["kind"].toString(), "batch"}, "cancelled", QStringLiteral("本次已取消，可通过重试失败/取消项重新查询"));
        }
        batch_["cancelled"] = batch_["cancelled"].toInt() + batch_["pending"].toArray().size();
        batch_["pending"] = QJsonArray(); batch_["retryable"] = uniqueItems(retryable);
        batch_["retryAt"] = 0; batch_["message"] = QStringLiteral("任务已取消，已保存的资料继续保留");
        persistBatch(); batchState(); return;
    }
    if (action == "clearCache") {
        cancelActive(); store_.clearCache();
        post({{"previewUrl", QString()}, {"message", QStringLiteral("搜索和预览缓存已清理；选定资料与来源冷却时间继续保留")}});
        pump();
    }
}
void Worker::pump() try {
    if (stopped_ || active_ || manualOpen_) return;
    if (!automatic_.empty()) {
        auto job = automatic_.front(); automatic_.pop_front(); lookup(job, false); return;
    }
    if (batchPaused_) return;
    auto pending = batch_["pending"].toArray();
    if (pending.isEmpty()) {batchPaused_ = true; batchState(); return;}
    const auto item = pending.first().toObject();
    const auto context = item["context"].toObject();
    const auto kind = item["kind"].toString();
    if (!validContext(context) || (kind != "cover" && kind != "lyrics")) {
        pending.removeFirst(); batch_["pending"] = pending;
        batch_["done"] = batch_["done"].toInt() + 1;
        persistBatch(); batchState();
        QTimer::singleShot(0, this, [this] { pump(); }); return;
    }
    auto query = context; query["_source"] = sourceSelection(batch_["source"].toString(), kind);
    lookup({context, query, kind, "batch"}, false);
} catch (const std::exception &error) {
    cancelActive(); batchPaused_ = true;
    batch_["message"] = QString::fromUtf8(error.what()); batchState();
}
void Worker::finish(const QString &status, const QString &message) {
    const auto job = active_; active_.reset(); multi_.reset();
    if (job && job->mode == "batch") {
        auto pending = batch_["pending"].toArray();
        batch_["retryAt"] = 0; batch_["message"] = message;
        if (status == "retry") {
            batchPaused_ = true;
        } else {
            // Complete exactly this task; a canceled/replaced request cannot pop
            // a different task because transport callbacks are generation-gated.
            if (!pending.isEmpty()) pending.removeFirst();
            batch_["pending"] = pending; batch_["done"] = batch_["done"].toInt() + 1;
            auto counts = batch_["counts"].toObject(); counts[status] = counts[status].toInt() + 1; batch_["counts"] = counts;
            if (status == "error") {
                auto retryable = batch_["retryable"].toArray();
                retryable.append(QJsonObject{{"context", job->context}, {"kind", job->kind}});
                batch_["retryable"] = uniqueItems(retryable);
            }
        }
        recordBatchResult(*job, status, message);
        try { persistBatch(); } catch (const std::exception &error) {
            batchPaused_ = true; batch_["message"] = QString::fromUtf8(error.what());
        }
        batchState();
    } else if (job && job->mode == "auto") {
        post({{"automaticMessage", message}, {"automaticStatus", status}, {"automaticContext", job->context.toVariantMap()}, {"automaticKind", job->kind}});
    } else post({{"busy", false}, {"retryAt", qint64(0)}, {"status", status}, {"message", message}});
    QTimer::singleShot(0, this, [this] { if (!stopped_) pump(); });
}
}
