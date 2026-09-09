#pragma once
// Pure release policy: no networking, filesystem writes or executable commands.
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QRegularExpression>
#include <QtCore/QSet>
#include <QtCore/QUrl>
#include <QtCore/QVersionNumber>
#include <optional>

namespace liusheng::updates {
inline constexpr qsizetype MaxResponse = 256 * 1024;
inline constexpr qsizetype MaxNotes = 24000;
inline const QString Repository = QStringLiteral("https://github.com/DHKun/Liusheng");
inline const QUrl Endpoint(QStringLiteral("https://api.github.com/repos/DHKun/Liusheng/releases/latest"));

struct Version {
    QVersionNumber number;
    QString prerelease;
    QString canonical;
};
inline std::optional<Version> version(QString text) {
    if (text.size() > 160) return std::nullopt;
    static const QRegularExpression pattern(QStringLiteral(
        "^v?(0|[1-9][0-9]{0,8})\\.(0|[1-9][0-9]{0,8})\\.(0|[1-9][0-9]{0,8})"
        "(?:-([0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*))?(?:\\+([0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*))?$"));
    const auto match = pattern.match(text);
    if (!match.hasMatch()) return std::nullopt;
    const auto pre = match.captured(4);
    static const QRegularExpression numeric(QStringLiteral("^[0-9]+$"));
    for (const auto &part : pre.split('.'))
        if (numeric.match(part).hasMatch() && part.size() > 1 && part.startsWith('0')) return std::nullopt;
    QVersionNumber number(match.captured(1).toInt(), match.captured(2).toInt(), match.captured(3).toInt());
    return Version{number, pre, number.toString()};
}
inline bool newerStable(const Version &remote, const Version &current) {
    const int comparison = QVersionNumber::compare(remote.number, current.number);
    return remote.prerelease.isEmpty() && (comparison > 0 || (comparison == 0 && !current.prerelease.isEmpty()));
}
inline bool officialUrl(const QUrl &url, const QString &path) {
    return url.isValid() && url.scheme() == QStringLiteral("https")
        && url.host().compare(QStringLiteral("github.com"), Qt::CaseInsensitive) == 0
        && url.userInfo().isEmpty() && url.port(-1) == -1
        && !url.hasQuery() && !url.hasFragment()
        && url.path(QUrl::FullyEncoded) == path;
}
inline QString assetKind(const QString &name, const QString &v) {
    if (name == QStringLiteral("liusheng_%1_amd64.deb").arg(v)) return QStringLiteral("deb");
    if (name == QStringLiteral("liusheng-%1-linux-x86_64.AppImage").arg(v)) return QStringLiteral("appimage");
    if (name == QStringLiteral("liusheng-%1-macos-arm64.zip").arg(v)) return QStringLiteral("macos");
    const QRegularExpression rpm(QStringLiteral("^liusheng-%1-[1-9][0-9]*(?:\\.fc[0-9]+)?\\.x86_64\\.rpm$").arg(QRegularExpression::escape(v)));
    if (rpm.match(name).hasMatch()) return QStringLiteral("rpm");
    return {};
}
inline bool compatible(const QString &kind, const QString &platform) {
    return (platform == QStringLiteral("linux-x86_64") && (kind == "deb" || kind == "rpm" || kind == "appimage"))
        || (platform == QStringLiteral("macos-arm64") && kind == "macos");
}
inline QString label(const QString &kind) {
    if (kind == "deb") return QStringLiteral("DEB · Linux x86_64");
    if (kind == "rpm") return QStringLiteral("RPM · Linux x86_64");
    if (kind == "appimage") return QStringLiteral("AppImage · Linux x86_64");
    return QStringLiteral("macOS · Apple Silicon arm64");
}
struct Release {
    QString tag;
    Version parsed;
    QString notes;
    QString published;
    QUrl page;
    QJsonArray assets;
    int recommended = -1;
};
inline std::optional<Release> parseRelease(const QJsonObject &json, const QString &platform, const QString &preferred) {
    if (!json.value("draft").isBool() || json.value("draft").toBool()
        || !json.value("prerelease").isBool() || json.value("prerelease").toBool()) return std::nullopt;
    const QString tag = json.value("tag_name").toString();
    const auto parsed = version(tag);
    if (!parsed || !parsed->prerelease.isEmpty()) return std::nullopt;
    const QString encodedTag = QString::fromLatin1(QUrl::toPercentEncoding(tag));
    const QUrl page(json.value("html_url").toString());
    if (!officialUrl(page, QStringLiteral("/DHKun/Liusheng/releases/tag/") + encodedTag)) return std::nullopt;
    if (!json.value("assets").isArray() || json.value("assets").toArray().size() > 128) return std::nullopt;
    Release release{tag, *parsed, json.value("body").toString().left(MaxNotes), json.value("published_at").toString().left(40), page, {}, -1};
    QSet<QString> seen;
    QSet<QString> duplicates;
    for (const auto &value : json.value("assets").toArray()) {
        const auto asset = value.toObject();
        const auto name = asset.value("name").toString();
        const auto kind = assetKind(name, parsed->canonical);
        const double size = asset.value("size").toDouble();
        if (!compatible(kind, platform) || asset.value("state").toString() != "uploaded" || size <= 0 || size > 16.0 * 1024 * 1024 * 1024) continue;
        const QUrl url(asset.value("browser_download_url").toString());
        if (!officialUrl(url, QStringLiteral("/DHKun/Liusheng/releases/download/") + encodedTag + '/' + name)) continue;
        if (seen.contains(kind)) duplicates.insert(kind);
        seen.insert(kind);
        QString digest = asset.value("digest").toString();
        static const QRegularExpression sha256(QStringLiteral("^sha256:[a-fA-F0-9]{64}$"));
        if (!sha256.match(digest).hasMatch()) digest.clear();
        release.assets.append(QJsonObject{{"name", name}, {"kind", kind}, {"label", label(kind)},
            {"url", url.toString()}, {"size", size}, {"digest", digest.toLower()}});
    }
    // Ambiguous files for the same package type require choosing on GitHub.
    QJsonArray unique;
    for (const auto &asset : release.assets)
        if (!duplicates.contains(asset.toObject().value("kind").toString())) unique.append(asset);
    release.assets = unique;
    for (qsizetype i = 0; i < release.assets.size(); ++i)
        if (release.assets[i].toObject().value("kind").toString() == preferred) release.recommended = int(i);
    if (release.recommended < 0 && !release.assets.isEmpty()) release.recommended = 0;
    return release;
}
} // namespace liusheng::updates
