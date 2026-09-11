#include "online_service.h"
#include "online/policy.h"
#include "online/providers.h"
#include "online/storage.h"
#include <QtTest/QtTest>
#include <QtGui/QImage>
#include <QtCore/QBuffer>
#include <QtCore/QTemporaryDir>
#include <QtNetwork/QTcpServer>
#include <QtNetwork/QTcpSocket>
#include <functional>
#include <memory>

using namespace liusheng;
using namespace liusheng::online;

static QJsonObject context(QString name = "Song") {
    return {{"trackKey", hash(name.toUtf8())}, {"albumKey", hash("album")}, {"identity", hash((name + "-identity").toUtf8())},
        {"path", "/music/" + name + ".wav"}, {"title", name}, {"artist", "Artist"}, {"album", "Album"}, {"albumArtist", "Artist"},
        {"duration", 120.0}, {"scope", "track"}, {"localCover", false}, {"localLyrics", false}};
}
static QJsonObject lyric(QString name = "Song", int id = 1) {
    return {{"id", id}, {"trackName", name}, {"artistName", "Artist"}, {"albumName", "Album"}, {"duration", 120.0},
        {"syncedLyrics", "[00:01.00]Original fixture line\n[00:02.00]Second fixture line"}, {"plainLyrics", "Original fixture line\nSecond fixture line"}, {"instrumental", false}};
}
static QString json(const QJsonObject &object) {return QString::fromUtf8(QJsonDocument(object).toJson(QJsonDocument::Compact));}
static QString contexts(const QJsonArray &array) {return QString::fromUtf8(QJsonDocument(array).toJson(QJsonDocument::Compact));}
static QByteArray picture() {
    QImage image(40, 40, QImage::Format_RGB32); image.fill(QColor("#7955a0"));
    QByteArray bytes; QBuffer buffer(&bytes); buffer.open(QIODevice::WriteOnly); image.save(&buffer, "PNG"); return bytes;
}
static QJsonObject qqSong(QString title = "Song") {
    return {{"songmid", "0024qkg32eglL5"}, {"songname", title}, {"singer", QJsonArray{QJsonObject{{"name", "Artist"}}}},
        {"albumname", "Album"}, {"albummid", "002abjXr4Gc7sw"}, {"interval", 120}};
}
static QJsonObject neteaseSong(QString title = "Song") {
    return {{"id", 2619125556LL}, {"name", title}, {"artists", QJsonArray{QJsonObject{{"name", "Artist"}}}}, {"duration", 120000},
        {"album", QJsonObject{{"id", 250300440}, {"name", "Album"}, {"picUrl", "https://p2.music.126.net/fixture/cover.jpg"}}}};
}
static QByteArray qqSearch(QJsonArray songs) {
    return QJsonDocument(QJsonObject{{"code", 0}, {"data", QJsonObject{{"song", QJsonObject{{"list", songs}}}}}}).toJson();
}
static QByteArray neteaseSearch(QJsonArray songs) {
    return QJsonDocument(QJsonObject{{"code", 200}, {"result", QJsonObject{{"songs", songs}, {"songCount", songs.size()}}}}).toJson();
}
static QByteArray qqLyric() {
    return QJsonDocument(QJsonObject{{"code", 0}, {"lyric", QString::fromLatin1(QByteArray("[00:01.00]fixture lyric\n[00:03.00]second line").toBase64())},
        {"trans", QString::fromLatin1(QStringLiteral("[00:01.00]测试附文").toUtf8().toBase64())}}).toJson();
}
struct Response {int status = 200; QByteArray bytes; QByteArray headers; bool hold = false;};
class Server : public QTcpServer {
public:
    QList<QByteArray> requests;
    std::function<Response(QUrl)> handler;
    Server() {
        connect(this, &QTcpServer::newConnection, this, [this] {
            while (hasPendingConnections()) {
                auto *socket = nextPendingConnection();
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
                connect(socket, &QIODevice::readyRead, socket, [this, socket] {
                    QByteArray bytes = socket->property("incoming").toByteArray() + socket->readAll();
                    socket->setProperty("incoming", bytes);
                    if (!bytes.contains("\r\n\r\n") || socket->property("responded").toBool()) return;
                    socket->setProperty("responded", true); requests.append(bytes);
                    const auto target = bytes.split(' ').value(1);
                    const auto response = handler ? handler(QUrl("http://localhost" + QString::fromUtf8(target))) : Response{404, {}, {}, false};
                    if (response.hold) return;
                    socket->write("HTTP/1.1 " + QByteArray::number(response.status) + " Fixture\r\nContent-Length: " + QByteArray::number(response.bytes.size()) + "\r\nConnection: close\r\n" + response.headers + "\r\n" + response.bytes);
                    socket->disconnectFromHost();
                });
            }
        });
        if (!listen(QHostAddress::LocalHost)) qFatal("fixture server could not start");
    }
    QUrl url() const {return QUrl("http://127.0.0.1:" + QString::number(serverPort()));}
};
class Tests : public QObject {
    Q_OBJECT
private slots:
    void supplementaryUrlsStayWithinMetadataAndImageProviders() {
        for (const QString provider : {"netease", "qqmusic", "deezer"}) {
            const auto plan = providerQueries(provider, "cover", context()); QVERIFY(!plan.isEmpty());
            for (const auto &url : plan) { QVERIFY(safeUrl(url, false)); QVERIFY(!url.toString().contains("/music/")); }
        }
        QVERIFY(safeUrl(QUrl("https://y.gtimg.cn/music/photo_new/T002R500x500M000002abjXr4Gc7sw.jpg"), true));
        QVERIFY(!safeUrl(QUrl("https://y.gtimg.cn/track.mp3"), true));
        QVERIFY(!safeUrl(QUrl("https://p2.music.126.net.evil.invalid/cover.jpg"), true));
        QVERIFY(!safeUrl(QUrl("https://music.163.com/api/song/enhance/player/url"), false));
        QVERIFY(trustedImageUrl("netease", QUrl("http://p2.music.126.net/fixture/cover.jpg")).scheme() == "https");
        QVERIFY(trustedImageUrl("qqmusic", QUrl("https://p2.music.126.net/fixture/cover.jpg")).isEmpty());
        QVERIFY(!safeSourcePage(QUrl("https://evil.invalid/song?id=123")));
    }
    void catalogMatchingRejectsUnrelatedItemsAndRequiresEditionConfirmation() {
        const auto doc = QJsonDocument::fromJson(qqSearch({qqSong(), qqSong("Unrelated")})).object();
        auto rows = catalogCandidates("qqmusic", "lyrics", doc, context()); QCOMPARE(rows.size(), 1);
        QVERIFY(rows.first().toObject()["confident"].toBool()); QVERIFY(rows.first().toObject()["lyricsPending"].toBool());
        auto c = context(); c["title"] = "Song (Live)";
        rows = catalogCandidates("qqmusic", "lyrics", doc, c); QCOMPARE(rows.size(), 1);
        QVERIFY(!rows.first().toObject()["confident"].toBool()); QVERIFY(rows.first().toObject()["manualOnly"].toBool());
        QVERIFY_THROWS_EXCEPTION(std::runtime_error, catalogCandidates("qqmusic", "lyrics", QJsonObject{{"code", 403}}, c));
        auto bad = qqSong(); bad["songmid"] = "../outside";
        QVERIFY(catalogCandidates("qqmusic", "lyrics", QJsonDocument::fromJson(qqSearch({bad})).object(), c).isEmpty());
    }
    void providerLyricsValidateBase64TranslationAndInstrumentalFlags() {
        auto candidate = catalogCandidates("qqmusic", "lyrics", QJsonDocument::fromJson(qqSearch({qqSong()})).object(), context()).first().toObject();
        const auto result = readProviderLyrics(candidate, QJsonDocument::fromJson(qqLyric()).object());
        QVERIFY(result["synced"].toBool()); QVERIFY(!result["lyricsPending"].toBool());
        QVERIFY(result["text"].toString().contains(QStringLiteral("测试附文")));
        QVERIFY_THROWS_EXCEPTION(std::runtime_error, readProviderLyrics(candidate, QJsonObject{{"code", 0}, {"lyric", "invalid@"}}));
        candidate["providerId"] = "netease";
        const auto silent = readProviderLyrics(candidate, QJsonObject{{"code", 200}, {"nolyric", true}});
        QVERIFY(silent["instrumental"].toBool()); QVERIFY(silent["text"].toString().isEmpty());
        QVERIFY_THROWS_EXCEPTION(std::runtime_error, readProviderLyrics(candidate, QJsonObject{{"code", 200}, {"uncollected", true}}));
    }
    void multiSourceFailureLeavesHealthyResultsAndDoesNotCachePartialAbsence() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &url) {
            if (url.path() == "/api/search") return Response{503, {}, "Retry-After: 60\r\n", false};
            if (url.path().startsWith("/soso/")) return Response{200, qqSearch({qqSong()}), {}, false};
            return Response{200, neteaseSearch({neteaseSong()}), {}, false};
        };
        OnlineService service; service.configureTest(server.url(), 400); service.setStorageRoot(directory.path());
        service.open(json(context()), "lyrics"); service.searchWithSource("Song", "Artist", "Album", "all");
        QTRY_COMPARE_WITH_TIMEOUT(service.state()["status"].toString(), QString("candidates"), 3000);
        QCOMPARE(service.state()["candidates"].toList().size(), 2); QCOMPARE(server.requests.size(), 3);
        QCOMPARE(service.state()["sourceReports"].toList().size(), 3); QVERIFY(service.state()["partialResults"].toBool());
        for (const auto &row : service.state()["candidates"].toList()) QVERIFY(!row.toMap()["confident"].toBool());
        auto query = context(); query["_source"] = "all"; Store store(directory.path());
        QVERIFY(store.cached(lookupCacheKey(query, "lyrics")).isEmpty());
    }
    void manualQQLyricsLoadOnPreviewAndPersistOffline() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &url) {return Response{200, url.path().startsWith("/soso/") ? qqSearch({qqSong()}) : qqLyric(), {}, false};};
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        QSignalSpy saved(&service, &OnlineService::applied);
        service.open(json(context()), "lyrics"); service.searchWithSource("Song", "Artist", "Album", "qqmusic");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates")); QCOMPARE(server.requests.size(), 1);
        QVERIFY(service.state()["candidates"].toList().first().toMap()["lyricsPending"].toBool());
        service.preview(0); QTRY_COMPARE(service.state()["status"].toString(), QString("preview"));
        QCOMPARE(server.requests.size(), 2); QVERIFY(!service.state()["candidates"].toList().first().toMap()["lyricsPending"].toBool());
        QVERIFY(service.state()["previewText"].toString().contains("fixture lyric"));
        service.choose(false); QTRY_COMPARE(saved.size(), 1);
        Store store(directory.path()); auto binding = store.binding(context(), "lyrics");
        QCOMPARE(binding["provider"].toString(), QStringLiteral("QQ 音乐")); QVERIFY(binding["pinned"].toBool());
        QFile file(directory.path() + "/objects/" + binding["blob"].toString()); QVERIFY(file.open(QIODevice::ReadOnly));
        QVERIFY(file.readAll().contains("fixture lyric"));
        for (const auto &request : server.requests) { QVERIFY(!request.contains("Cookie:")); QVERIFY(!request.contains("X-Real-IP:")); }
    }
    void neteaseCoverDetailsMustMatchSongAndAlbumBeforeDownloading() {
        auto song = neteaseSong(); auto album = song["album"].toObject(); album.remove("picUrl"); song["album"] = album;
        QTemporaryDir directory; Server server;
        server.handler = [song](const QUrl &url) {
            if (url.path() == "/api/search/get") return Response{200, neteaseSearch({song}), {}, false};
            if (url.path() == "/api/song/detail/") return Response{200, QJsonDocument(QJsonObject{{"code", 200}, {"songs", QJsonArray{neteaseSong()}}}).toJson(), {}, false};
            return Response{200, picture(), {}, false};
        };
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.open(json(context()), "cover"); service.searchWithSource("Song", "Artist", "Album", "netease");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates")); service.preview(0);
        QTRY_COMPARE(service.state()["status"].toString(), QString("preview")); QCOMPARE(server.requests.size(), 3);
        QVERIFY(!QImage(QUrl(service.state()["previewUrl"].toString()).toLocalFile()).isNull());
        auto c = service.state()["candidates"].toList().first().toMap();
        auto wrong = neteaseSong(); wrong["id"] = 7;
        QVERIFY(neteaseCoverUrl(QJsonObject::fromVariantMap(c), QJsonObject{{"code", 200}, {"songs", QJsonArray{wrong}}}).isEmpty());
    }
    void allSourceFailuresRemainRetryableAndNeverBecomeMissing() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{503, {}, "Retry-After: 60\r\n", false};};
        OnlineService service; service.configureTest(server.url(), 300); service.setStorageRoot(directory.path());
        service.startBatchWithSource(contexts({context()}), "lyrics", "all");
        QTRY_COMPARE_WITH_TIMEOUT(service.state()["batchDone"].toInt(), 1, 3000);
        QCOMPARE(server.requests.size(), 3); QCOMPARE(service.state()["batchRetryable"].toInt(), 1);
        QCOMPARE(service.state()["batchResults"].toList().first().toMap()["status"].toString(), QString("error"));
    }
    void multiSourceCancellationStopsFurtherSourcesAndPreservesSelection() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) { return Response{200, {}, {}, true}; };
        OnlineService service; service.configureTest(server.url(), 250); service.setStorageRoot(directory.path());
        service.open(json(context()), "lyrics"); service.searchWithSource("Song", "Artist", "Album", "all");
        QTRY_COMPARE(server.requests.size(), 1); service.cancel(); QTRY_VERIFY(!service.state()["busy"].toBool());
        QTest::qWait(500); QCOMPARE(server.requests.size(), 1);
        QVERIFY(service.state()["candidates"].toList().isEmpty());
    }
    void extraAutomaticSourcesRequireOptInAndDisablingCancelsThem() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &url) {
            if (url.path() == "/api/search") return Response{200, "[]", {}, false};
            return Response{200, {}, {}, true};
        };
        OnlineService service; service.configureTest(server.url(), 500); service.setStorageRoot(directory.path()); service.setAutoLyrics(true);
        QVERIFY(!service.autoExtraSources()); service.requestAutomatic(contexts({context()}));
        QTRY_COMPARE(service.state()["automaticStatus"].toString(), QString("missing"));
        for (const auto &request : server.requests) QVERIFY(request.split(' ')[1].startsWith("/api/search?"));
        const auto old = server.requests.size(); service.setAutoExtraSources(true); service.requestAutomatic(contexts({context()}));
        QTRY_VERIFY(server.requests.size() > old); service.setAutoExtraSources(false); const auto count = server.requests.size();
        QTest::qWait(1000); QCOMPARE(server.requests.size(), count);
        Store store(directory.path()); QVERIFY(store.binding(context(), "lyrics").isEmpty());
    }
    void selectedBatchSourceSurvivesCancellationAndRestart() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{200, {}, {}, true};};
        {
            OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
            service.startBatchWithSource(contexts({context()}), "lyrics", "qqmusic"); QTRY_COMPARE(server.requests.size(), 1);
            service.cancelBatch(); QTRY_COMPARE(service.state()["batchCancelled"].toInt(), 1);
        }
        server.handler = [](const QUrl &url) {return Response{200, url.path().startsWith("/soso/") ? qqSearch({qqSong()}) : qqLyric(), {}, false};};
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path()); service.inspectBatch();
        QTRY_VERIFY(service.state()["batchRetryable"].toInt() == 1); service.retryBatchFailures();
        QTRY_COMPARE_WITH_TIMEOUT(service.state()["batchDone"].toInt(), 1, 3500);
        for (const auto &request : server.requests) QVERIFY(!request.split(' ')[1].startsWith("/api/search?"));
        Store store(directory.path()); QCOMPARE(store.binding(context(), "lyrics")["provider"].toString(), QStringLiteral("QQ 音乐"));
    }
    void deezerCoverPreviewUsesValidatedCdnAndPersistsProviderIdentity() {
        QTemporaryDir directory; Server server;
        const QJsonObject album{{"id", 629767311}, {"title", "Album"}, {"artist", QJsonObject{{"name", "Artist"}}},
            {"cover_big", "https://cdn-images.dzcdn.net/images/cover/abc/500x500-000000-80-0-0.jpg"}};
        server.handler = [album](const QUrl &url) {
            if (url.path() == "/search/album") return Response{200, QJsonDocument(QJsonObject{{"data", QJsonArray{album}}}).toJson(), {}, false};
            return Response{200, picture(), {}, false};
        };
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        QSignalSpy saved(&service, &OnlineService::applied);
        service.open(json(context()), "cover"); service.searchWithSource("Song", "Artist", "Album", "deezer");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates"));
        QVERIFY(!service.state()["candidates"].toList().first().toMap()["confident"].toBool());
        service.preview(0); QTRY_COMPARE(service.state()["status"].toString(), QString("preview"));
        QVERIFY(!QImage(QUrl(service.state()["previewUrl"].toString()).toLocalFile()).isNull());
        service.choose(false); QTRY_COMPARE(saved.size(), 1);
        Store store(directory.path()); const auto binding = store.binding(context(), "cover");
        QCOMPARE(binding["sourceId"].toString(), QString("629767311"));
        QCOMPARE(binding["provider"].toString(), QString("Deezer"));
    }
    void qqCoverRequestsPortableImageFormatsAndValidatesDownloadedPixels() {
        QTemporaryDir directory; Server server;
        server.handler = [&server](const QUrl &url) {
            if (url.path().startsWith("/soso/")) return Response{200, qqSearch({qqSong()}), {}, false};
            // A CDN can negotiate WebP despite a .jpg extension. Advertising
            // only portable formats keeps the package independent of plugins.
            if (server.requests.last().contains("image/webp")) return Response{200, "RIFF-invalid-WEBP", {}, false};
            return Response{200, picture(), "Content-Type: image/png\r\n", false};
        };
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.open(json(context()), "cover"); service.searchWithSource("Song", "Artist", "Album", "qqmusic");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates")); service.preview(0);
        QTRY_COMPARE(service.state()["status"].toString(), QString("preview"));
        QCOMPARE(server.requests.size(), 2);
        QVERIFY(server.requests.last().contains("image/jpeg, image/png"));
        QVERIFY(!QImage(QUrl(service.state()["previewUrl"].toString()).toLocalFile()).isNull());
    }
    void sourceSelectionIsPartOfSearchCacheIdentity() {
        auto a = context(), b = context(); a["_source"] = "all"; b["_source"] = "qqmusic";
        QVERIFY(lookupCacheKey(a, "lyrics") != lookupCacheKey(b, "lyrics"));
        b["_source"] = "netease"; QVERIFY(lookupCacheKey(a, "lyrics") != lookupCacheKey(b, "lyrics"));
        const auto same = mergeCandidates(catalogCandidates("qqmusic", "lyrics", QJsonDocument::fromJson(qqSearch({qqSong()})).object(), context()), catalogCandidates("qqmusic", "lyrics", QJsonDocument::fromJson(qqSearch({qqSong()})).object(), context()));
        QCOMPARE(same.size(), 1);
    }
