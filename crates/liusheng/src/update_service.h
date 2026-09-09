#pragma once
#include <QtCore/QObject>
#include <QtCore/QDateTime>
#include <QtCore/QPointer>
#include <QtCore/QTimer>
#include <QtCore/QVariantList>
#include <QtNetwork/QNetworkAccessManager>
#include <QtNetwork/QNetworkReply>
#include <QtNetwork/QSslSocket>
#include <QtQml/qqmlregistration.h>
#include "updates/release_policy.h"

namespace liusheng {
// Checking is asynchronous and opt-out. The service never installs or executes a download.
class UpdateService : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(bool automaticEnabled READ automaticEnabled WRITE setAutomaticEnabled NOTIFY automaticEnabledChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY changed)
    Q_PROPERTY(QString status READ status NOTIFY changed)
    Q_PROPERTY(QString message READ message NOTIFY changed)
    Q_PROPERTY(QString currentVersion READ currentVersion CONSTANT)
    Q_PROPERTY(QString latestVersion READ latestVersion NOTIFY changed)
    Q_PROPERTY(QString notes READ notes NOTIFY changed)
    Q_PROPERTY(QString publishedAt READ publishedAt NOTIFY changed)
    Q_PROPERTY(QString lastChecked READ lastChecked NOTIFY changed)
    Q_PROPERTY(QString skippedVersion READ skippedVersion NOTIFY changed)
    Q_PROPERTY(QString actionError READ actionError NOTIFY changed)
    Q_PROPERTY(QString installHint READ installHint NOTIFY changed)
    Q_PROPERTY(bool ignored READ ignored NOTIFY changed)
    Q_PROPERTY(bool notifyAvailable READ notifyAvailable NOTIFY changed)
    Q_PROPERTY(QVariantList assets READ assets NOTIFY changed)
    Q_PROPERTY(int recommendedIndex READ recommendedIndex NOTIFY changed)
public:
    explicit UpdateService(QObject *parent = nullptr);
    ~UpdateService() override;
    bool automaticEnabled() const { return automaticEnabled_; }
    void setAutomaticEnabled(bool enabled);
    bool busy() const { return reply_ != nullptr; }
    QString status() const { return status_; }
    QString message() const { return message_; }
    QString currentVersion() const { return current_; }
    QString latestVersion() const { return release_ ? release_->parsed.canonical : QString{}; }
    QString notes() const { return release_ ? release_->notes : QString{}; }
    QString publishedAt() const { return release_ ? release_->published : QString{}; }
    QString lastChecked() const { return checkedAt_.toString(Qt::ISODate); }
    QString skippedVersion() const { return skipped_; }
    QString actionError() const { return actionError_; }
    QString installHint() const;
    bool ignored() const { return !latestVersion().isEmpty() && latestVersion() == skipped_; }
    bool notifyAvailable() const { return status_ == "available" && !ignored() && latestVersion() != snoozed_; }
    QVariantList assets() const { return release_ ? release_->assets.toVariantList() : QVariantList{}; }
    int recommendedIndex() const { return release_ ? release_->recommended : -1; }
    Q_INVOKABLE bool secureTransportAvailable() const { return QSslSocket::supportsSsl(); }
    Q_INVOKABLE void initialize();
    Q_INVOKABLE void startupCheck();
    Q_INVOKABLE void check(bool manual = true);
    Q_INVOKABLE void remindLater();
    Q_INVOKABLE bool skipVersion();
    Q_INVOKABLE bool clearSkippedVersion();
    Q_INVOKABLE bool openReleasePage();
    Q_INVOKABLE bool openAsset(int index);
#ifdef LIUSHENG_UPDATE_TEST
    UpdateService(const QUrl &endpoint, const QString &storageRoot, const QString &current,
                  const QString &platform, const QString &preferred, QObject *parent = nullptr);
    void setDeadlineForTest(int milliseconds) { deadlineMs_ = milliseconds; }
    void setNetworkBlockedForTest(bool blocked) { networkBlocked_ = blocked; }
    QUrl lastOpenedForTest;
#endif
signals:
    void changed();
    void automaticEnabledChanged();
    void finished();
private:
    void request(bool conditional);
    void readIncoming();
    void finishRequest();
    void fail(const QString &message);
    void acceptRelease(const updates::Release &release);
    bool saveCache();
    bool saveSkipped(const QString &version);
    bool openOfficial(const QUrl &url);
    QString cachePath_;
    QString preferencesPath_;
    QString current_;
    QString platform_;
    QString preferred_;
    QUrl endpoint_ = updates::Endpoint;
    bool automaticEnabled_ = true;
    bool initialized_ = false;
    bool startupAttempted_ = false;
    bool manual_ = false;
    bool retried304_ = false;
    bool networkBlocked_ = false;
    QString status_ = QStringLiteral("idle");
    QString message_ = QStringLiteral("尚未检查更新");
    QString actionError_;
    QString skipped_;
    QString snoozed_;
    QString abortReason_;
    QByteArray incoming_;
    QByteArray etag_;
    QJsonObject cachedRelease_;
    QDateTime checkedAt_;
    QDateTime retryAt_;
    QDateTime lastAttempt_;
    std::optional<updates::Release> release_;
    QNetworkAccessManager *network_ = nullptr;
    QPointer<QNetworkReply> reply_;
    QTimer deadline_;
    int deadlineMs_ = 10000;
};
} // namespace liusheng
