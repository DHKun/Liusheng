#include "worker.h"
#include <QtCore/QCoreApplication>
#include <QtCore/QRandomGenerator>
#include <stdexcept>

namespace liusheng::online {
void Worker::abortRequest() {
    ++epoch_;
    if (reply_) {
        reply_->disconnect(this);
        reply_->abort();
        reply_->deleteLater();
        reply_ = nullptr;
    }
}
void Worker::retryState(const QString &host, qint64 until, int attempt, const QString &message) {
    if (!active_) return;
    const auto detail = QStringLiteral("%1 · %2；等待后自动重试（%3/2）")
        .arg(host, message).arg(std::max(1, attempt));
    if (active_->mode == "batch") {
        batch_["retryAt"] = until;
        batch_["retryHost"] = host;
        batch_["message"] = detail;
        recordBatchResult(*active_, "retry", detail);
        persistBatch();
        batchState();
    } else if (active_->mode == "manual") {
        post({{"busy", true}, {"status", "waiting"}, {"retryAt", until}, {"retryHost", host}, {"message", detail}});
    } else {
        post({{"automaticStatus", "waiting"}, {"automaticMessage", detail}});
    }
}
void Worker::request(QUrl url, bool image, Reply callback, int redirects, qint64 started, int retries, bool failFast) try {
    const auto now = QDateTime::currentMSecsSinceEpoch();
    if (blocked_) { callback({}, 0, QStringLiteral("本次运行已关闭在线资料请求")); return; }
    if (!safeUrl(url, image) || redirects > 4) { callback({}, 0, QStringLiteral("资料地址或跳转超出来源范围")); return; }
    const auto host = url.host();
    const auto token = epoch_;
    const auto until = retryAt_.value(host);
    if (until > now) {
        if (failFast) { callback({}, 429, QStringLiteral("来源冷却中")); return; }
        retryState(host, until, retries, QStringLiteral("来源暂时繁忙或要求稍后重试"));
        QTimer::singleShot(int(std::min<qint64>(86400000, until - now + 20)), this,
            [this, url, image, callback, redirects, token, retries, failFast] {
                if (!stopped_ && token == epoch_) request(url, image, callback, redirects, 0, retries, failFast);
            });
        return;
    }
    const qint64 wait = host == "musicbrainz.org" ? nextMusicBrainz_ - now : nextLookup_ - now;
    if (wait > 0) {
        QTimer::singleShot(int(wait + 1), this, [this, url, image, callback, redirects, started, token, retries, failFast] {
            if (!stopped_ && token == epoch_) request(url, image, callback, redirects, started, retries, failFast);
        });
        return;
    }
    // A provider's cooldown/rate-limit queue is separate from network I/O time.
    if (!started) started = now;
    const int deadline = failFast ? std::min(deadlineMs, 7000) : deadlineMs;
    if (now - started >= deadline) { callback({}, 408, QStringLiteral("资料请求超时")); return; }
    if (active_ && active_->mode == "batch") {
        batch_["retryAt"] = 0;
        batch_["message"] = QStringLiteral("正在查询 %1 · %2").arg(host, active_->query["title"].toString());
        batchState();
    } else if (active_ && active_->mode == "manual") {
        post({{"retryAt", qint64(0)}, {"busy", true}, {"status", image ? "previewing" : "searching"},
            {"message", QStringLiteral("正在查询 %1%2…").arg(host, retries ? QStringLiteral("（重试 %1/2）").arg(retries) : QString())}});
    }
    if (!network_) network_ = new QNetworkAccessManager(this);
    if (host == "musicbrainz.org") nextMusicBrainz_ = now + 1300;
    nextLookup_ = now + 250;
    QUrl target = url;
#ifdef LIUSHENG_ONLINE_TEST
    if (!testServer.isEmpty()) {
        target.setScheme(testServer.scheme()); target.setHost(testServer.host()); target.setPort(testServer.port());
    }
#endif
    QNetworkRequest request(target);
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute, QNetworkRequest::ManualRedirectPolicy);
    request.setAttribute(QNetworkRequest::CookieLoadControlAttribute, QNetworkRequest::Manual);
    request.setAttribute(QNetworkRequest::CookieSaveControlAttribute, QNetworkRequest::Manual);
    request.setRawHeader("User-Agent", "Liusheng/" + QCoreApplication::applicationVersion().toUtf8() + " (https://github.com/DHKun/Liusheng)");
    // Advertise formats shipped by every supported package. Some CDNs
    // negotiate WebP even for .jpg URLs; optional WebP plugins vary by host.
    request.setRawHeader("Accept", image ? "image/jpeg, image/png" : "application/json");
    request.setTransferTimeout(std::min(deadline, 8000));
    // Public metadata endpoints only: ordinary Referer, no user cookies or login.
    if (host == "music.163.com") request.setRawHeader("Referer", "https://music.163.com/");
    if (host == "c.y.qq.com") request.setRawHeader("Referer", "https://y.qq.com/portal/player.html");
    auto *reply = network_->get(request);
    reply_ = reply;
    const auto incoming = std::make_shared<QByteArray>();
    const auto failure = std::make_shared<QString>();
    const qsizetype limit = image ? ImageLimit : JsonLimit;
    reply->setReadBufferSize(limit + 1);
    connect(reply, &QIODevice::readyRead, this, [reply, incoming, failure, limit] {
        if (reply->isOpen()) incoming->append(reply->read(limit + 1 - incoming->size()));
        if (incoming->size() > limit) { *failure = QStringLiteral("在线资料响应超过大小限制"); reply->abort(); }
    });
    QTimer::singleShot(int(std::max<qint64>(1, deadline - (now - started))), reply, [reply, failure] {
        *failure = QStringLiteral("资料请求超时"); reply->abort();
    });
    connect(reply, &QNetworkReply::finished, this, [this, reply, url, image, callback, redirects, started, token, incoming, failure, limit, retries, failFast] {
        if (token != epoch_ || stopped_) { reply->deleteLater(); return; }
        try {
            if (reply->isOpen()) incoming->append(reply->read(limit + 1 - incoming->size()));
            int code = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
            const QUrl redirect = reply->attribute(QNetworkRequest::RedirectionTargetAttribute).toUrl();
            const auto error = reply->error();
            const auto retry = reply->rawHeader("Retry-After");
            const auto serverDate = reply->rawHeader("Date");
            reply_ = nullptr;
            reply->deleteLater();
            if (incoming->size() > limit) { callback({}, code, QStringLiteral("在线资料响应超过大小限制")); return; }
            // Qt's transfer timeout aborts with OperationCanceledError. Explicit
            // user cancellation disconnects this handler in abortRequest().
            const bool timedOut = *failure == QStringLiteral("资料请求超时") || error == QNetworkReply::TimeoutError
                || (failure->isEmpty() && error == QNetworkReply::OperationCanceledError);
            if (!failure->isEmpty() && !timedOut) { callback({}, code, *failure); return; }
            if (timedOut) code = 408;
            if (!redirect.isEmpty() && code >= 300 && code < 400) {
                this->request(url.resolved(redirect), image, callback, redirects + 1, started, retries, failFast);
                return;
            }
            const bool localFailure = timedOut || code == 0;
            const bool temporary = code == 408 || code == 429 || code == 500 || code == 502 || code == 503 || code == 504
                || (code == 0 && (error == QNetworkReply::RemoteHostClosedError || error == QNetworkReply::TemporaryNetworkFailureError));
            if (temporary) {
                if (code == 0) code = 502;
                const qint64 jitter = retry.isEmpty() ? QRandomGenerator::global()->bounded(250) : 0;
                retryAt_[url.host()] = QDateTime::currentMSecsSinceEpoch() + retryDelayMs(retry, serverDate, code, retries, QDateTime::currentMSecsSinceEpoch()) + jitter;
                persistBatch(); // also persist per-host cooldowns for manual searches
                if (retries < 2 && !failFast) {
                    this->request(url, image, callback, redirects, 0, retries + 1, failFast);
                    return;
                }
                const auto reason = localFailure ? (timedOut ? QStringLiteral("请求超时") : QStringLiteral("连接暂时中断"))
                    : QStringLiteral("暂时不可用（HTTP %1）").arg(code);
                callback({}, code, QStringLiteral("%1：%2").arg(url.host(), reason));
                return;
            }
            if (code == 404) { callback({}, code, {}); return; }
            if (code < 200 || code >= 300 || error != QNetworkReply::NoError) {
                const auto message = code > 0 ? QStringLiteral("%1 请求失败（HTTP %2），可稍后重试").arg(url.host()).arg(code)
                    : QStringLiteral("%1 连接失败，请检查网络、代理或证书设置后重试").arg(url.host());
                callback({}, code, message); return;
            }
            retryAt_.remove(url.host());
            callback(*incoming, code, {});
        } catch (const std::exception &error) { finish("error", QString::fromUtf8(error.what())); }
    });
} catch (const std::exception &error) { finish("error", QString::fromUtf8(error.what())); }
}
