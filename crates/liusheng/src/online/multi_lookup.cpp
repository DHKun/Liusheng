#include "worker.h"
#include <stdexcept>

namespace liusheng::online {
void Worker::lookupMultiple(const Job &job) {
    auto run = std::make_shared<SourceSearch>();
    run->job = job;
    run->providers = providerOrder(sourceSelection(job.query["_source"].toString(), job.kind), job.kind, job.query);
    multi_ = run;
    nextProvider(run);
}
void Worker::nextProvider(const std::shared_ptr<SourceSearch> &run) try {
    if (multi_ != run || !active_ || stopped_) return;
    if (run->providerIndex >= run->providers.size()) {
        auto candidates = run->candidates;
        if (run->failed) {
            // Partial source coverage is suitable for manual selection. Keep
            // automatic application conservative while any provider is missing.
            QJsonArray reviewed;
            for (const auto &value : candidates) {
                auto row = value.toObject(); row["confident"] = false; row["manualOnly"] = true; reviewed.append(row);
            }
            candidates = reviewed;
        } else store_.cache(lookupCacheKey(run->job.query, run->job.kind), candidates);
        if (run->job.mode == "manual") post({{"sourceReports", run->reports}, {"partialResults", run->failed > 0}});
        multi_.reset();
        if (candidates.isEmpty() && run->failed) {
            // Errors never become a negative search-cache entry.
            finish(run->job.mode == "batch" ? "error" : "retry", QStringLiteral("部分来源暂不可用，请稍后重试"));
        } else receiveCandidates(candidates);
        return;
    }
    run->providerClock.start();
    const auto provider = run->providers[run->providerIndex];
    if (run->job.mode == "manual") post({{"status", "searching"}, {"busy", true}, {"message", QStringLiteral("正在查找 · %1").arg(providerName(provider))}});
    queryProvider(run, providerQueries(provider, run->job.kind, run->job.query), 0);
} catch (const std::exception &error) { finish("error", QString::fromUtf8(error.what())); }
void Worker::queryProvider(const std::shared_ptr<SourceSearch> &run, const QList<QUrl> &plan, int attempt) {
    if (multi_ != run || !active_ || stopped_) return;
    if (attempt >= plan.size()) { completeProvider(run, {}); return; }
    if (attempt > 0 && run->providerClock.elapsed() > 15000) {
        completeProvider(run, {}, QStringLiteral("查询超时")); return;
    }
    request(plan[attempt], false, [this, run, plan, attempt](const QByteArray &bytes, int code, const QString &error) {
        if (multi_ != run || !active_ || stopped_) return;
        try {
            if (!error.isEmpty()) { completeProvider(run, {}, error); return; }
            QJsonArray candidates;
            const auto provider = run->providers[run->providerIndex];
            if (code != 404) {
                QJsonParseError parseError;
                const auto document = QJsonDocument::fromJson(bytes, &parseError);
                if (parseError.error != QJsonParseError::NoError || (provider == "lrclib" ? !document.isArray() : !document.isObject())) {
                    completeProvider(run, {}, QStringLiteral("返回格式无效")); return;
                }
                if (provider == "lrclib") candidates = lyricsCandidates(document.array(), run->job.query);
                else if (provider == "musicbrainz") candidates = coverCandidates(document.object(), run->job.query);
                else candidates = catalogCandidates(provider, run->job.kind, document.object(), run->job.query);
                const bool expanded = (provider == "lrclib" || provider == "musicbrainz") && (attempt > 0
                    || (provider == "musicbrainz" && plan[attempt].path().endsWith("/recording/") && known(run->job.query["album"].toString())));
                QJsonArray checked;
                for (const auto &value : candidates) {
                    auto candidate = value.toObject(); candidate["providerId"] = provider;
                    if (expanded) {
                        if (provider == "lrclib" && !relatedTitle(candidate["title"].toString(), run->job.query["title"].toString())) continue;
                        if (provider == "musicbrainz" && !relatedTitle(candidate["title"].toString(), run->job.query["album"].toString())
                            && !relatedTitle(candidate["recordingTitle"].toString(), run->job.query["title"].toString())) continue;
                        candidate["confident"] = false; candidate["manualOnly"] = true;
                        candidate["note"] = QStringLiteral("扩展匹配，请核对版本");
                    }
                    checked.append(candidate);
                }
                candidates = checked;
            }
            if (candidates.isEmpty() && attempt + 1 < plan.size()) { queryProvider(run, plan, attempt + 1); return; }
            completeProvider(run, candidates);
        } catch (const std::exception &exception) { completeProvider(run, {}, QString::fromUtf8(exception.what())); }
    }, 0, 0, 0, run->providers.size() > 1);
}
void Worker::completeProvider(const std::shared_ptr<SourceSearch> &run, QJsonArray candidates, const QString &error) {
    if (multi_ != run || !active_ || stopped_) return;
    const auto provider = run->providers[run->providerIndex];
    run->reports.append(QVariantMap{{"provider", providerName(provider)}, {"count", candidates.size()},
        {"status", error.isEmpty() ? candidates.isEmpty() ? "missing" : "ready" : "error"}, {"message", error.left(300)}});
    if (!error.isEmpty()) ++run->failed;
    run->candidates = mergeCandidates(run->candidates, candidates);
    ++run->providerIndex;
    if (run->job.mode == "batch") {
        batch_["sourceReports"] = QJsonArray::fromVariantList(run->reports);
        batchState();
    }
    if (run->job.mode == "manual") {
        candidates_ = run->candidates; selected_ = -1; publishCandidates();
        post({{"sourceReports", run->reports}});
    }
    // Yield between sources so cancel and track replacement can be processed.
    QTimer::singleShot(0, this, [this, run] { nextProvider(run); });
}
void Worker::fetchProviderLyrics(QJsonObject candidate, bool saveText) {
    if (!active_) return;
    const auto job = *active_;
    const auto url = providerLyricUrl(candidate);
    if (url.isEmpty()) { finish("error", QStringLiteral("歌词条目无效")); return; }
    request(url, false, [this, candidate, saveText, job](const QByteArray &bytes, int code, const QString &error) mutable {
        try {
            if (!error.isEmpty()) { finish("error", error); return; }
            if (code == 404) { finish("missing", QStringLiteral("此条目暂无歌词，请选择其他结果")); return; }
            QJsonParseError parseError; const auto document = QJsonDocument::fromJson(bytes, &parseError);
            if (parseError.error != QJsonParseError::NoError || !document.isObject()) { finish("error", QStringLiteral("歌词返回格式无效")); return; }
            candidate = readProviderLyrics(candidate, document.object());
            if (saveText) { save(job.context, "lyrics", candidate, {}, false); return; }
            if (selected_ >= 0 && selected_ < candidates_.size()) candidates_[selected_] = candidate;
            publishCandidates();
            post({{"source", candidate["provider"].toString()}, {"previewText", candidate["instrumental"].toBool() ? QStringLiteral("纯音乐") : candidate["text"].toString().left(24000)}});
            finish("preview", QString());
        } catch (const std::exception &exception) { finish("error", QString::fromUtf8(exception.what())); }
    });
}
void Worker::fetchProviderCover(QJsonObject candidate, bool saveImage) {
    if (!active_) return;
    const auto job = *active_;
    const auto provider = candidate["providerId"].toString();
    const auto image = trustedImageUrl(provider, QUrl(candidate["imageUrl"].toString()));
    if (provider == "netease" && image.isEmpty() && !candidate["detailLoaded"].toBool()) {
        request(neteaseDetailUrl(candidate), false, [this, candidate, saveImage](const QByteArray &bytes, int code, const QString &error) mutable {
            try {
                if (!error.isEmpty()) { finish("error", error); return; }
                if (code == 404) { finish("cover-missing", QStringLiteral("此条目暂无封面")); return; }
                const auto document = QJsonDocument::fromJson(bytes);
                candidate["imageUrl"] = neteaseCoverUrl(candidate, document.object()).toString(); candidate["detailLoaded"] = true;
                fetchProviderCover(candidate, saveImage);
            } catch (const std::exception &exception) { finish("error", QString::fromUtf8(exception.what())); }
        });
        return;
    }
    if (image.isEmpty()) { finish("cover-missing", QStringLiteral("此条目暂无封面，请选择其他结果")); return; }
    request(image, true, [this, candidate, saveImage, job](const QByteArray &bytes, int code, const QString &error) mutable {
        try {
            if (!error.isEmpty()) { finish("error", error); return; }
            if (code == 404) { finish("cover-missing", QStringLiteral("此条目暂无封面，请选择其他结果")); return; }
            if (saveImage) { save(job.context, "cover", candidate, bytes, false); return; }
            candidate["previewUrl"] = store_.previewImage(bytes);
            if (selected_ >= 0 && selected_ < candidates_.size()) candidates_[selected_] = candidate;
            publishCandidates(); post({{"previewUrl", candidate["previewUrl"].toString()}, {"source", candidate["provider"].toString()}});
            finish("preview", QString());
        } catch (const std::exception &exception) { finish("error", QString::fromUtf8(exception.what())); }
    });
}
}
