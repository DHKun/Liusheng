#pragma once
#include <QtCore/QObject>
#include <QtCore/QThread>
#include <QtCore/QUrl>
#include <QtCore/QVariantMap>
#include <QtQml/qqmlregistration.h>

namespace liusheng {
namespace online { class Worker; }
class OnlineService : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QVariantMap state READ state NOTIFY changed)
    Q_PROPERTY(QString storageRoot READ storageRoot WRITE setStorageRoot NOTIFY storageRootChanged)
    Q_PROPERTY(bool autoCovers READ autoCovers WRITE setAutoCovers NOTIFY preferencesChanged)
    Q_PROPERTY(bool autoLyrics READ autoLyrics WRITE setAutoLyrics NOTIFY preferencesChanged)
    Q_PROPERTY(bool autoExtraSources READ autoExtraSources WRITE setAutoExtraSources NOTIFY preferencesChanged)
public:
    explicit OnlineService(QObject *parent = nullptr);
    ~OnlineService() override;
    QVariantMap state() const { return state_; }
    QString storageRoot() const { return root_; }
    void setStorageRoot(const QString &root);
    bool autoCovers() const { return covers_; }
    bool autoLyrics() const { return lyrics_; }
    void setAutoCovers(bool value);
    void setAutoLyrics(bool value);
    bool autoExtraSources() const { return extras_; }
    void setAutoExtraSources(bool value);
    Q_INVOKABLE void open(const QString &contextJson, const QString &kind);
    Q_INVOKABLE void close();
    Q_INVOKABLE void search(const QString &title, const QString &artist, const QString &album);
    Q_INVOKABLE void searchWithSource(const QString &title, const QString &artist, const QString &album, const QString &source);
    Q_INVOKABLE void startBatchWithSource(const QString &contextsJson, const QString &kind, const QString &source);
    Q_INVOKABLE void preview(int index);
    Q_INVOKABLE void choose(bool albumScope);
    Q_INVOKABLE void restoreDefault(bool albumScope);
    Q_INVOKABLE void importLocal(const QUrl &file, bool albumScope);
    Q_INVOKABLE void completeLyricImport(const QString &requestId, const QString &text, bool wordTimed, const QString &error);
    Q_INVOKABLE void cancel();
    Q_INVOKABLE void requestAutomatic(const QString &contextsJson);
    Q_INVOKABLE void startBatch(const QString &contextsJson, const QString &kind);
    Q_INVOKABLE void inspectBatch();
    Q_INVOKABLE void pauseBatch();
    Q_INVOKABLE void resumeBatch();
    Q_INVOKABLE void retryBatchFailures();
    Q_INVOKABLE void cancelBatch();
    Q_INVOKABLE void clearSearchCache();
    Q_INVOKABLE void openSource();
#ifdef LIUSHENG_ONLINE_TEST
    void configureTest(const QUrl &server, int deadlineMs = 1000);
#endif
signals:
    void changed();
    void storageRootChanged();
    void preferencesChanged();
    void resourceChanged(const QString &key, const QString &kind);
    void lyricImportRequested(const QString &requestId, const QString &fileUrl);
    void applied(const QString &identity, const QString &key, const QString &kind, const QString &message);
private:
    friend class online::Worker;
    void dispatch(QVariantMap command);
    void receive(const QVariantMap &state);
    QVariantMap state_;
    QString root_;
    QThread thread_;
    online::Worker *worker_ = nullptr;
    bool covers_ = false;
    bool lyrics_ = false;
    bool extras_ = false;
    bool blocked_ = false;
    QString pendingImport_;
    QVariantMap importContext_;
#ifdef LIUSHENG_ONLINE_TEST
    QUrl testServer_;
    int testDeadline_ = 1000;
#endif
};
}
