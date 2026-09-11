#pragma once
// Provider parsing and conservative candidate selection for local music metadata.
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QRegularExpression>
#include <QtCore/QSet>
#include <QtCore/QUrl>
#include <QtCore/QUrlQuery>
#include <QtCore/QCryptographicHash>
#include <algorithm>
#include <cmath>

namespace liusheng::online {
inline constexpr qsizetype JsonLimit = 2 * 1024 * 1024;
inline constexpr qsizetype ImageLimit = 8 * 1024 * 1024;
inline constexpr qsizetype LyricLimit = 1024 * 1024;
QString hash(const QByteArray &bytes);
bool key(const QString &value);
bool mbid(const QString &value);
QString normalized(const QString &text);
QString cleanSearchText(const QString &text);
QString baseTitle(const QString &text);
bool relatedTitle(const QString &candidate, const QString &wanted);
QJsonObject cleanQuery(QJsonObject query);
QList<QUrl> lookupPlan(const QJsonObject &query, const QString &kind);
QString lookupCacheKey(const QJsonObject &query, const QString &kind);
bool known(const QString &value);
QSet<QString> versions(const QString &text);
bool safeUrl(const QUrl &url, bool image);
bool validContext(const QJsonObject &context);
QString bindingKey(const QJsonObject &context, const QString &kind);
bool validLyrics(const QString &text, double duration = 0);
QUrl lyricsUrl(const QJsonObject &query);
QUrl coversUrl(const QJsonObject &query);
QJsonArray lyricsCandidates(const QJsonArray &input, const QJsonObject &query);
QJsonArray coverCandidates(const QJsonObject &input, const QJsonObject &query);
qint64 retryDelayMs(const QByteArray &header, const QByteArray &serverDate, int code, int attempt, qint64 now);
int automaticChoice(const QJsonArray &candidates);
} // namespace liusheng::online
