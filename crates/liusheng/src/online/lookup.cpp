#include "worker.h"
#include "../online_service.h"
#include <QtCore/QFileInfo>
#include <stdexcept>

namespace liusheng::online {
void Worker::publishCandidates() {
    QVariantList list;
    for (const auto &value : candidates_) {
        auto candidate = value.toObject(); candidate.remove("text");
        list.append(candidate.toVariantMap());
    }
    post({{"candidates", list}, {"selected", selected_}});
}
void Worker::lookup(Job job, bool fresh) {
    job.query = cleanQuery(job.query);
    active_ = job;
    if (blocked_) { finish("error", QStringLiteral("本次运行已关闭在线资料请求")); return; }
    if (job.mode == "manual") post({{"busy", true}, {"status", "searching"}, {"message", QStringLiteral("正在查找资料…")}, {"previewUrl", QString()}, {"previewText", QString()},
        {"queryHint", QStringLiteral("检索：%1 · %2 · %3").arg(job.query["title"].toString(), job.query["artist"].toString(), job.query["album"].toString())}});
    if (job.mode != "manual" && !store_.binding(job.context, job.kind).isEmpty()) {finish("kept", QStringLiteral("已有资料已保留")); return;}
    const auto queryKey = lookupCacheKey(job.query, job.kind);
    if (!fresh) {
        const auto cached = store_.cached(queryKey);
        if (cached["candidates"].isArray()) {receiveCandidates(cached["candidates"].toArray()); return;}
    }
    if (sourceSelection(job.query["_source"].toString(), job.kind) != "primary") { lookupMultiple(job); return; }
    lookupAttempt(job, queryKey, lookupPlan(job.query, job.kind), 0);
}
void Worker::lookupAttempt(Job job, QString queryKey, QList<QUrl> plan, int attempt) {
    if (!active_ || stopped_) return;
    if (attempt >= plan.size()) { store_.cache(queryKey, {}); receiveCandidates({}); return; }
    // The finite query plan and per-request retry bound cap work. Server-requested
    // cooldowns are asynchronous and excluded from active network deadlines.
    if (job.mode == "manual") post({{"attempts", attempt + 1}, {"message", attempt == 0 ? QStringLiteral("正在按完整歌名与艺术家查找…") : QStringLiteral("正在扩大检索范围（%1/%2），版本由你确认…").arg(attempt + 1).arg(plan.size())}});
    request(plan[attempt], false, [this, job, queryKey, plan, attempt](const QByteArray &bytes, int code, const QString &error) {
        try {
            if (!error.isEmpty()) {finish((code == 429 || code == 503 || code == 502 || code == 504 || code == 500 || code == 408) ? "retry" : "error", error); return;}
            QJsonArray candidates;
            if (code != 404) {
                QJsonParseError parseError;
                const auto document = QJsonDocument::fromJson(bytes, &parseError);
                if (parseError.error != QJsonParseError::NoError || (job.kind == "lyrics" ? !document.isArray() : !document.isObject())) {
                    finish("error", QStringLiteral("来源响应格式无效")); return;
                }
                candidates = job.kind == "lyrics" ? lyricsCandidates(document.array(), job.query) : coverCandidates(document.object(), job.query);
                if (attempt > 0 || (job.kind == "cover" && known(job.query["album"].toString()) && plan[attempt].path().endsWith("/recording/"))) {
                    QJsonArray related;
                    for (const auto &value : candidates) {
                        auto candidate = value.toObject();
                        if (job.kind == "lyrics" && !relatedTitle(candidate["title"].toString(), job.query["title"].toString())) continue;
                        if (job.kind == "cover" && !relatedTitle(candidate["title"].toString(), job.query["album"].toString())
                            && !relatedTitle(candidate["recordingTitle"].toString(), job.query["title"].toString())) continue;
                        candidate["confident"] = false;
                        candidate["manualOnly"] = true;
                        candidate["note"] = QStringLiteral("扩展检索，需确认演唱者、版本与时间轴。") + candidate["note"].toString();
                        related.append(candidate);
                    }
                    candidates = related;
                }
            }
            if (candidates.isEmpty() && attempt + 1 < plan.size()) {
                lookupAttempt(job, queryKey, plan, attempt + 1); return;
            }
            store_.cache(queryKey, candidates);
            receiveCandidates(candidates);
        } catch (const std::exception &exception) {finish("error", QString::fromUtf8(exception.what()));}
    });
}
void Worker::receiveCandidates(QJsonArray candidates) {
    if (!active_) return;
    const auto job = *active_;
    if (job.mode == "manual") {
        candidates_ = std::move(candidates); selected_ = -1; publishCandidates();
        finish(candidates_.isEmpty() ? "missing" : "candidates", candidates_.isEmpty() ? QStringLiteral("当前来源暂无可用结果。已尝试完整名称与扩展检索，可修改搜索词或导入本地资料。") : QStringLiteral("请选择候选并预览，确认版本后应用"));
        return;
    }
    const int selected = automaticChoice(candidates);
    if (selected < 0) {
        finish(candidates.isEmpty() ? "missing" : "review", candidates.isEmpty() ? QStringLiteral("当前来源暂无可用结果；点击此行修改搜索词或导入本地资料") : QStringLiteral("找到多个或近似候选，请手动确认版本")); return;
    }
    const auto candidate = candidates[selected].toObject();
    if (job.kind == "cover") fetchCover(candidate, true);
    else if (candidate["lyricsPending"].toBool()) fetchProviderLyrics(candidate, true);
    else save(job.context, job.kind, candidate, {}, false);
}
void Worker::fetchCover(QJsonObject candidate, bool saveImage, bool groupFallback, int sizeAttempt) {
    if (!active_) return;
    const auto job = *active_;
    if (QStringList{"netease", "qqmusic", "deezer"}.contains(candidate["providerId"].toString())) { fetchProviderCover(candidate, saveImage); return; }
    groupFallback = groupFallback || candidate["groupFallback"].toBool();
    const auto id = candidate[groupFallback ? "group" : "id"].toString();
    if (!mbid(id)) { finish("error", QStringLiteral("发行版本标识无效")); return; }
    const QStringList suffixes = {"/front-1200", "/front-500", "/front"};
    const QUrl url("https://coverartarchive.org/" + QString(groupFallback ? "release-group/" : "release/") + id + suffixes.value(sizeAttempt, "/front"));
    request(url, true, [this, candidate, saveImage, groupFallback, sizeAttempt, job](const QByteArray &bytes, int code, const QString &error) mutable {
        try {
            if (!error.isEmpty()) {finish((code == 429 || code == 503 || code == 502 || code == 504 || code == 500 || code == 408) ? "retry" : "error", error); return;}
            if (code == 404) {
                if (sizeAttempt < 2) { fetchCover(candidate, saveImage, groupFallback, sizeAttempt + 1); return; }
                if (!groupFallback && job.mode == "manual" && mbid(candidate["group"].toString())) {
                    fetchCover(candidate, saveImage, true); return;
                }
                finish("cover-missing", QStringLiteral("已找到发行记录，但图片来源缺少可用封面。可选择其他版本或导入本地图片。")); return;
            }
            if (groupFallback) {
                candidate["note"] = QStringLiteral("发行组后备封面，可能来自其他发行版本；请核对图片");
                candidate["sourceUrl"] = "https://musicbrainz.org/release-group/" + candidate["group"].toString();
                candidate["groupFallback"] = true;
            }
            if (saveImage) {save(job.context, job.kind, candidate, bytes, false); return;}
            candidate["previewUrl"] = store_.previewImage(bytes);
            if (selected_ >= 0 && selected_ < candidates_.size()) candidates_[selected_] = candidate;
            publishCandidates();
            post({{"previewUrl", candidate["previewUrl"].toString()}, {"source", candidate["provider"].toString()}});
            finish("preview", groupFallback ? candidate["note"].toString() : QStringLiteral("封面预览已就绪，可选择应用到当前歌曲或整个专辑"));
        } catch (const std::exception &exception) {finish("error", QString::fromUtf8(exception.what()));}
    });
}
void Worker::save(QJsonObject context, const QString &kind, QJsonObject candidate, QByteArray image, bool pinned) {
    const auto binding = store_.save(context, kind, candidate, image, pinned);
    notifyResource(binding);
    post({{"source", binding["provider"].toString()}});
    finish("saved", QStringLiteral("已保存"));
    if (pinned) {
        const auto identity = context["identity"].toString();
        const auto key = binding["key"].toString();
        const auto message = kind == "cover" ? QStringLiteral("封面已应用") : QStringLiteral("歌词已应用");
        // Acknowledgement follows durable storage and the resource refresh notification.
        QMetaObject::invokeMethod(owner_, [owner = owner_, identity, key, kind, message] {
            emit owner->applied(identity, key, kind, message);
        }, Qt::QueuedConnection);
    }
}
}
