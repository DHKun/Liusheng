#pragma once
#include "storage.h"
#include "providers.h"
#include <QtCore/QElapsedTimer>
#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtCore/QTimer>
#include <QtNetwork/QNetworkAccessManager>
#include <QtNetwork/QNetworkReply>
#include <deque>
#include <functional>
#include <memory>
#include <optional>

namespace liusheng { class OnlineService; }
namespace liusheng::online {
struct Job {
    QJsonObject context;
    QJsonObject query;
    QString kind;
    QString mode;
};
struct SourceSearch {
    Job job;
    QStringList providers;
    int providerIndex = 0;
    int failed = 0;
    QJsonArray candidates;
    QVariantList reports;
    QElapsedTimer providerClock;
};
// Lives entirely on a private thread: network, JSON parsing, image decoding and I/O.
class Worker final : public QObject {
public:
    Worker(OnlineService *owner, QString root, bool blocked);
    void command(const QVariantMap &message);
    void shutdown();
#ifdef LIUSHENG_ONLINE_TEST
    QUrl testServer;
    int deadlineMs = 1000;
#else
    int deadlineMs = 15000;
#endif
private:
    using Reply = std::function<void(QByteArray, int, QString)>;
    OnlineService *owner_;
    Store store_;
    bool blocked_;
    bool ready_ = false;
    bool manualOpen_ = false;
    bool covers_ = false;
    bool lyrics_ = false;
    bool extras_ = false;
    std::shared_ptr<SourceSearch> multi_;
    bool batchPaused_ = true;
    bool stopped_ = false;
    QJsonObject context_;
    QString kind_ = "lyrics";
    QJsonArray candidates_;
    int selected_ = -1;
    QJsonObject batch_;
    std::deque<Job> automatic_;
    std::optional<Job> active_;
    QNetworkAccessManager *network_ = nullptr;
    QPointer<QNetworkReply> reply_;
    quint64 epoch_ = 0;
    QHash<QString, qint64> retryAt_;
    qint64 nextMusicBrainz_ = 0;
    qint64 nextLookup_ = 0;
    void initialize();
    void post(QVariantMap state);
    void notifyResource(const QJsonObject &binding);
    void batchState();
    void persistBatch();
    void normalizeBatch();
    void recordBatchResult(const Job &job, const QString &status, const QString &message);
    void retryState(const QString &host, qint64 until, int attempt, const QString &message);
    void abortRequest();
    void cancelActive();
    void pump();
    void lookup(Job job, bool fresh);
    void lookupAttempt(Job job, QString cacheKey, QList<QUrl> plan, int attempt);
    void receiveCandidates(QJsonArray candidates);
    void lookupMultiple(const Job &job);
    void nextProvider(const std::shared_ptr<SourceSearch> &run);
    void queryProvider(const std::shared_ptr<SourceSearch> &run, const QList<QUrl> &plan, int attempt);
    void completeProvider(const std::shared_ptr<SourceSearch> &run, QJsonArray candidates, const QString &error = {});
    void fetchProviderCover(QJsonObject candidate, bool saveImage);
    void fetchProviderLyrics(QJsonObject candidate, bool saveText);
    void fetchCover(QJsonObject candidate, bool save, bool groupFallback = false, int sizeAttempt = 0);
    void finish(const QString &status, const QString &message);
    void publishCandidates();
    void save(QJsonObject context, const QString &kind, QJsonObject candidate, QByteArray image, bool pinned);
    void request(QUrl url, bool image, Reply callback, int redirects = 0, qint64 started = 0, int retries = 0, bool failFast = false);
    void dispatch(const QVariantMap &message);
    void dispatchQueue(const QVariantMap &message);
};
}
