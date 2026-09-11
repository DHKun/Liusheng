#pragma once
#include "policy.h"
#include <QtCore/QStringList>

namespace liusheng::online {
QString providerName(const QString &provider);
QString sourceSelection(const QString &source, const QString &kind);
QStringList providerOrder(const QString &selection, const QString &kind, const QJsonObject &query);
QList<QUrl> providerQueries(const QString &provider, const QString &kind, const QJsonObject &query);
QJsonArray catalogCandidates(const QString &provider, const QString &kind, const QJsonObject &document, const QJsonObject &query);
QUrl providerLyricUrl(const QJsonObject &candidate);
QJsonObject readProviderLyrics(QJsonObject candidate, const QJsonObject &document);
QUrl neteaseDetailUrl(const QJsonObject &candidate);
QUrl neteaseCoverUrl(const QJsonObject &candidate, const QJsonObject &document);
QUrl trustedImageUrl(const QString &provider, QUrl url);
bool safeProviderUrl(const QUrl &url, bool image);
bool safeSourcePage(const QUrl &url);
QJsonArray mergeCandidates(const QJsonArray &left, const QJsonArray &right);
}
