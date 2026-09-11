#pragma once
// Explicit opt-in smoke: public sample titles, temporary storage, metadata-only
// output. Normal CI uses loopback fixtures and never depends on public services.
#include <QtCore/QElapsedTimer>
#include <QtCore/QJsonDocument>
#include <QtCore/QTemporaryDir>
#include <QtCore/QThread>
#include <cstdio>

inline bool waitOnline(liusheng::OnlineService &service, const std::function<bool(const QVariantMap &)> &done, int deadline = 22000) {
    QElapsedTimer elapsed; elapsed.start();
    while (elapsed.elapsed() < deadline) {
        QCoreApplication::processEvents(QEventLoop::AllEvents, 10);
        if (done(service.state())) return true;
        QThread::msleep(5);
    }
    return false;
}
inline int onlineReportedCoverSmoke() {
    using liusheng::OnlineService;
    QTemporaryDir root;
    auto target = context("reported-live-cover");
    target["title"] = QStringLiteral("起风了 (Live)");
    target["artist"] = QStringLiteral("林俊杰(百视听音乐mp3bst.com)");
    target["albumArtist"] = target["artist"];
    target["album"] = target["title"];
    target["duration"] = 315;
    OnlineService service;
    service.configureTest(QUrl(), 15000);
    service.setStorageRoot(root.path());
    service.open(json(target), "cover");
    waitOnline(service, [](const auto &state) { return state["context"].toMap().contains("trackKey"); });
    service.search(target["title"].toString(), target["artist"].toString(), target["album"].toString());
    waitOnline(service, [](const auto &state) { const auto status = state["status"].toString(); return status == "candidates" || status == "missing" || status == "error" || status == "retry"; }, 150000);
    QJsonArray metadata;
    const auto candidates = service.state()["candidates"].toList();
    for (const auto &value : candidates) {
        const auto c = value.toMap();
        metadata.append(QJsonObject{{"id", c["id"].toString()}, {"title", c["title"].toString()}, {"artist", c["artist"].toString()},
            {"recordingTitle", c["recordingTitle"].toString()}, {"manualOnly", c["manualOnly"].toBool()}});
    }
    bool preview = false;
    for (int i = 0; i < std::min(2, int(candidates.size())) && !preview; ++i) {
        service.preview(i);
        waitOnline(service, [](const auto &state) {return state["busy"].toBool();}, 1000);
        waitOnline(service, [](const auto &state) {return !state["busy"].toBool();}, 120000);
        const auto local = QUrl(service.state()["previewUrl"].toString());
        preview = local.isLocalFile() && !QImage(local.toLocalFile()).isNull();
    }
    const auto bytes = QJsonDocument(QJsonObject{{"passed", preview}, {"sample", target["title"]}, {"candidates", metadata},
        {"decodedLocalPreview", preview}, {"status", service.state()["status"].toString()}, {"message", service.state()["message"].toString()},
        {"scope", "production HTTPS, reported public metadata and temporary files; playback version still requires user confirmation"}}).toJson();
    std::fwrite(bytes.constData(), 1, size_t(bytes.size()), stdout);
    return preview ? 0 : 1;
}
inline int onlineLiveSmoke() {
    using liusheng::OnlineService;
    QTemporaryDir root;
    QJsonArray results;
    bool passed = true;
    const bool reported = QCoreApplication::arguments().contains("--reported-cases");
    const QList<QJsonObject> samples = reported ? QList<QJsonObject>{
        {{"title", QStringLiteral("伟大的渺小 (Jazz Version)")}, {"artist", QStringLiteral("林俊杰")}, {"album", QStringLiteral("感爵这一刻")}, {"duration", 299}},
        {{"title", QStringLiteral("起风了 (Live)")}, {"artist", QStringLiteral("林俊杰(百视听音乐mp3bst.com)")}, {"album", ""}, {"duration", 0}}
    } : QList<QJsonObject>{
        {{"title", "Yesterday"}, {"artist", "The Beatles"}, {"album", "Help!"}, {"duration", 125}},
        {{"title", QStringLiteral("晴天")}, {"artist", QStringLiteral("周杰伦")}, {"album", QStringLiteral("叶惠美")}, {"duration", 269}}
    };
    int number = 0;
    for (const auto &sample : samples) {
        auto target = context("public-sample-" + QString::number(number++));
        for (auto it = sample.begin(); it != sample.end(); ++it) target[it.key()] = it.value();
        OnlineService service;
        service.configureTest(QUrl(), 15000);
        service.setStorageRoot(root.path() + '/' + QString::number(number));
        service.open(json(target), "lyrics");
        waitOnline(service, [](const auto &state) { return state["context"].toMap().contains("trackKey"); });
        service.search(target["title"].toString(), target["artist"].toString(), target["album"].toString());
        const bool finished = waitOnline(service, [](const auto &state) { const auto value = state["status"].toString(); return value == "candidates" || value == "missing" || value == "error" || value == "retry"; });
        const auto state = service.state();
        const bool valid = finished && state["status"].toString() == "candidates";
        passed = passed && valid;
        QJsonArray metadata;
        for (const auto &value : state["candidates"].toList().mid(0, 8)) {
            const auto candidate = value.toMap();
            metadata.append(QJsonObject{{"title", candidate["title"].toString()}, {"artist", candidate["artist"].toString()}, {"album", candidate["album"].toString()}, {"duration", candidate["duration"].toDouble()}, {"manualOnly", candidate["manualOnly"].toBool()}});
        }
        results.append(QJsonObject{{"provider", "LRCLIB"}, {"sample", sample["title"]}, {"status", state["status"].toString()}, {"candidates", state["candidates"].toList().size()}, {"metadata", metadata}, {"passed", valid}, {"message", state["message"].toString()}});
    }
    auto album = context("cover-sample");
    album["title"] = reported ? QStringLiteral("才二十三") : QStringLiteral("Yesterday");
    album["album"] = reported ? QStringLiteral("才二十三") : QStringLiteral("Help!");
    album["artist"] = reported ? QStringLiteral("方大同") : QStringLiteral("The Beatles");
    album["albumArtist"] = album["artist"];
    OnlineService service;
    service.configureTest(QUrl(), 15000);
    service.setStorageRoot(root.path()+"/cover"); service.open(json(album), "cover");
    waitOnline(service, [](const auto &state) { return state["context"].toMap().contains("trackKey"); });
    service.search(album["title"].toString(), album["artist"].toString(), album["album"].toString());
    waitOnline(service, [](const auto &state) { const auto value = state["status"].toString(); return value == "candidates" || value == "missing" || value == "error" || value == "retry"; });
    const int count = service.state()["candidates"].toList().size();
    bool decoded = false;
    for (int i = 0; i < std::min(count, 3) && !decoded; ++i) {
        service.preview(i);
        waitOnline(service, [](const auto &state) { return state["busy"].toBool(); }, 1000);
        waitOnline(service, [](const auto &state) { return !state["busy"].toBool(); }, 35000);
        decoded = service.state()["status"].toString() == "preview" && QUrl(service.state()["previewUrl"].toString()).isLocalFile();
    }
    passed = passed && decoded;
    results.append(QJsonObject{{"provider", "MusicBrainz / Cover Art Archive"}, {"sample", album["artist"].toString() + " / " + album["album"].toString()}, {"candidates", count}, {"decodedLocalPreview", decoded}, {"status", service.state()["status"].toString()}, {"message", service.state()["message"].toString()}, {"passed", decoded}});
    const auto report = QJsonDocument(QJsonObject{{"passed", passed}, {"scope", "production HTTPS, fixed public samples and temporary resources; coverage is not estimated"}, {"results", results}}).toJson();
    std::fwrite(report.constData(), 1, size_t(report.size()), stdout);
    return passed ? 0 : 1;
}
