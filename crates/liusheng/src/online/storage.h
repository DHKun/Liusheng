#pragma once
#include "policy.h"
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QSaveFile>
#include <QtCore/QDateTime>

namespace liusheng::online {
class Store {
public:
    explicit Store(QString root) : root_(std::move(root)) {}
    void initialize();
    QJsonObject binding(const QJsonObject &context, const QString &kind) const;
    QJsonObject save(const QJsonObject &context, const QString &kind, QJsonObject candidate, const QByteArray &image, bool pinned);
    QJsonObject clear(const QJsonObject &context, const QString &kind);
    QString previewImage(const QByteArray &image);
    QJsonObject readJson(const QString &relative, qsizetype limit = JsonLimit) const;
    void writeJson(const QString &relative, const QJsonObject &value) const;
    void cache(const QString &queryKey, const QJsonArray &candidates) const;
    QJsonObject cached(const QString &queryKey) const;
    void clearCache() const;
    QString root() const { return root_; }
private:
    QString root_;
    mutable unsigned cacheWrites_ = 0;
    void pruneCache() const;
    void write(const QString &relative, const QByteArray &bytes) const;
};
struct ImageData { QByteArray jpeg; QString accent; };
ImageData normalizeImage(const QByteArray &bytes);
}
