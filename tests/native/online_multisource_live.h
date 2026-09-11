#pragma once
// Opt-in only; requests fixed public metadata, uses isolated temporary storage,
// and reports identity/counts without publishing lyric text.
inline int onlineMultisourceLive() {
    QTemporaryDir root;
    const QList<QJsonObject> samples{
        {{"provider", "qqmusic"}, {"kind", "cover"}, {"title", QStringLiteral("才二十三")}, {"artist", QStringLiteral("方大同")}, {"album", QStringLiteral("才二十三")}, {"duration", 224}},
        {{"provider", "netease"}, {"kind", "cover"}, {"title", QStringLiteral("才二十三")}, {"artist", QStringLiteral("方大同")}, {"album", QStringLiteral("才二十三")}, {"duration", 224}},
        {{"provider", "qqmusic"}, {"kind", "lyrics"}, {"title", QStringLiteral("起风了 (Live)")}, {"artist", QStringLiteral("林俊杰(百视听音乐mp3bst.com)")}, {"album", ""}, {"duration", 315}},
        {{"provider", "netease"}, {"kind", "lyrics"}, {"title", QStringLiteral("才二十三")}, {"artist", QStringLiteral("方大同")}, {"album", QStringLiteral("梦想家 The Dreamer")}, {"duration", 224}},
        {{"provider", "deezer"}, {"kind", "cover"}, {"title", "Twenty Three"}, {"artist", "Khalil Fong"}, {"album", "Twenty Three"}, {"duration", 224}}
    };
    QJsonArray reports;
    bool passed = true;
    int index = 0;
    for (const auto &sample : samples) {
        auto target = context("multisource-public-" + QString::number(++index));
        for (const auto &field : {"title", "artist", "album", "duration"}) target[field] = sample[field];
        target["albumArtist"] = target["artist"];
        const QString kind = sample["kind"].toString();
        OnlineService service; service.configureTest(QUrl(), 15000);
        const QString directory = root.path() + '/' + QString::number(index);
        service.setStorageRoot(directory);
        QSignalSpy applied(&service, &OnlineService::applied);
        QElapsedTimer elapsed; elapsed.start();
        service.open(json(target), kind);
        waitOnline(service, [](const auto &s) {return s["context"].toMap().contains("trackKey");});
        service.searchWithSource(target["title"].toString(), target["artist"].toString(), target["album"].toString(), sample["provider"].toString());
        waitOnline(service, [](const auto &s) {return QStringList{"candidates", "missing", "error", "retry"}.contains(s["status"].toString());}, 60000);
        const auto rows = service.state()["candidates"].toList();
        QVariantMap chosen;
        bool preview = false, saved = false;
        if (!rows.isEmpty()) {
            chosen = rows.first().toMap();
            service.preview(0);
            waitOnline(service, [](const auto &s) {return QStringList{"preview", "error", "retry", "cover-missing", "missing"}.contains(s["status"].toString());}, 60000);
            preview = service.state()["status"].toString() == "preview";
            if (kind == "cover") preview = preview && !QImage(QUrl(service.state()["previewUrl"].toString()).toLocalFile()).isNull();
            else preview = preview && validLyrics(service.state()["previewText"].toString());
            if (preview) {
                service.choose(false);
                waitOnline(service, [](const auto &s) {return s["status"].toString() == "saved" || s["status"].toString() == "error";});
                Store store(directory); const auto binding = store.binding(target, kind);
                saved = !binding.isEmpty() && binding["pinned"].toBool() && applied.size() == 1;
            }
        }
        const bool ok = preview && saved;
        passed = passed && ok;
        const QJsonObject report{{"provider", sample["provider"]}, {"kind", kind}, {"sample", sample["title"]}, {"candidates", rows.size()},
            {"selectedTitle", chosen["title"].toString()}, {"selectedArtist", chosen["artist"].toString()}, {"selectedAlbum", chosen["album"].toString()},
            {"selectedDuration", chosen["duration"].toDouble()}, {"sourceUrl", chosen["sourceUrl"].toString()},
            {"previewValidated", preview}, {"durablySaved", saved}, {"passed", ok}, {"status", service.state()["status"].toString()},
            {"seconds", elapsed.elapsed() / 1000.0}, {"message", service.state()["message"].toString()}};
        reports.append(report);
        const auto line = QJsonDocument(report).toJson(QJsonDocument::Compact);
        std::fwrite(line.constData(), 1, size_t(line.size()), stdout); std::fputs("\n", stdout); std::fflush(stdout);
    }
    const auto bytes = QJsonDocument(QJsonObject{{"passed", passed}, {"scope", "production HTTPS, fixed public samples, validated preview and durable storage; no audio uploaded"}, {"results", reports}}).toJson();
    std::fwrite(bytes.constData(), 1, size_t(bytes.size()), stdout);
    return passed ? 0 : 1;
}
