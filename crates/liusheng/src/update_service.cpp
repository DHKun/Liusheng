#include "update_service.h"
#include <algorithm>
#include <QtCore/QCoreApplication>
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QSaveFile>
#include <QtCore/QSysInfo>
#include <QtGui/QDesktopServices>
#include <QtNetwork/QNetworkProxyFactory>
#include <QtNetwork/QNetworkRequest>
#include <QtNetwork/QSslSocket>

namespace liusheng {
namespace {
QString storage(const char *variable, const QString &fallback, const QString &file) {
    const auto base = qEnvironmentVariable(variable);
    return (base.isEmpty() ? QDir::homePath() + fallback : base) + "/liusheng/" + file;
}
QJsonObject readJson(const QString &path, qint64 limit) {
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly) || file.size() > limit) return {};
    return QJsonDocument::fromJson(file.read(limit + 1)).object();
}
bool writeJson(const QString &path, const QJsonObject &object) {
    if (!QDir().mkpath(QFileInfo(path).absolutePath())) return false;
    QSaveFile file(path);
    if (!file.open(QIODevice::WriteOnly)) return false;
    file.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
    const auto bytes = QJsonDocument(object).toJson(QJsonDocument::Compact);
    return file.write(bytes) == bytes.size() && file.commit();
}
bool safeEtag(const QByteArray &value) {
    return value.size() <= 512 && std::all_of(value.begin(), value.end(), [](char c) { return c >= 0x20 && c < 0x7f; });
}
QString platformName() {
    const auto architecture = QSysInfo::buildCpuArchitecture();
#ifdef Q_OS_MACOS
    return architecture == "arm64" ? QStringLiteral("macos-arm64") : QStringLiteral("unsupported");
#else
    return architecture == "x86_64" ? QStringLiteral("linux-x86_64") : QStringLiteral("unsupported");
#endif
}
QString preferredPackage() {
#ifdef Q_OS_MACOS
    return QStringLiteral("macos");
#else
    if (qEnvironmentVariableIsSet("APPIMAGE")) return QStringLiteral("appimage");
    QFile file(QStringLiteral("/etc/os-release"));
    if (file.open(QIODevice::ReadOnly)) {
        QSet<QByteArray> identities;
        for (auto line : file.read(16384).split('\n')) {
            if (line.startsWith("ID=") || line.startsWith("ID_LIKE=")) {
                auto value = line.mid(line.indexOf('=') + 1).trimmed();
                value.replace('"', ""); value.replace('\'', "");
                for (const auto &word : value.split(' ')) identities.insert(word.toLower());
            }
        }
        if (identities.contains("fedora")) return QStringLiteral("rpm");
        if (identities.contains("debian") || identities.contains("ubuntu")) return QStringLiteral("deb");
    }
    return QStringLiteral("appimage");
#endif
}
class SystemProxyFactory final : public QNetworkProxyFactory {
    QList<QNetworkProxy> queryProxy(const QNetworkProxyQuery &query) override {
        return QNetworkProxyFactory::systemProxyForQuery(query);
    }
};
} // namespace