// Regressions for the reported 503 -> stalled batch / duplicate history path.
// Included inside Tests' private slots so all cases use the production service.
void transient503RetriesSameSearchThenSavesExactlyOnce() {
    QTemporaryDir directory; Server server; int calls = 0;
    server.handler = [&calls](const QUrl &) {
        if (++calls < 3) return Response{503, {}, "Retry-After: 1\r\n", false};
        return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false};
    };
    OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
    QSignalSpy saved(&service, &OnlineService::resourceChanged);
    service.startBatch(contexts({context()}), "lyrics");
    QTRY_COMPARE_WITH_TIMEOUT(service.state()["batchDone"].toInt(), 1, 5500);
    QCOMPARE(service.state()["batchRemaining"].toInt(), 0);
    QCOMPARE(saved.size(), 1); QCOMPARE(calls, 3);
    QCOMPARE(service.state()["batchResults"].toList().size(), 1);
    QCOMPARE(service.state()["batchCounts"].toMap()["saved"].toInt(), 1);
    for (const auto &request : server.requests) QCOMPARE(request.split(' ')[1], server.requests.first().split(' ')[1]);
}
void cancelledBatchAccountsEveryItem() {
    QTemporaryDir directory; Server server;
    server.handler = [](const QUrl &) {return Response{503, {}, "Retry-After: 60\r\n", false};};
    OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
    service.startBatch(contexts({context()}), "lyrics"); QTRY_COMPARE(server.requests.size(), 1);
    service.cancelBatch(); QTRY_COMPARE(service.state()["batchRemaining"].toInt(), 0);
    QCOMPARE(service.state()["batchTotal"].toInt(), service.state()["batchDone"].toInt() + service.state()["batchRemaining"].toInt() + service.state()["batchCancelled"].toInt());
    QCOMPARE(service.state()["batchCancelled"].toInt(), 1);
    QCOMPARE(service.state()["batchRetryable"].toInt(), 1);
}

    void retryAfterSupportsSecondsDatesAndBoundedBackoff() {
        const auto now = QDateTime::currentMSecsSinceEpoch();
        QCOMPARE(retryDelayMs("3", {}, 503, 0, now), qint64(3000));
        QCOMPARE(retryDelayMs("0", {}, 503, 0, now), qint64(1000));
        QCOMPARE(retryDelayMs("999999999", {}, 503, 0, now), qint64(86400000));
        QCOMPARE(retryDelayMs("invalid", {}, 503, 0, now), qint64(2000));
        QCOMPARE(retryDelayMs({}, {}, 503, 1, now), qint64(4000));
        QCOMPARE(retryDelayMs({}, {}, 429, 0, now), qint64(60000));
        QCOMPARE(retryDelayMs("Wed, 21 Oct 2015 07:28:10 GMT", "Wed, 21 Oct 2015 07:28:00 GMT", 503, 0, now), qint64(10000));
    }
    void exhaustedRetriesKeepOnePendingRowAndNeverCacheAbsence() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{503, {}, "Retry-After: 1\r\n", false};};
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.startBatch(contexts({context()}), "lyrics");
        QTRY_VERIFY_WITH_TIMEOUT(server.requests.size() == 3 && service.state()["batchPaused"].toBool(), 5500);
        QCOMPARE(service.state()["batchDone"].toInt(), 0);
        QCOMPARE(service.state()["batchRemaining"].toInt(), 1);
        QCOMPARE(service.state()["batchResults"].toList().size(), 1);
        QVERIFY(service.state()["batchMessage"].toString().contains("503"));
        QTest::qWait(1300); QCOMPARE(server.requests.size(), 3);
        Store store(directory.path()); QVERIFY(store.cached(lookupCacheKey(context(), "lyrics")).isEmpty());
        server.handler = [](const QUrl &) {return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false};};
        service.resumeBatch(); QTRY_COMPARE(service.state()["batchDone"].toInt(), 1);
        QCOMPARE(service.state()["batchResults"].toList().size(), 1);
    }
    void pauseAndCancelInvalidateScheduledRetryAndExplicitRetryRecovers() {
        QTemporaryDir directory; Server server; int count = 0;
        server.handler = [&count](const QUrl &) {
            if (++count == 1) return Response{503, {}, "Retry-After: 1\r\n", false};
            return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false};
        };
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.startBatch(contexts({context()}), "lyrics");
        QTRY_COMPARE(service.state()["batchPhase"].toString(), QString("waiting"));
        service.pauseBatch(); QTRY_VERIFY(service.state()["batchPaused"].toBool());
        QTest::qWait(1200); QCOMPARE(count, 1);
        service.cancelBatch(); QTRY_COMPARE(service.state()["batchCancelled"].toInt(), 1);
        service.retryBatchFailures(); QTRY_COMPARE(service.state()["batchDone"].toInt(), 1);
        QCOMPARE(service.state()["batchCancelled"].toInt(), 0);
        QCOMPARE(service.state()["batchRetryable"].toInt(), 0);
        QCOMPARE(service.state()["batchResults"].toList().size(), 1);
        QCOMPARE(count, 2);
    }
    void providerCooldownIsIsolatedAndCacheClearPreservesIt() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &url) {
            if (url.path().startsWith("/ws/")) return Response{503, {}, "Retry-After: 60\r\n", false};
            return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false};
        };
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.open(json(context()), "cover"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("waiting"));
        service.cancel(); service.clearSearchCache();
        service.open(json(context()), "lyrics"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates"));
        QCOMPARE(server.requests.size(), 2);
        service.open(json(context()), "cover"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("waiting"));
        QCOMPARE(server.requests.size(), 2);
    }
    void restartingWaitingBatchPreservesPendingAndStartsPaused() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{503, {}, "Retry-After: 60\r\n", false};};
        {
            OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
            service.startBatch(contexts({context()}), "lyrics");
            QTRY_COMPARE(service.state()["batchPhase"].toString(), QString("waiting"));
        }
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path()); service.inspectBatch();
        QTRY_COMPARE(service.state()["batchRemaining"].toInt(), 1);
        QVERIFY(service.state()["batchPaused"].toBool()); QCOMPARE(server.requests.size(), 1);
        service.resumeBatch(); QTRY_COMPARE(service.state()["batchPhase"].toString(), QString("waiting"));
        QCOMPARE(service.state()["batchResults"].toList().size(), 1); QCOMPARE(server.requests.size(), 1);
    }
    void legacyCanceledBatchHasConsistentCountsAndDeduplicatedRecovery() {
        QTemporaryDir directory; Store store(directory.path()); store.initialize();
        const QJsonObject row{{"context", context()}, {"kind", "lyrics"}, {"status", "retry"}, {"title", "Song"}, {"artist", "Artist"}};
        store.writeJson("batch.json", {{"version", 1}, {"kind", "lyrics"}, {"total", 1}, {"done", 0}, {"pending", QJsonArray()}, {"results", QJsonArray{row, row}}});
        OnlineService service; service.setStorageRoot(directory.path()); service.inspectBatch();
        QTRY_COMPARE(service.state()["batchTotal"].toInt(), 1);
        QCOMPARE(service.state()["batchCancelled"].toInt(), 1); QCOMPARE(service.state()["batchDone"].toInt(), 0);
        QCOMPARE(service.state()["batchRemaining"].toInt(), 0); QCOMPARE(service.state()["batchResults"].toList().size(), 1);
        QCOMPARE(service.state()["batchRetryable"].toInt(), 1);
    }
    void sourceAliasOnMatchingReleaseTrackKeepsTraditionalRecordingCandidate() {
        QTemporaryDir directory; Server server;
        const QString id = "12345678-1234-1234-1234-123456789abc";
        const QJsonObject release{{"id", id}, {"title", "JJ20"}, {"status", "Official"},
            {"media", QJsonArray{QJsonObject{{"track", QJsonArray{QJsonObject{{"title", QStringLiteral("起风了")}}}}}}}};
        const QJsonObject recording{{"title", QStringLiteral("起風了")}, {"artist-credit", QJsonArray{QJsonObject{{"name", QStringLiteral("林俊杰")}}}}, {"releases", QJsonArray{release}}};
        server.handler = [recording](const QUrl &url) {
            if (url.path().contains("recording")) return Response{200, QJsonDocument(QJsonObject{{"recordings", QJsonArray{recording}}}).toJson(), {}, false};
            return Response{200, "{\"releases\":[]}", {}, false};
        };
        auto c = context(); c["title"] = QStringLiteral("起风了 (Live)"); c["artist"] = QStringLiteral("林俊杰(百视听音乐mp3bst.com)");
        OnlineService service; service.configureTest(server.url(), 400); service.setStorageRoot(directory.path());
        service.open(json(c), "cover"); service.search(c["title"].toString(), c["artist"].toString(), "Wrong album");
        QTRY_COMPARE_WITH_TIMEOUT(service.state()["status"].toString(), QString("candidates"), 5500);
        const auto candidate = service.state()["candidates"].toList().first().toMap();
        QCOMPARE(candidate["title"].toString(), QString("JJ20")); QVERIFY(candidate["manualOnly"].toBool());
        QCOMPARE(candidate["recordingTitle"].toString(), QStringLiteral("起风了"));
        QVERIFY(!candidate["confident"].toBool());
    }
    void copiedLiveAlbumTagPrefersRecordingAndKeepsConfirmationRequired() {
        auto c = context(); c["title"] = "Song (Live)"; c["album"] = "Song (Live)";
        const auto plan = lookupPlan(c, "cover");
        QVERIFY(plan.first().path().endsWith("/recording/"));
        QVERIFY(QUrlQuery(plan.first()).queryItemValue("query").contains("recording:\"Song\""));
        QCOMPARE(c["title"].toString(), QString("Song (Live)"));
        QTemporaryDir directory; Server server;
        const QJsonObject release{{"id", "12345678-1234-1234-1234-123456789abc"}, {"title", "Concert"}, {"status", "Official"}, {"artist-credit", QJsonArray{QJsonObject{{"name", "Artist"}}}}};
        const QJsonObject recording{{"title", "Song"}, {"releases", QJsonArray{release}}};
        server.handler = [recording](const QUrl &) {return Response{200, QJsonDocument(QJsonObject{{"recordings", QJsonArray{recording}}}).toJson(), {}, false};};
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.startBatch(contexts({c}), "cover");
        QTRY_COMPARE(service.state()["batchDone"].toInt(), 1);
        QCOMPARE(service.state()["batchCounts"].toMap()["review"].toInt(), 1);
        QCOMPARE(server.requests.size(), 1);
        Store store(directory.path()); QVERIFY(store.binding(c, "cover").isEmpty());
    }
    void replacementBatchCannotReceiveOldRetryCompletion() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &url) {
            const auto name = QUrlQuery(url).queryItemValue("track_name");
            if (name == "First") return Response{503, {}, "Retry-After: 1\r\n", false};
            return Response{200, QJsonDocument(QJsonArray{lyric(name)}).toJson(), {}, false};
        };
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.startBatch(contexts({context("First")}), "lyrics"); QTRY_COMPARE(service.state()["batchPhase"].toString(), QString("waiting"));
        service.startBatch(contexts({context("Second")}), "lyrics");
        QTRY_COMPARE_WITH_TIMEOUT(service.state()["batchDone"].toInt(), 1, 3500);
        QCOMPARE(service.state()["batchResults"].toList().size(), 1);
        QCOMPARE(service.state()["batchResults"].toList().first().toMap()["title"].toString(), QString("Second"));
        QCOMPARE(server.requests.size(), 2);
        Store store(directory.path()); QVERIFY(store.binding(context("First"), "lyrics").isEmpty());
    }
    void wordLyricImportRequiresMatchingValidationAndSurvivesRestart() {
        QTemporaryDir directory; OnlineService service; service.setStorageRoot(directory.path());
        QSignalSpy requests(&service, &OnlineService::lyricImportRequested);
        QSignalSpy applied(&service, &OnlineService::applied);
        service.open(json(context()), "lyrics"); QTRY_VERIFY(service.state()["context"].toMap().contains("trackKey"));
        service.importLocal(QUrl::fromLocalFile("/fixture/song.ttml"), false);
        QCOMPARE(requests.size(), 1); QVERIFY(service.state()["busy"].toBool());
        const auto id = requests.first()[0].toString();
        service.completeLyricImport("wrong-request", "<tt/>", false, "");
        QVERIFY(service.state()["busy"].toBool()); QCOMPARE(applied.size(), 0);
        service.completeLyricImport(id, "", false, "invalid word timeline");
        QCOMPARE(service.state()["status"].toString(), QString("error"));
        service.importLocal(QUrl::fromLocalFile("/fixture/song.ttml"), false);
        const QString source = "<tt><body><p begin=\"1s\" end=\"4s\"><span begin=\"0s\" dur=\"2s\">word</span></p></body></tt>";
        service.completeLyricImport(requests.last()[0].toString(), source, true, "");
        QTRY_COMPARE(applied.size(), 1);
        Store store(directory.path()); const auto value = store.binding(context(), "lyrics");
        QVERIFY(value["pinned"].toBool()); QFile file(directory.path() + "/objects/" + value["blob"].toString());
        QVERIFY(file.open(QIODevice::ReadOnly)); QCOMPARE(QString::fromUtf8(file.readAll()), source);
        service.importLocal(QUrl::fromLocalFile("/fixture/stale.qrc"), false);
        const auto stale = requests.last()[0].toString(); service.close();
        service.open(json(context("another")), "lyrics");
        QTRY_COMPARE(service.state()["context"].toMap()["title"].toString(), QString("another"));
        service.completeLyricImport(stale, source, true, ""); QTest::qWait(50);
        QVERIFY(store.binding(context("another"), "lyrics").isEmpty());
        QCOMPARE(applied.size(), 1);
    }
    void canceledWordImportCannotRestoreOldSelectionOrLeaveBusyState() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) { return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false}; };
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        QSignalSpy requests(&service, &OnlineService::lyricImportRequested);
        QSignalSpy applied(&service, &OnlineService::applied);
        service.open(json(context()), "lyrics");
        QTRY_VERIFY(service.state()["context"].toMap().contains("trackKey"));
        const QString text = "[1000,2000](1000,2000,0)original fixture";
        service.importLocal(QUrl::fromLocalFile("/fixture/local.yrc"), false);
        const auto closed = requests.last()[0].toString();
        service.close(); QVERIFY(!service.state()["busy"].toBool());
        service.completeLyricImport(closed, text, true, "");
        QTest::qWait(40); QCOMPARE(applied.size(), 0);
        service.open(json(context()), "lyrics");
        QTRY_COMPARE(service.state()["status"].toString(), QString("idle"));
        service.importLocal(QUrl::fromLocalFile("/fixture/local.qrc"), false);
        const auto reset = requests.last()[0].toString();
        service.restoreDefault(false);
        QTRY_VERIFY(!service.state()["busy"].toBool());
        service.completeLyricImport(reset, text, true, "");
        QTest::qWait(40); QCOMPARE(applied.size(), 0);
        Store store(directory.path()); QVERIFY(store.binding(context(), "lyrics")["disabled"].toBool());
        service.importLocal(QUrl::fromLocalFile("/fixture/local.ttml"), false);
        const auto searched = requests.last()[0].toString();
        service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates"));
        service.completeLyricImport(searched, text, true, "");
        QTest::qWait(40); QCOMPARE(applied.size(), 0);
        const auto count = requests.size();
        service.importLocal(QUrl("https://example.invalid/private.ttml"), false);
        QCOMPARE(requests.size(), count);
        QCOMPARE(service.state()["status"].toString(), QString("error"));
        QVERIFY(!service.state()["busy"].toBool());
    }
    void downloadTagsAreCleanedWhileVersionsAndOriginalIdentitySurvive() {
        QCOMPARE(cleanSearchText(QStringLiteral("林俊杰(百视听音乐mp3bst.com)")), QStringLiteral("林俊杰"));
        QCOMPARE(cleanSearchText(QStringLiteral(" 起风了 （Live） [320kbps] ")), QStringLiteral("起风了 (Live)"));
        QCOMPARE(baseTitle(QStringLiteral("伟大的渺小 (Jazz Version)")), QStringLiteral("伟大的渺小"));
        QVERIFY(versions(QStringLiteral("伟大的渺小 (Jazz Version)")).contains("jazz"));
        QCOMPARE(cleanSearchText("will.i.am"), QString("will.i.am"));
        QCOMPARE(cleanSearchText("Song (feat. Artist)"), QString("Song (feat. Artist)"));
        auto c = context(); c["artist"] = QStringLiteral("林俊杰(百视听音乐mp3bst.com)"); c["title"] = QStringLiteral("起风了 (Live)");
        const auto cleaned = cleanQuery(c);
        QCOMPARE(cleaned["identity"], c["identity"]);
        QCOMPARE(cleaned["path"], c["path"]);
        QVERIFY(c["artist"].toString().contains("mp3bst"));
        const auto plan = lookupPlan(c, "lyrics");
        QCOMPARE(QUrlQuery(plan.first()).queryItemValue("artist_name"), QStringLiteral("林俊杰"));
        QVERIFY(plan.size() <= 4);
        QVERIFY(relatedTitle(QStringLiteral("伟大的渺小"), QStringLiteral("伟大的渺小 (Jazz Version)")));
        QVERIFY(!relatedTitle("Homecoming", "Home"));
        auto studio = lyric(); auto jazz = context(); jazz["title"] = "Song (Jazz Version)";
        QCOMPARE(automaticChoice(lyricsCandidates({studio}, jazz)), -1);
    }
    void manualApplyAcknowledgesDurableWriteAndIgnoresDoubleClick() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) { return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false}; };
        OnlineService service; service.configureTest(server.url(), 5000); service.setStorageRoot(directory.path());
        QSignalSpy applied(&service, &OnlineService::applied);
        QSignalSpy resources(&service, &OnlineService::resourceChanged);
        service.open(json(context()), "lyrics"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates"));
        service.preview(0); QTRY_COMPARE(service.state()["selected"].toInt(), 0);
        service.choose(false); QVERIFY(service.state()["busy"].toBool());
        QCOMPARE(service.state()["status"].toString(), QString("saving"));
        service.choose(false);
        QTRY_COMPARE(applied.size(), 1);
        QCOMPARE(resources.size(), 1);
        Store store(directory.path()); QVERIFY(!store.binding(context(), "lyrics").isEmpty());
        QCOMPARE(applied.first()[0].toString(), context()["identity"].toString());
        QCOMPARE(applied.first()[1].toString(), context()["trackKey"].toString());
        QCOMPARE(applied.first()[2].toString(), QString("lyrics"));
        QVERIFY(!service.state()["busy"].toBool());
        service.preview(0); QTRY_COMPARE(service.state()["status"].toString(), QString("preview"));
        const auto binding = store.binding(context(), "lyrics");
        const auto file = directory.path() + "/bindings/" + context()["trackKey"].toString() + ".lyrics.json";
        QVERIFY(QFile::remove(file)); QVERIFY(QDir().mkdir(file));
        service.choose(false);
        QTRY_COMPARE(service.state()["status"].toString(), QString("error"));
        QCOMPARE(applied.size(), 1);
        QVERIFY(!service.state()["busy"].toBool());
    }
    void exactMissUsesBaseTitleAndLeavesJazzCandidateForManualReview() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &url) {
            const auto title = QUrlQuery(url).queryItemValue("track_name");
            if (title == "Song") return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false};
            return Response{200, "[]", {}, false};
        };
        auto c = context(); c["title"] = "Song (Jazz Version)"; c["artist"] = "Artist(site.example.com)";
        OnlineService service; service.configureTest(server.url(), 5000); service.setStorageRoot(directory.path()); service.setAutoLyrics(true);
        QSignalSpy applied(&service, &OnlineService::resourceChanged);
        service.requestAutomatic(contexts({c}));
        QTRY_COMPARE(service.state()["automaticStatus"].toString(), QString("review"));
        QCOMPARE(server.requests.size(), 2); QCOMPARE(applied.size(), 0);
        service.open(json(c), "lyrics"); service.search(c["title"].toString(), c["artist"].toString(), "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates"));
        auto candidate = service.state()["candidates"].toList().first().toMap();
        QVERIFY(candidate["manualOnly"].toBool()); QVERIFY(!candidate["confident"].toBool());
        QVERIFY(candidate["note"].toString().contains("Jazz"));
    }
    void lookupNegativeCacheIsSharedByQueryAndInvalidatesLegacyStrategy() {
        auto a = context(); auto b = context("Other-path");
        b["title"] = a["title"];
        QCOMPARE(lookupCacheKey(a, "lyrics"), lookupCacheKey(b, "lyrics"));
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) { return Response{200, "[]", {}, false}; };
        OnlineService service; service.configureTest(server.url(), 5000); service.setStorageRoot(directory.path()); service.setAutoLyrics(true);
        service.requestAutomatic(contexts({a}));
        QTRY_COMPARE(service.state()["automaticStatus"].toString(), QString("missing"));
        const auto total = server.requests.size(); QVERIFY(total >= 2);
        service.requestAutomatic(contexts({b})); QTest::qWait(400);
        QCOMPARE(server.requests.size(), total);
        service.open(json(b), "lyrics"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("missing"));
        QVERIFY(server.requests.size() > total);
    }
    void coverRecordingFallbackWorksWithAnIncorrectAlbumTag() {
        QTemporaryDir directory; Server server;
        QJsonObject release{{"id", "12345678-1234-1234-1234-123456789abc"}, {"title", "Real Album"}, {"status", "Official"}};
        QJsonObject recording{{"title", "Song"}, {"artist-credit", QJsonArray{QJsonObject{{"name", "Artist"}}}}, {"releases", QJsonArray{release}}};
        server.handler = [recording](const QUrl &url) {
            if (url.path().contains("/recording/")) return Response{200, QJsonDocument(QJsonObject{{"recordings", QJsonArray{recording}}}).toJson(), {}, false};
            return Response{200, "{\"releases\":[]}", {}, false};
        };
        OnlineService service; service.configureTest(server.url(), 6000); service.setStorageRoot(directory.path());
        service.open(json(context()), "cover"); service.search("Song", "Artist", "Wrong Album");
        QTRY_COMPARE_WITH_TIMEOUT(service.state()["status"].toString(), QString("candidates"), 10000);
        const auto candidate = service.state()["candidates"].toList().first().toMap();
        QCOMPARE(candidate["title"].toString(), QString("Real Album"));
        QCOMPARE(candidate["recordingTitle"].toString(), QString("Song"));
        QVERIFY(candidate["manualOnly"].toBool());
    }
    void releaseGroupCandidatesKeepTypeAndArtistAlias() {
        QJsonObject artist{{"name", "Alias Artist"}, {"aliases", QJsonArray{QJsonObject{{"name", "Artist"}}}}};
        QJsonObject item{{"id", "12345678-1234-1234-1234-123456789abc"}, {"title", "Album"}, {"artist-credit", QJsonArray{QJsonObject{{"artist", artist}}}}};
        const auto groups = coverCandidates({{"release-groups", QJsonArray{item}}}, context());
        QCOMPARE(groups.size(), 1); QVERIFY(groups.first().toObject()["groupFallback"].toBool());
        QVERIFY(groups.first().toObject()["sourceUrl"].toString().contains("/release-group/"));
        item["status"] = "Official";
        const auto releases = coverCandidates({{"releases", QJsonArray{item}}}, context());
        QVERIFY(releases.first().toObject()["confident"].toBool());
    }
    void missingLargeThumbnailFallsBackToAvailableSmallerImage() {
        QTemporaryDir directory; Server server;
        QJsonObject release{{"id", "12345678-1234-1234-1234-123456789abc"}, {"title", "Album"}, {"status", "Official"}, {"artist-credit", QJsonArray{QJsonObject{{"name", "Artist"}}}}};
        server.handler = [release](const QUrl &url) {
            if (url.path().startsWith("/ws/2/")) return Response{200, QJsonDocument(QJsonObject{{"releases", QJsonArray{release}}}).toJson(), {}, false};
            if (url.path().endsWith("front-500")) return Response{200, picture(), {}, false};
            return Response{404, {}, {}, false};
        };
        OnlineService service; service.configureTest(server.url(), 5000); service.setStorageRoot(directory.path());
        service.open(json(context()), "cover"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates"));
        service.preview(0);
        QTRY_VERIFY_WITH_TIMEOUT(!service.state()["previewUrl"].toString().isEmpty(), 6000);
        QCOMPARE(server.requests.size(), 3);
        QVERIFY(server.requests.last().contains("front-500"));
    }
    void queryNormalizationKeepsVersions() {
        QCOMPARE(normalized(QStringLiteral(" ＳＯＮＧ（Live） ")), QStringLiteral("songlive"));
        QVERIFY(versions("Song (Live)").contains("live"));
        QVERIFY(versions(QStringLiteral("现场演唱会")).contains("live"));
        QVERIFY(versions("TV Size").contains("short"));
        QVERIFY(versions("Acoustic Remix").contains("remix"));
        QVERIFY(!known(QStringLiteral("未知专辑")));
        QVERIFY(!known("Unknown Artist"));
    }
    void requestParametersContainOnlyNecessaryMetadata() {
        const auto c = context();
        auto lyrics = lyricsUrl(c); auto covers = coversUrl(c);
        QVERIFY(!lyrics.toString().contains("/music/"));
        QVERIFY(!covers.toString().contains("trackKey"));
        QCOMPARE(QUrlQuery(lyrics).queryItemValue("artist_name"), QString("Artist"));
        QVERIFY(QUrlQuery(covers).queryItemValue("query").contains("release:"));
        auto noAlbum = c; noAlbum["album"] = "";
        QCOMPARE(coversUrl(noAlbum).path(), QString("/ws/2/recording/"));
        auto quoted = c; quoted["album"] = "one\" OR release:*";
        QVERIFY(QUrlQuery(coversUrl(quoted)).queryItemValue("query").contains("\\\""));
    }
    void strictProviderUrls_data() {
        QTest::addColumn<QString>("url"); QTest::addColumn<bool>("image"); QTest::addColumn<bool>("allowed");
        QTest::newRow("lyrics") << "https://lrclib.net/api/search?q=Song" << false << true;
        QTest::newRow("brainz") << "https://musicbrainz.org/ws/2/release/" << false << true;
        QTest::newRow("cdn") << "https://ia801.archive.org/file.jpg" << true << true;
        QTest::newRow("caa") << "https://coverartarchive.org/release/x" << true << true;
        QTest::newRow("cleartext") << "http://lrclib.net/api/search" << false << false;
        QTest::newRow("credential") << "https://user:pass@lrclib.net/api/search" << false << false;
        QTest::newRow("foreign") << "https://archive.org.other.example/p.jpg" << true << false;
        QTest::newRow("port") << "https://musicbrainz.org:8080/path" << false << false;
        QTest::newRow("local") << "file:///tmp/cover.jpg" << true << false;
        QTest::newRow("fragment") << "https://lrclib.net/api#fragment" << false << false;
    }
    void strictProviderUrls() {QFETCH(QString, url); QFETCH(bool, image); QFETCH(bool, allowed); QCOMPARE(safeUrl(QUrl(url), image), allowed);}
    void lyricsMatchingRequiresSongArtistEditionDurationAndUniqueResult() {
        auto c = context(); auto good = lyric();
        QCOMPARE(automaticChoice(lyricsCandidates({good}, c)), 0);
        auto different = good; different["duration"] = 125.0;
        QCOMPARE(automaticChoice(lyricsCandidates({different}, c)), -1);
        different = good; different["artistName"] = "Cover Singer";
        QCOMPARE(automaticChoice(lyricsCandidates({different}, c)), -1);
        different = good; different["trackName"] = "Song (Live)";
        QCOMPARE(automaticChoice(lyricsCandidates({different}, c)), -1);
        different = good; different["albumName"] = "Album (Remastered)";
        QCOMPARE(automaticChoice(lyricsCandidates({different}, c)), -1);
        auto second = good; second["id"] = 2; second["syncedLyrics"] = "[00:04.00]A different timing";
        QCOMPARE(automaticChoice(lyricsCandidates({good, second}, c)), -1);
        QCOMPARE(lyricsCandidates({good, good}, c).size(), 1);
        c["artist"] = "";
        QCOMPARE(automaticChoice(lyricsCandidates({good}, c)), -1);
    }
    void ranksAllBoundedMatchesBeforeLimitingVisibleCandidates() {
        QJsonArray values;
        for (int i = 0; i < 30; ++i) {
            auto item = lyric("Song live " + QString::number(i), i);
            item["albumName"] = "Other";
            item["duration"] = 150 + i;
            values.append(item);
        }
        values.append(lyric());
        const auto candidates = lyricsCandidates(values, context());
        QCOMPARE(candidates.size(), 20);
        QCOMPARE(candidates.first().toObject()["title"].toString(), QString("Song"));
        QCOMPARE(automaticChoice(candidates), 0);
    }
    void translationAndPlainAndInstrumentalRemainDistinct() {
        auto item = lyric(); item["syncedLyrics"] = "";
        auto values = lyricsCandidates({item}, context());
        QVERIFY(!values.first().toObject()["synced"].toBool());
        QVERIFY(!values.first().toObject()["text"].toString().isEmpty());
        item["instrumental"] = true;
        values = lyricsCandidates({item}, context());
        QVERIFY(values.first().toObject()["instrumental"].toBool());
        QVERIFY(values.first().toObject()["text"].toString().isEmpty());
        item["instrumental"] = false; item["syncedLyrics"] = "[00:01.00]原文\n[00:01.00]Translation";
        QVERIFY(lyricsCandidates({item}, context()).first().toObject()["synced"].toBool());
    }
    void lyricBoundsRejectInvalidExpandedAndEmptyContent() {
        QVERIFY(validLyrics("[00:01.00]A\n[00:02.00]B", 120));
        QVERIFY(validLyrics("Plain text"));
        QVERIFY(!validLyrics("[01:99.00]Bad", 120));
        QVERIFY(!validLyrics("[10:00.00]Later", 120));
        QVERIFY(!validLyrics(QString(8193, 'x')));
        QVERIFY(!validLyrics(QString(10001, '\n')));
        QVERIFY(!validLyrics("[ar:Artist]\n[ti:Title]"));
        QVERIFY(!validLyrics(QString("line") + QChar(0)));
        QString expanded;
        for (int i = 0; i < 300; ++i) expanded += QString("[00:01.00]").repeated(50) + QString(7000, 'x') + '\n';
        QVERIFY(!validLyrics(expanded));
    }
    void coversKeepReleaseIdentityAndConservativeAmbiguity() {
        QJsonObject release{{"id", "12345678-1234-1234-1234-123456789abc"}, {"title", "Album"}, {"status", "Official"},
            {"artist-credit", QJsonArray{QJsonObject{{"name", "Artist"}}}}, {"date", "2020-01-01"}};
        const auto one = coverCandidates({{"releases", QJsonArray{release}}}, context());
        QCOMPARE(automaticChoice(one), 0);
        auto second = release; second["id"] = "22345678-1234-1234-1234-123456789abc";
        QCOMPARE(automaticChoice(coverCandidates({{"releases", QJsonArray{release, second}}}, context())), -1);
        release["disambiguation"] = "Live edition";
        QCOMPARE(automaticChoice(coverCandidates({{"releases", QJsonArray{release}}}, context())), -1);
        release["id"] = "../../invalid";
        QVERIFY(coverCandidates({{"releases", QJsonArray{release}}}, context()).isEmpty());
    }
    void persistentStoreKeepsPinsAndResetIndependentOfCache() {
        QTemporaryDir directory; Store store(directory.path()); store.initialize();
        const auto c = context(); auto item = lyricsCandidates({lyric()}, c).first().toObject();
        auto saved = store.save(c, "lyrics", item, {}, true);
        QVERIFY(key(saved["blob"].toString().chopped(4)));
        QVERIFY(store.binding(c, "lyrics")["pinned"].toBool());
        QVERIFY_THROWS_EXCEPTION(std::runtime_error, store.save(c, "lyrics", item, {}, false));
        store.clearCache(); QVERIFY(!store.binding(c, "lyrics").isEmpty());
        auto newIdentity = c; newIdentity["identity"] = hash("replacement"); QVERIFY(store.binding(newIdentity, "lyrics").isEmpty());
        store.clear(c, "lyrics"); QVERIFY(store.binding(c, "lyrics")["disabled"].toBool());
        store.save(c, "lyrics", item, {}, true); QVERIFY(!store.binding(c, "lyrics")["disabled"].toBool());
        QFile file(directory.path() + "/objects/" + saved["blob"].toString()); QVERIFY(file.open(QIODevice::WriteOnly)); file.write("tampered"); file.close();
        QVERIFY(store.binding(c, "lyrics")["unavailable"].toBool());
        QVERIFY_THROWS_EXCEPTION(std::runtime_error, store.save(c, "lyrics", item, {}, false));
    }
    void coverStorageHashesPixelsAndKeepsUnknownAlbumsSeparate() {
        QTemporaryDir directory; Store store(directory.path()); store.initialize();
        auto a = context("A"), b = context("B"); a["albumKey"] = ""; b["albumKey"] = "";
        a["scope"] = "album";
        auto result = store.save(a, "cover", {{"provider", "fixture"}}, picture(), true);
        QCOMPARE(result["key"], a["trackKey"]);
        QVERIFY(!store.binding(a, "cover").isEmpty()); QVERIFY(store.binding(b, "cover").isEmpty());
        QVERIFY(result["accent"].toString().startsWith('#'));
        const auto preview = QUrl(store.previewImage(picture())); QVERIFY(preview.isLocalFile());
    }
    void malformedImagesAreRejected() {
        QVERIFY_THROWS_EXCEPTION(std::runtime_error, normalizeImage("<svg xmlns='http://www.w3.org/2000/svg'></svg>"));
        QVERIFY_THROWS_EXCEPTION(std::runtime_error, normalizeImage(QByteArray(ImageLimit + 1, 'x')));
        QImage image(8193, 1, QImage::Format_RGB32); image.fill(Qt::black);
        QByteArray bytes; QBuffer buffer(&bytes); buffer.open(QIODevice::WriteOnly); image.save(&buffer, "PNG");
        QVERIFY_THROWS_EXCEPTION(std::runtime_error, normalizeImage(bytes));
        QVERIFY(!normalizeImage(picture()).jpeg.isEmpty());
    }
    void serviceManualSearchPreviewChooseAndOfflineRestart() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false};};
        {
            OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
            QSignalSpy changed(&service, &OnlineService::resourceChanged);
            service.open(json(context()), "lyrics");
            QTRY_VERIFY(service.state()["context"].toMap().contains("trackKey"));
            QCOMPARE(server.requests.size(), 0);
            service.search("Song", "Artist", "Album");
            QTRY_COMPARE_WITH_TIMEOUT(service.state()["status"].toString(), QString("candidates"), 5000);
            QCOMPARE(service.state()["candidates"].toList().size(), 1);
            service.preview(0); QTRY_VERIFY(!service.state()["previewText"].toString().isEmpty());
            service.choose(false); QTRY_COMPARE(changed.size(), 1);
            QCOMPARE(changed.first()[1].toString(), QString("lyrics"));
            QVERIFY(server.requests.first().toLower().contains("user-agent: liusheng/"));
            QVERIFY(!server.requests.first().contains("/music/"));
            QVERIFY(!server.requests.first().toLower().contains("cookie:"));
        }
        Store stored(directory.path()); QVERIFY(!stored.binding(context(), "lyrics").isEmpty());
        OnlineService reopened; reopened.configureTest(server.url()); reopened.setStorageRoot(directory.path());
        reopened.open(json(context()), "lyrics"); QTRY_COMPARE(reopened.state()["source"].toString(), QString("LRCLIB"));
        QCOMPARE(server.requests.size(), 1);
    }
    void disabledAutomaticPerformsNoRequests() {
        QTemporaryDir directory; Server server; OnlineService service;
        service.configureTest(server.url()); service.setStorageRoot(directory.path());
        service.requestAutomatic(contexts({context()}));
        QTest::qWait(50); QCOMPARE(server.requests.size(), 0);
    }
    void autoUniqueChoicePersistsAndDoesNotFetchItAgain() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false};};
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path()); service.setAutoLyrics(true);
        QSignalSpy changed(&service, &OnlineService::resourceChanged);
        service.requestAutomatic(contexts({context()})); QTRY_COMPARE_WITH_TIMEOUT(changed.size(), 1, 5000);
        service.requestAutomatic(contexts({context()})); QTest::qWait(100); QCOMPARE(server.requests.size(), 1);
        Store store(directory.path()); QVERIFY(!store.binding(context(), "lyrics")["pinned"].toBool());
    }
    void autoAmbiguityRemainsUnselected() {
        QTemporaryDir directory; Server server;
        auto second = lyric(); second["id"] = 2; second["syncedLyrics"] = "[00:03.00]Different fixture";
        server.handler = [second](const QUrl &) {return Response{200, QJsonDocument(QJsonArray{lyric(), second}).toJson(), {}, false};};
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path()); service.setAutoLyrics(true);
        QSignalSpy changed(&service, &OnlineService::resourceChanged);
        service.requestAutomatic(contexts({context()}));
        QTRY_COMPARE_WITH_TIMEOUT(service.state()["automaticStatus"].toString(), QString("review"), 5000);
        QCOMPARE(changed.size(), 0);
    }
    void timeoutsAndCancellationLeaveNoSelectedResource() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{200, {}, {}, true};};
        OnlineService service; service.configureTest(server.url(), 200); service.setStorageRoot(directory.path());
        service.open(json(context()), "lyrics"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE_WITH_TIMEOUT(service.state()["status"].toString(), QString("retry"), 10000);
        QVERIFY(service.state()["message"].toString().contains(QStringLiteral("超时")));
        service.search("Song", "Artist", "Album"); QTRY_VERIFY(service.state()["busy"].toBool()); service.cancel();
        QTRY_VERIFY(!service.state()["busy"].toBool());
        QTest::qWait(300); QVERIFY(service.state()["message"].toString().contains(QStringLiteral("取消")));
        Store store(directory.path()); QVERIFY(store.binding(context(), "lyrics").isEmpty());
    }
    void foreignRedirectAndHugeResponseAreRejected() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{302, {}, "Location: https://foreign.invalid/data\r\n", false};};
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.open(json(context()), "lyrics"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("error")); QCOMPARE(server.requests.size(), 1);
        server.handler = [](const QUrl &) {return Response{200, QByteArray(JsonLimit + 100, 'x'), {}, false};};
        service.search("Song", "Artist", "Album");
        QTRY_VERIFY_WITH_TIMEOUT(server.requests.size() >= 2 && !service.state()["busy"].toBool(), 5000);
        QVERIFY(service.state()["message"].toString().contains(QStringLiteral("大小")));
    }
    void rateLimitsSurviveServiceRestart() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{429, {}, "Retry-After: 60\r\n", false};};
        {
            OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
            service.open(json(context()), "lyrics"); service.search("Song", "Artist", "Album");
            QTRY_COMPARE(service.state()["status"].toString(), QString("waiting"));
            QVERIFY(service.state()["retryAt"].toLongLong() > QDateTime::currentMSecsSinceEpoch());
        }
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.open(json(context()), "lyrics"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("waiting"));
        QVERIFY(service.state()["message"].toString().contains(QStringLiteral("稍后"))); QCOMPARE(server.requests.size(), 1);
        service.cancel(); QTRY_VERIFY(!service.state()["busy"].toBool());
    }
    void batchPersistsPerItemAndReopensPaused() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &url) {
            const auto name = QUrlQuery(url).queryItemValue("track_name");
            return Response{200, QJsonDocument(QJsonArray{lyric(name, name == "First" ? 1 : 2)}).toJson(), {}, false};
        };
        {
            OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
            service.startBatch(contexts({context("First"), context("Second")}), "lyrics");
            QTRY_COMPARE_WITH_TIMEOUT(service.state()["batchDone"].toInt(), 2, 5000);
            QCOMPARE(service.state()["batchRemaining"].toInt(), 0);
            QCOMPARE(service.state()["batchResults"].toList().size(), 2);
        }
        OnlineService reopened; reopened.configureTest(server.url(), 4000); reopened.setStorageRoot(directory.path()); reopened.inspectBatch();
        QTRY_COMPARE(reopened.state()["batchDone"].toInt(), 2); QVERIFY(reopened.state()["batchPaused"].toBool());
        QCOMPARE(server.requests.size(), 2);
    }
    void batchPauseCancelAndManualPriority() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{200, {}, {}, true};};
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.startBatch(contexts({context()}), "lyrics"); QTRY_COMPARE(server.requests.size(), 1);
        service.pauseBatch(); QTRY_VERIFY(service.state()["batchPaused"].toBool());
        QCOMPARE(service.state()["batchRemaining"].toInt(), 1);
        server.handler = [](const QUrl &) {return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false};};
        service.open(json(context()), "lyrics"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates"));
        QCOMPARE(service.state()["batchDone"].toInt(), 0);
        service.cancelBatch(); QTRY_COMPARE(service.state()["batchRemaining"].toInt(), 0);
    }
    void coverDownloadRedirectPreviewSaveAndReleaseGroupIdentity_data() {
        QTest::addColumn<bool>("fallback");
        QTest::newRow("release-front") << false;
        QTest::newRow("release-group-fallback") << true;
    }
    void coverDownloadRedirectPreviewSaveAndReleaseGroupIdentity() {
        QFETCH(bool, fallback);
        QTemporaryDir directory; Server server;
        const QString id = "12345678-1234-1234-1234-123456789abc";
        const QString group = "22345678-1234-1234-1234-123456789abc";
        QJsonObject release{{"id", id}, {"title", "Album"}, {"status", "Official"}, {"artist-credit", QJsonArray{QJsonObject{{"name", "Artist"}}}}, {"release-group", QJsonObject{{"id", group}}}};
        server.handler = [release, fallback](const QUrl &url) {
            if (url.path().startsWith("/ws/2/")) return Response{200, QJsonDocument(QJsonObject{{"releases", QJsonArray{release}}}).toJson(), {}, false};
            if (fallback && url.path().startsWith("/release/")) return Response{404, {}, {}, false};
            if (url.path().startsWith("/release")) return Response{302, {}, "Location: https://archive.org/download/fixture/front.jpg\r\n", false};
            return Response{200, picture(), "Content-Type: image/png\r\n", false};
        };
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        QSignalSpy saved(&service, &OnlineService::resourceChanged);
        service.open(json(context()), "cover"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates"));
        service.preview(0); QTRY_COMPARE(service.state()["status"].toString(), QString("preview"));
        QVERIFY(QUrl(service.state()["previewUrl"].toString()).isLocalFile());
        QCOMPARE(service.state()["candidates"].toList().first().toMap()["id"].toString(), id);
        service.choose(false); QTRY_COMPARE(saved.size(), 1);
        Store store(directory.path()); const auto binding = store.binding(context(), "cover");
        QCOMPARE(binding["sourceId"].toString(), fallback ? group : id);
        QVERIFY(binding["pinned"].toBool());
        service.preview(0); QTRY_COMPARE(service.state()["status"].toString(), QString("preview"));
        QVERIFY(!service.state()["previewUrl"].toString().isEmpty());
    }
    void batchRateLimitPausesWithoutConsumingPendingItems() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{429, {}, "Retry-After: 60\r\n", false};};
        OnlineService service; service.configureTest(server.url()); service.setStorageRoot(directory.path());
        service.startBatch(contexts({context("First"), context("Second")}), "lyrics");
        QTRY_COMPARE(service.state()["batchPhase"].toString(), QString("waiting"));
        QCOMPARE(service.state()["batchDone"].toInt(), 0);
        QCOMPARE(service.state()["batchRemaining"].toInt(), 2);
        service.pauseBatch(); QTRY_VERIFY(service.state()["batchPaused"].toBool());
        service.resumeBatch(); QTRY_COMPARE(service.state()["batchPhase"].toString(), QString("waiting"));
        QCOMPARE(service.state()["batchRemaining"].toInt(), 2);
        QCOMPARE(service.state()["batchResults"].toList().size(), 1);
        QCOMPARE(server.requests.size(), 1);
    }
    void replacementAutomaticTrackCancelsStaleResources() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &url) {
            const auto name = QUrlQuery(url).queryItemValue("track_name");
            return name == "First" ? Response{200, {}, {}, true} : Response{200, QJsonDocument(QJsonArray{lyric(name)}).toJson(), {}, false};
        };
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path()); service.setAutoLyrics(true);
        QSignalSpy saved(&service, &OnlineService::resourceChanged);
        service.requestAutomatic(contexts({context("First")})); QTRY_COMPARE(server.requests.size(), 1);
        service.requestAutomatic(contexts({context("Second")})); QTRY_COMPARE_WITH_TIMEOUT(saved.size(), 1, 5000);
        QCOMPARE(saved.first()[0].toString(), context("Second")["trackKey"].toString());
        Store store(directory.path()); QVERIFY(store.binding(context("First"), "lyrics").isEmpty());
        QVERIFY(!store.binding(context("Second"), "lyrics").isEmpty());
    }
    void disablingPreferenceAbortsAutomaticAndKeepsManualAvailable() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{200, {}, {}, true};};
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path()); service.setAutoLyrics(true);
        QSignalSpy saved(&service, &OnlineService::resourceChanged);
        service.requestAutomatic(contexts({context()})); QTRY_COMPARE(server.requests.size(), 1);
        service.setAutoLyrics(false); QTest::qWait(100); QCOMPARE(saved.size(), 0);
        server.handler = [](const QUrl &) {return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false};};
        service.open(json(context()), "lyrics"); service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("candidates"));
    }
    void storageFailureIsReportedInsideTheAsyncBoundary() {
        QTemporaryDir directory; Server server;
        server.handler = [](const QUrl &) {return Response{200, QJsonDocument(QJsonArray{lyric()}).toJson(), {}, false};};
        OnlineService service; service.configureTest(server.url(), 4000); service.setStorageRoot(directory.path());
        service.open(json(context()), "lyrics"); QTRY_VERIFY(service.state()["context"].toMap().contains("trackKey"));
        QVERIFY(QDir(directory.path()+"/search").removeRecursively());
        QFile blocker(directory.path()+"/search"); QVERIFY(blocker.open(QIODevice::WriteOnly)); blocker.write("fixture"); blocker.close();
        service.search("Song", "Artist", "Album");
        QTRY_COMPARE(service.state()["status"].toString(), QString("error"));
        QVERIFY(!service.state()["busy"].toBool());
    }

};
#include "online_assets_live.h"
#include "online_multisource_live.h"
int main(int argc, char **argv) {
    QGuiApplication app(argc, argv);
    app.setApplicationVersion("test");
    if (app.arguments().contains("--multi-live")) return onlineMultisourceLive();
    if (app.arguments().contains("--reported-cover")) return onlineReportedCoverSmoke();
    if (app.arguments().contains("--live")) return onlineLiveSmoke();
    Tests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "online_assets_test.moc"
