#include "update_service.h"
#include <memory>
#include <QtTest/QTest>
#include <QtTest/QSignalSpy>
#include <QtCore/QTemporaryDir>
#include <QtCore/QFile>
#include <QtCore/QJsonDocument>
#include <QtNetwork/QTcpServer>
#include <QtNetwork/QTcpSocket>
using liusheng::UpdateService;
namespace policy = liusheng::updates;

class Server : public QObject {
public:
    QTcpServer server;
    int requests = 0;
    QList<QByteArray> headers;
    int code = 200;
    QByteArray body;
    QByteArray extra;
    int delay = 0;
    bool hang = false;
    bool omitLength = false;
    explicit Server(QObject *parent = nullptr) : QObject(parent) {
        if (!server.listen(QHostAddress::LocalHost)) qFatal("Loopback test server failed");
        connect(&server, &QTcpServer::newConnection, this, [this] {
            while (auto *socket = server.nextPendingConnection()) {
                auto request = std::make_shared<QByteArray>();
                auto handled = std::make_shared<bool>(false);
                connect(socket, &QTcpSocket::readyRead, this, [this, socket, request, handled] {
                    request->append(socket->readAll());
                    if (*handled || !request->contains("\r\n\r\n")) return;
                    *handled = true; ++requests; headers.append(*request);
                    if (hang) return;
                    const QByteArray response = "HTTP/1.1 " + QByteArray::number(code) + " Result\r\n"
                        + "Content-Type: application/json\r\n"
                        + (omitLength ? QByteArray{} : "Content-Length: " + QByteArray::number(body.size()) + "\r\n")
                        + "Connection: close\r\n" + extra + "\r\n" + body;
                    QTimer::singleShot(delay, socket, [socket, response] {
                        socket->write(response); socket->disconnectFromHost();
                    });
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
    }
    QByteArray header(int index, const QByteArray &name) const {
        for (const auto &line : headers[index].split('\n')) {
            const auto split = line.indexOf(':');
            if (split > 0 && line.left(split).trimmed().toLower() == name.toLower()) return line.mid(split+1).trimmed();
        }
        return {};
    }
    QUrl url() const { return QUrl(QStringLiteral("http://127.0.0.1:%1/latest").arg(server.serverPort())); }
};
static QJsonObject release(QString v = "0.4.0") {
    QJsonArray assets;
    const QStringList names{QStringLiteral("liusheng_%1_amd64.deb").arg(v),
        QStringLiteral("liusheng-%1-1.fc44.x86_64.rpm").arg(v),
        QStringLiteral("liusheng-%1-linux-x86_64.AppImage").arg(v),
        QStringLiteral("liusheng-%1-macos-arm64.zip").arg(v)};
    for (const auto &name : names) assets.append(QJsonObject{{"name", name}, {"size", 67108864}, {"state", "uploaded"},
        {"browser_download_url", policy::Repository + "/releases/download/v" + v + '/' + name},
        {"digest", "sha256:" + QString(64, 'a')}});
    return {{"tag_name", "v" + v}, {"html_url", policy::Repository + "/releases/tag/v" + v},
        {"prerelease", false}, {"draft", false}, {"assets", assets}, {"body", "## 更新\n<b>仅作为文本显示</b>"},
        {"published_at", "2026-09-09T10:00:00Z"}};
}
static void setRelease(Server &server, QString v = "0.4.0") {
    server.body = QJsonDocument(release(v)).toJson(QJsonDocument::Compact);
}
static void write(const QString &path, const QByteArray &bytes) {
    QFile file(path); if (!file.open(QIODevice::WriteOnly) || file.write(bytes) != bytes.size()) qFatal("Fixture write failed");
}
class UpdateTests : public QObject {
    Q_OBJECT
private slots:
    void semantic_version_ordering_data() {
        QTest::addColumn<QString>("current"); QTest::addColumn<QString>("remote"); QTest::addColumn<bool>("expected");
        QTest::newRow("numeric 10 vs 9") << "0.9.9" << "v0.10.0" << true;
        QTest::newRow("downgrade") << "1.0.0" << "v0.99.99" << false;
        QTest::newRow("equal") << "0.4.0" << "v0.4.0" << false;
        QTest::newRow("build metadata") << "0.4.0+local" << "v0.4.0+release" << false;
        QTest::newRow("rc to stable") << "0.4.0-rc.1" << "v0.4.0" << true;
        QTest::newRow("stable excludes prerelease") << "0.4.0" << "v0.5.0-beta" << false;
        QTest::newRow("major") << "1.9.9" << "v2.0.0" << true;
    }
    void semantic_version_ordering() {
        QFETCH(QString, current); QFETCH(QString, remote); QFETCH(bool, expected);
        auto a = policy::version(current), b = policy::version(remote);
        QVERIFY(a && b); QCOMPARE(policy::newerStable(*b, *a), expected);
    }
    void malformed_versions() {
        for (const auto &text : {"0.4", "01.2.3", "v0.4.0evil", " 0.4.0", "1.2.3-01", "1.2.3-", "1.2.3+", "99999999999999.0.1"})
            QVERIFY2(!policy::version(QString::fromLatin1(text)), text);
    }
    void release_requires_published_stable_official_repository() {
        auto json = release(); QVERIFY(policy::parseRelease(json, "linux-x86_64", "rpm"));
        for (const auto &key : {"draft", "prerelease"}) {
            auto bad = json; bad[key] = true; QVERIFY(!policy::parseRelease(bad, "linux-x86_64", "deb"));
            bad.remove(key); QVERIFY(!policy::parseRelease(bad, "linux-x86_64", "deb"));
        }
        for (const auto &url : {"http://github.com/DHKun/Liusheng/releases/tag/v0.4.0", "https://github.com.evil.test/DHKun/Liusheng/releases/tag/v0.4.0",
            "https://github.com/evil/Liusheng/releases/tag/v0.4.0", "https://user@github.com/DHKun/Liusheng/releases/tag/v0.4.0",
            "https://github.com/DHKun/Liusheng/releases/tag/v0.4.0?x=y", "file:///tmp/test"}) {
            auto bad = json; bad["html_url"] = QString::fromLatin1(url); QVERIFY(!policy::parseRelease(bad, "linux-x86_64", "deb"));
        }
    }
    void package_selection_and_unsupported_architecture() {
        auto linuxRelease = policy::parseRelease(release(), "linux-x86_64", "rpm"); QVERIFY(linuxRelease); QCOMPARE(linuxRelease->assets.size(), 3);
        QCOMPARE(linuxRelease->assets[linuxRelease->recommended].toObject()["kind"].toString(), "rpm");
        auto mac = policy::parseRelease(release(), "macos-arm64", "macos"); QVERIFY(mac); QCOMPARE(mac->assets.size(), 1);
        auto other = policy::parseRelease(release(), "unsupported", "appimage"); QVERIFY(other); QVERIFY(other->assets.isEmpty());
        auto portable = policy::parseRelease(release(), "linux-x86_64", "appimage");
        QCOMPARE(portable->assets[portable->recommended].toObject()["kind"].toString(), "appimage");
    }
    void malicious_empty_mismatched_duplicate_assets_are_excluded() {
        auto json = release(); auto assets = json["assets"].toArray();
        auto bad = assets[0].toObject(); bad["browser_download_url"] = "https://evil.test/installer"; assets[0] = bad;
        bad = assets[1].toObject(); bad["size"] = 0; assets[1] = bad;
        assets.append(assets[2]); // Ambiguous duplicate AppImages also stay on the release page.
        json["assets"] = assets;
        const auto parsed = policy::parseRelease(json, "linux-x86_64", "deb"); QVERIFY(parsed); QVERIFY(parsed->assets.isEmpty());
        QCOMPARE(parsed->recommended, -1);
    }
    void notes_are_bounded_and_kept_literal() {
        auto json = release(); json["body"] = QString(50000, 'x');
        QCOMPARE(policy::parseRelease(json, "linux-x86_64", "deb")->notes.size(), policy::MaxNotes);
        QVERIFY(policy::parseRelease(release(), "linux-x86_64", "deb")->notes.contains("<b>"));
    }
    void startup_once_and_request_headers() {
        QTemporaryDir root; Server server; setRelease(server);
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.startupCheck(); service.startupCheck(); QVERIFY(service.busy());
        QTRY_VERIFY(!service.busy()); QCOMPARE(server.requests, 1); QCOMPARE(service.status(), "available"); QVERIFY(service.notifyAvailable());
        QCOMPARE(server.header(0, "User-Agent"), QByteArray("Liusheng/0.3.3"));
        QVERIFY(server.header(0, "Authorization").isEmpty()); QVERIFY(!server.headers[0].contains(root.path().toUtf8()));
        QVERIFY(server.header(0, "Cookie").isEmpty()); QVERIFY(!service.lastChecked().isEmpty());
    }
    void disabled_automatic_keeps_manual_check() {
        QTemporaryDir root; Server server; setRelease(server);
        UpdateService service(server.url(), root.path(), "0.3.3", "macos-arm64", "macos");
        service.setAutomaticEnabled(false); service.startupCheck(); QCOMPARE(server.requests, 0);
        service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(service.status(), "available"); QCOMPARE(server.requests, 1);
    }
    void test_sessions_make_zero_requests() {
        QTemporaryDir root; Server server; setRelease(server);
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.setNetworkBlockedForTest(true); service.startupCheck(); service.check();
        QCOMPARE(server.requests, 0); QCOMPARE(service.status(), "error");
    }
    void concurrent_manual_check_joins_startup_request() {
        QTemporaryDir root; Server server; setRelease(server); server.delay = 70;
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.startupCheck(); service.check(true); service.setAutomaticEnabled(false);
        QTRY_VERIFY(!service.busy()); QCOMPARE(server.requests, 1); QCOMPARE(service.status(), "available");
    }
    void disabling_automatic_cancels_pending_background_request() {
        QTemporaryDir root; Server server; setRelease(server); server.hang = true;
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.startupCheck(); QTRY_COMPARE(server.requests, 1); service.setAutomaticEnabled(false);
        QVERIFY(!service.busy()); QCOMPARE(service.status(), "idle"); QVERIFY(!service.notifyAvailable());
    }
    void latest_equal_or_older_never_prompts() {
        QTemporaryDir root; Server server; setRelease(server);
        UpdateService service(server.url(), root.path(), "0.5.0", "linux-x86_64", "deb");
        service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(service.status(), "current"); QVERIFY(!service.notifyAvailable());
    }
    void etag_revalidation_after_restart() {
        QTemporaryDir root; Server server; setRelease(server); server.extra = "ETag: W/\"fixture\"\r\n";
        { UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
          service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(service.status(), "available"); }
        server.code = 304; server.body.clear();
        UpdateService restored(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        restored.check(); QTRY_VERIFY(!restored.busy()); QCOMPARE(restored.status(), "available");
        QCOMPARE(server.header(server.headers.size()-1, "If-None-Match"), QByteArray("W/\"fixture\""));
    }
    void corrupt_cache_is_ignored_and_unexpected_304_retries_once() {
        QTemporaryDir root; write(root.path()+"/cache.json", "broken"); Server server; server.code = 304;
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(server.requests, 2); QCOMPARE(service.status(), "error");
        QVERIFY(server.header(0, "If-None-Match").isEmpty());
    }
    void skipped_version_persists_manual_still_reveals_it() {
        QTemporaryDir root; Server server; setRelease(server);
        { UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
          service.check(); QTRY_VERIFY(!service.busy()); QVERIFY(service.skipVersion()); QVERIFY(service.ignored()); QVERIFY(!service.notifyAvailable()); }
        { UpdateService restored(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
          restored.check(); QTRY_VERIFY(!restored.busy()); QCOMPARE(restored.status(), "available"); QVERIFY(restored.ignored());
          QVERIFY(restored.openAsset(0)); QVERIFY(restored.clearSkippedVersion()); QVERIFY(restored.notifyAvailable()); }
        { UpdateService restored(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
          restored.check(); QTRY_VERIFY(!restored.busy()); QVERIFY(restored.skipVersion()); }
        setRelease(server, "0.5.0");
        UpdateService newer(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        newer.check(); QTRY_VERIFY(!newer.busy()); QVERIFY(newer.notifyAvailable());
    }
    void later_is_session_only() {
        QTemporaryDir root; Server server; setRelease(server);
        { UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
          service.check(); QTRY_VERIFY(!service.busy()); service.remindLater(); QVERIFY(!service.notifyAvailable()); }
        UpdateService restored(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        restored.startupCheck(); QTRY_VERIFY(!restored.busy()); QVERIFY(restored.notifyAvailable());
    }
    void skip_write_failure_preserves_previous_preference() {
        QTemporaryDir root; Server server; setRelease(server);
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.check(); QTRY_VERIFY(!service.busy()); QDir().mkdir(root.path()+"/preferences.json");
        QVERIFY(!service.skipVersion()); QVERIFY(!service.actionError().isEmpty()); QVERIFY(!service.ignored());
    }
    void browser_actions_use_only_validated_asset_or_release() {
        QTemporaryDir root; Server server; setRelease(server);
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        QVERIFY(!service.openAsset(0)); QVERIFY(service.openReleasePage()); QCOMPARE(service.lastOpenedForTest.host(), "github.com");
        service.check(); QTRY_VERIFY(!service.busy()); QVERIFY(!service.openAsset(-1)); QVERIFY(!service.openAsset(100));
        QVERIFY(service.openAsset(0)); QVERIFY(service.lastOpenedForTest.path().endsWith("_amd64.deb"));
        QVERIFY(service.openReleasePage()); QCOMPARE(service.lastOpenedForTest.path(), "/DHKun/Liusheng/releases/tag/v0.4.0");
    }
    void http_errors_and_invalid_body_data() {
        QTest::addColumn<int>("code"); QTest::addColumn<QByteArray>("body"); QTest::addColumn<QString>("status");
        QTest::newRow("404") << 404 << QByteArray("{}") << "empty";
        QTest::newRow("500") << 500 << QByteArray("{}") << "error";
        QTest::newRow("broken JSON") << 200 << QByteArray("{") << "error";
        QTest::newRow("HTML proxy page") << 200 << QByteArray("<html>sign in</html>") << "error";
        auto draft = release(); draft["draft"] = true;
        QTest::newRow("draft") << 200 << QJsonDocument(draft).toJson() << "error";
    }
    void http_errors_and_invalid_body() {
        QFETCH(int, code); QFETCH(QByteArray, body); QFETCH(QString, status);
        QTemporaryDir root; Server server; server.code=code; server.body=body;
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(service.status(), status); QVERIFY(!service.notifyAvailable());
    }
    void rate_limit_is_persisted_and_manual_respects_retry_after() {
        QTemporaryDir root; Server server; server.code=429; server.body="{}"; server.extra="Retry-After: 120\r\n";
        { UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
          service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(service.status(), "error"); }
        UpdateService restored(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        restored.check(); QVERIFY(!restored.busy()); QCOMPARE(server.requests, 1); QVERIFY(!restored.notifyAvailable());
    }
    void absolute_timeout_and_destructor_cleanup() {
        QTemporaryDir root; Server server; server.hang=true;
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.setDeadlineForTest(80); service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(service.status(), "error");
        auto *pending = new UpdateService(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        pending->check(); delete pending;
    }
    void oversized_response_is_rejected() {
        QTemporaryDir root; Server server; server.body=QByteArray(policy::MaxResponse+1, 'x');
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(service.status(), "error"); QVERIFY(!service.notifyAvailable());
    }
    void response_without_length_is_bounded() {
        QTemporaryDir root; Server server; server.omitLength = true;
        server.body = QByteArray(policy::MaxResponse * 2, 'x');
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(service.status(), "error");
        QVERIFY(service.message().contains(QStringLiteral("大小限制")));
    }
    void completed_checks_have_click_cooldown() {
        QTemporaryDir root; Server server; setRelease(server);
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.check(); QTRY_VERIFY(!service.busy()); service.check(); service.check();
        QCOMPARE(server.requests, 1); QVERIFY(!service.actionError().isEmpty());
    }
    void network_failure_preserves_success_date_and_never_claims_current() {
        QTemporaryDir root; Server server; setRelease(server);
        QString checked;
        { UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
          service.check(); QTRY_VERIFY(!service.busy()); checked = service.lastChecked(); }
        server.code = 503;
        UpdateService restored(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        restored.check(); QTRY_VERIFY(!restored.busy()); QCOMPARE(restored.status(), "error");
        QCOMPARE(restored.lastChecked(), checked); QVERIFY(!restored.notifyAvailable());
    }
    void pending_assets_keep_release_page_and_disabled_download() {
        QTemporaryDir root; Server server; auto data = release(); data["assets"] = QJsonArray{};
        server.body = QJsonDocument(data).toJson();
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(service.status(), "available");
        QVERIFY(service.assets().isEmpty()); QVERIFY(!service.openAsset(0)); QVERIFY(service.openReleasePage());
    }
    void redirect_is_never_followed() {
        QTemporaryDir root; Server server; server.code=302; server.extra="Location: http://127.0.0.1:1/forbidden\r\n";
        UpdateService service(server.url(), root.path(), "0.3.3", "linux-x86_64", "deb");
        service.check(); QTRY_VERIFY(!service.busy()); QCOMPARE(service.status(), "error"); QCOMPARE(server.requests, 1);
    }
};
int main(int argc, char **argv) {
    QCoreApplication app(argc, argv); app.setApplicationVersion("0.3.3");
    if (app.arguments().contains("--live")) {
        QTemporaryDir root;
        UpdateService service(policy::Endpoint, root.path(), app.applicationVersion(), "linux-x86_64", "appimage");
        QObject::connect(&service, &UpdateService::finished, &app, [&] {
            qInfo().noquote() << service.status() << service.message() << "assets=" << service.assets().size();
            app.exit(service.status() == "current" || service.status() == "available" ? 0 : 1);
        });
        QTimer::singleShot(0, &service, [&] {service.check();});
        return app.exec();
    }
    UpdateTests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "update_service_test.moc"