UpdateService::UpdateService(QObject *parent) : QObject(parent),
    cachePath_(storage("XDG_CACHE_HOME", "/.cache", "release-cache.json")),
    preferencesPath_(storage("XDG_CONFIG_HOME", "/.config", "update-preferences.json")),
    current_(QCoreApplication::applicationVersion()), platform_(platformName()) {
    const auto args = QCoreApplication::arguments();
    for (const auto &flag : {"--ui-test", "--functional-test", "--smoke-test", "--output-smoke-test", "--startup-benchmark", "--no-update-check"})
        networkBlocked_ |= args.contains(QString::fromLatin1(flag));
    networkBlocked_ |= qEnvironmentVariableIsSet("LIUSHENG_QA_DIR");
    deadline_.setSingleShot(true);
    connect(&deadline_, &QTimer::timeout, this, [this] {
        if (reply_) { abortReason_ = QStringLiteral("连接 GitHub 超时，请检查网络或系统代理后重试。"); reply_->abort(); }
    });
}
#ifdef LIUSHENG_UPDATE_TEST
UpdateService::UpdateService(const QUrl &endpoint, const QString &root, const QString &current,
    const QString &platform, const QString &preferred, QObject *parent) : UpdateService(parent) {
    endpoint_ = endpoint; cachePath_ = root + "/cache.json"; preferencesPath_ = root + "/preferences.json";
    current_ = current; platform_ = platform; preferred_ = preferred; networkBlocked_ = false;
}
#endif
UpdateService::~UpdateService() {
    if (reply_) { reply_->disconnect(this); reply_->abort(); }
}
void UpdateService::setAutomaticEnabled(bool enabled) {
    if (automaticEnabled_ == enabled) return;
    automaticEnabled_ = enabled;
    if (!enabled && reply_ && !manual_) {
        reply_->disconnect(this); reply_->abort(); reply_->deleteLater(); reply_.clear(); deadline_.stop();
        status_ = "idle"; message_ = QStringLiteral("启动检查已关闭，可随时手动检查。"); emit changed();
    }
    emit automaticEnabledChanged();
}
void UpdateService::initialize() {
    if (initialized_) return;
    initialized_ = true;
    if (preferred_.isEmpty()) preferred_ = preferredPackage();
    const auto preference = readJson(preferencesPath_, 4096);
    const auto skipped = updates::version(preference.value("skippedVersion").toString());
    if (skipped && skipped->prerelease.isEmpty()) skipped_ = skipped->canonical;
    const auto cache = readJson(cachePath_, updates::MaxResponse * 2);
    if (cache.value("schema").toInt() == 1) {
        const auto json = cache.value("release").toObject();
        if (updates::parseRelease(json, platform_, preferred_)) {
            cachedRelease_ = json;
            const auto tag = cache.value("etag").toString().toLatin1();
            if (safeEtag(tag)) etag_ = tag;
        }
        const auto checked = QDateTime::fromString(cache.value("checkedAt").toString(), Qt::ISODate);
        if (checked.isValid() && checked <= QDateTime::currentDateTimeUtc()) checkedAt_ = checked;
        const auto retry = QDateTime::fromString(cache.value("retryAt").toString(), Qt::ISODate);
        if (retry.isValid()) retryAt_ = std::min(retry, QDateTime::currentDateTimeUtc().addDays(1));
    }
    emit changed();
}
void UpdateService::startupCheck() {
    if (startupAttempted_) return;
    startupAttempted_ = true;
    if (automaticEnabled_ && !networkBlocked_) check(false);
}
void UpdateService::check(bool manual) {
    initialize();
    startupAttempted_ = true;
    if (busy()) { manual_ |= manual; return; }
    actionError_.clear();
    if (networkBlocked_) { fail(QStringLiteral("当前测试会话已关闭联网检查。")); return; }
    if (!updates::version(current_)) { fail(QStringLiteral("当前构建版本格式无效，请在 GitHub 发布页核对版本。")); return; }
    const auto now = QDateTime::currentDateTimeUtc();
    if (retryAt_ > now) {
        fail(QStringLiteral("GitHub 请求频率受限，可在 %1 后重试。").arg(retryAt_.toLocalTime().toString("MM-dd HH:mm:ss"))); return;
    }
    // Prevent repeated clicks from issuing a burst while keeping manual retry useful.
    if (lastAttempt_.isValid() && lastAttempt_.msecsTo(now) < 3000) {
        actionError_ = QStringLiteral("请稍候几秒再检查。"); emit changed(); return;
    }
    lastAttempt_ = now; manual_ = manual; retried304_ = false;
    if (!network_) {
        network_ = new QNetworkAccessManager(this);
        network_->setProxyFactory(new SystemProxyFactory);
#ifdef LIUSHENG_UPDATE_TEST
        // Loopback fixtures remain isolated from machine-specific proxy/PAC settings.
        if (endpoint_.host() == "127.0.0.1") network_->setProxy(QNetworkProxy::NoProxy);
#endif
    }
    if (endpoint_.scheme() == "https" && !QSslSocket::supportsSsl()) {
        fail(QStringLiteral("当前运行环境缺少 TLS 支持，请从发布页更新安装包。")); return;
    }
    request(true);
}
void UpdateService::request(bool conditional) {
    status_ = "checking"; message_ = QStringLiteral("正在检查 GitHub 正式版本…");
    incoming_.clear(); abortReason_.clear();
    QNetworkRequest request(endpoint_);
    request.setRawHeader("Accept", "application/vnd.github+json");
    request.setRawHeader("User-Agent", "Liusheng/" + current_.toLatin1());
    request.setRawHeader("X-GitHub-Api-Version", "2022-11-28");
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute, QNetworkRequest::ManualRedirectPolicy);
    request.setAttribute(QNetworkRequest::CacheLoadControlAttribute, QNetworkRequest::AlwaysNetwork);
    request.setAttribute(QNetworkRequest::CookieLoadControlAttribute, QNetworkRequest::Manual);
    request.setAttribute(QNetworkRequest::CookieSaveControlAttribute, QNetworkRequest::Manual);
    request.setTransferTimeout(8000);
    if (conditional && !etag_.isEmpty() && !cachedRelease_.isEmpty()) request.setRawHeader("If-None-Match", etag_);
    reply_ = network_->get(request);
    reply_->setReadBufferSize(32768);
    connect(reply_, &QIODevice::readyRead, this, &UpdateService::readIncoming);
    connect(reply_, &QNetworkReply::finished, this, &UpdateService::finishRequest);
    connect(reply_, &QNetworkReply::metaDataChanged, this, [this] {
        if (reply_ && reply_->header(QNetworkRequest::ContentLengthHeader).toLongLong() > updates::MaxResponse) {
            abortReason_ = QStringLiteral("GitHub 返回的数据超过大小限制。"); reply_->abort();
        }
    });
    deadline_.start(deadlineMs_);
    emit changed();
}
void UpdateService::readIncoming() {
    if (!reply_ || !abortReason_.isEmpty()) return;
    incoming_ += reply_->read(updates::MaxResponse - incoming_.size() + 1);
    if (incoming_.size() > updates::MaxResponse) {
        abortReason_ = QStringLiteral("GitHub 返回的数据超过大小限制。"); reply_->abort();
    }
}
void UpdateService::finishRequest() {
    if (!reply_) return;
    // finished() can still contain a final short chunk.
    if (abortReason_.isEmpty()) incoming_ += reply_->read(updates::MaxResponse - incoming_.size() + 1);
    auto *reply = reply_.data(); reply_.clear(); deadline_.stop();
    const auto error = reply->error();
    const int code = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
    const auto newEtag = reply->rawHeader("ETag");
    const auto retryHeader = reply->rawHeader("Retry-After");
    const auto resetHeader = reply->rawHeader("X-RateLimit-Reset");
    reply->deleteLater();
    if (!abortReason_.isEmpty()) { fail(abortReason_); return; }
    if (incoming_.size() > updates::MaxResponse) { fail(QStringLiteral("GitHub 返回的数据超过大小限制。")); return; }
    if (code == 403 || code == 429) {
        const auto now = QDateTime::currentDateTimeUtc();
        bool numeric = false;
        auto seconds = retryHeader.toLongLong(&numeric);
        if (!numeric) {
            const auto date = QDateTime::fromString(QString::fromLatin1(retryHeader), Qt::RFC2822Date);
            seconds = date.isValid() ? now.secsTo(date) : resetHeader.toLongLong() - now.toSecsSinceEpoch();
        }
        retryAt_ = now.addSecs(std::clamp<qint64>(seconds, 60, 86400)); saveCache();
        fail(QStringLiteral("GitHub 暂时限制了请求，可在 %1 后重试。").arg(retryAt_.toLocalTime().toString("MM-dd HH:mm:ss"))); return;
    }
    if (code == 404) {
        release_.reset(); cachedRelease_ = {}; etag_.clear(); checkedAt_ = QDateTime::currentDateTimeUtc(); saveCache();
        status_ = "empty"; message_ = QStringLiteral("GitHub 当前尚无可获取的正式 Release。"); emit changed(); emit finished(); return;
    }
    if (code == 304) {
        const auto cached = updates::parseRelease(cachedRelease_, platform_, preferred_);
        if (cached) { checkedAt_ = QDateTime::currentDateTimeUtc(); saveCache(); acceptRelease(*cached); return; }
        if (!retried304_) { retried304_ = true; etag_.clear(); request(false); return; }
        fail(QStringLiteral("更新缓存无效，请稍后重新检查。")); return;
    }
    if (code >= 300 && code < 400) { fail(QStringLiteral("更新地址发生重定向，请在 GitHub 发布页查看版本。")); return; }
    if (error != QNetworkReply::NoError || code != 200) {
        if (error == QNetworkReply::SslHandshakeFailedError) fail(QStringLiteral("GitHub 的 TLS 连接校验失败，请检查系统时间和代理证书。"));
        else fail(QStringLiteral("暂时无法连接 GitHub，请检查网络或系统代理后重试。"));
        return;
    }
    QJsonParseError parseError;
    const auto doc = QJsonDocument::fromJson(incoming_, &parseError);
    const auto parsed = parseError.error == QJsonParseError::NoError && doc.isObject()
        ? updates::parseRelease(doc.object(), platform_, preferred_) : std::nullopt;
    if (!parsed) { fail(QStringLiteral("发布信息格式无效，或版本属于草稿／预发布。请查看正式发布页。")); return; }
    cachedRelease_ = doc.object(); etag_ = safeEtag(newEtag) ? newEtag : QByteArray{};
    checkedAt_ = QDateTime::currentDateTimeUtc(); retryAt_ = {};
    saveCache(); acceptRelease(*parsed);
}
void UpdateService::acceptRelease(const updates::Release &release) {
    release_ = release;
    const auto local = updates::version(current_);
    const bool newer = local && updates::newerStable(release.parsed, *local);
    status_ = newer ? "available" : "current";
    message_ = newer ? QStringLiteral("发现新版本 v%1").arg(latestVersion())
        : local && QVersionNumber::compare(local->number, release.parsed.number) > 0
            ? QStringLiteral("当前构建高于 GitHub 最新正式版本 v%1。").arg(latestVersion())
            : QStringLiteral("当前已是最新正式版本。");
    emit changed(); emit finished();
}
void UpdateService::fail(const QString &message) {
    status_ = "error"; message_ = message;
    emit changed(); emit finished();
}
QString UpdateService::installHint() const {
#ifdef Q_OS_MACOS
    return QStringLiteral("下载后先退出留声，再替换 Applications 中的应用。系统版本要求以新 Release 说明为准。");
#else
    if (qEnvironmentVariableIsSet("APPIMAGE"))
        return QStringLiteral("下载新的 AppImage 后先退出当前程序，再替换旧文件并保留执行权限。系统要求见 Release 说明。");
    const auto executable = QCoreApplication::applicationFilePath();
    if (executable.startsWith(QDir::homePath() + '/') || executable.contains("/target/"))
        return QStringLiteral("当前程序位于用户目录或构建目录。源码安装建议更新仓库后执行 just install；改用安装包时，请同步更新启动入口。系统要求见 Release 说明。");
    return QStringLiteral("推荐格式根据当前系统推断。请核对 Release 的系统要求，并沿用原安装方式升级。下载后先退出留声。");
#endif
}
bool UpdateService::saveCache() {
    return writeJson(cachePath_, {{"schema", 1}, {"release", cachedRelease_}, {"etag", QString::fromLatin1(etag_)},
        {"checkedAt", checkedAt_.toString(Qt::ISODate)}, {"retryAt", retryAt_.toString(Qt::ISODate)}});
}
void UpdateService::remindLater() { snoozed_ = latestVersion(); emit changed(); }
bool UpdateService::saveSkipped(const QString &value) {
    if (!writeJson(preferencesPath_, {{"version", 1}, {"skippedVersion", value}})) {
        actionError_ = QStringLiteral("跳过偏好保存失败，请检查配置目录的写入权限。"); emit changed(); return false;
    }
    skipped_ = value; actionError_.clear(); emit changed(); return true;
}
bool UpdateService::skipVersion() {
    return status_ == "available" && !latestVersion().isEmpty() && saveSkipped(latestVersion());
}
bool UpdateService::clearSkippedVersion() { initialize(); return saveSkipped({}); }
bool UpdateService::openOfficial(const QUrl &url) {
#ifdef LIUSHENG_UPDATE_TEST
    lastOpenedForTest = url;
    return true;
#else
    if (QDesktopServices::openUrl(url)) return true;
    actionError_ = QStringLiteral("浏览器启动失败，请手动打开 GitHub 的 DHKun/Liusheng 发布页。"); emit changed(); return false;
#endif
}
bool UpdateService::openReleasePage() {
    return openOfficial(release_ ? release_->page : QUrl(updates::Repository + "/releases/latest"));
}
bool UpdateService::openAsset(int index) {
    if (status_ != "available" || !release_ || index < 0 || index >= release_->assets.size()) return false;
    const auto asset = release_->assets[index].toObject();
    const QUrl url(asset.value("url").toString());
    const auto path = QStringLiteral("/DHKun/Liusheng/releases/download/")
        + QString::fromLatin1(QUrl::toPercentEncoding(release_->tag)) + '/' + asset.value("name").toString();
    if (!updates::officialUrl(url, path)) return false;
    return openOfficial(url);
}
} // namespace liusheng
