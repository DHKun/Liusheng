#include "policy.h"
#include "providers.h"
#include <QtCore/QDateTime>

namespace liusheng::online {
qint64 retryDelayMs(const QByteArray &header, const QByteArray &serverDate, int code, int attempt, qint64 now) {
    const qint64 fallback = code == 429 ? 60000 : (2000LL << std::clamp(attempt, 0, 5));
    const auto value = header.trimmed();
    bool ok = false;
    const auto seconds = value.toLongLong(&ok);
    if (ok && seconds >= 0) return std::clamp<qint64>(seconds, 1, 86400) * 1000;
    const auto httpDate = [](QByteArray text) {
        text = text.trimmed();
        // HTTP uses the literal GMT timezone; Qt's RFC2822 parser expects a
        // numeric offset. Normalize it while preserving the UTC instant.
        if (text.endsWith(" GMT")) text = text.chopped(4) + " +0000";
        return QDateTime::fromString(QString::fromLatin1(text), Qt::RFC2822Date);
    };
    const auto until = httpDate(value);
    if (until.isValid()) {
        const auto date = httpDate(serverDate);
        const qint64 origin = date.isValid() ? date.toMSecsSinceEpoch() : now;
        return std::clamp<qint64>(until.toMSecsSinceEpoch() - origin, 1000, 86400000);
    }
    return fallback;
}
QString hash(const QByteArray &bytes) {
    return QString::fromLatin1(QCryptographicHash::hash(bytes, QCryptographicHash::Sha256).toHex());
}
bool key(const QString &value) {
    return QRegularExpression("^[a-f0-9]{64}$").match(value).hasMatch();
}
bool mbid(const QString &value) {
    return QRegularExpression("^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$").match(value).hasMatch();
}
QString normalized(const QString &text) {
    QString result;
    for (const auto character : text.normalized(QString::NormalizationForm_KC).toCaseFolded()) {
        if (character.isLetterOrNumber()) result.append(character);
    }
    return result;
}
bool known(const QString &value) {
    const auto name = normalized(value);
    static const QSet<QString> unknown = {QString(), QStringLiteral("unknown"), QStringLiteral("unknownartist"), QStringLiteral("unknownalbum"), QStringLiteral("未知专辑"), QStringLiteral("未知艺术家")};
    return !unknown.contains(name);
}
QSet<QString> versions(const QString &text) {
    const auto value = text.normalized(QString::NormalizationForm_KC).toCaseFolded();
    static const QList<QPair<QString, QString>> patterns = {
        {"live", "\\blive\\b|现场|現場|演唱会|演唱會|ライブ"},
        {"remix", "\\bremix\\b|混音"},
        {"acoustic", "\\bacoustic\\b|unplugged|不插电|不插電"},
        {"instrumental", "\\binstrumental\\b|伴奏|off[ -]vocal"},
        {"short", "tv[ -]?(size|version)|radio[ -]?edit|short[ -]?(version|edit)|剪辑版|剪輯版"},
        {"extended", "\\bextended\\b|long[ -]?version"},
        {"remaster", "\\bremaster(ed)?\\b|重制|重製"},
        {"jazz", "\\bjazz\\b|爵士"},
        {"piano", "\\bpiano\\b|钢琴版|鋼琴版"}
    };
    QSet<QString> result;
    for (const auto &pattern : patterns)
        if (QRegularExpression(pattern.second).match(value).hasMatch()) result.insert(pattern.first);
    return result;
}
namespace {
const QRegularExpression &bracketPattern() {
    static const QRegularExpression pattern(QStringLiteral(R"(\([^()]*\)|\[[^\[\]]*\]|【[^【】]*】)"));
    return pattern;
}
QString stripSearchBrackets(QString text, bool editions) {
    static const QRegularExpression advertising(QStringLiteral(R"((?:https?://|www\.)|[a-z0-9][a-z0-9.-]*\.(?:com|cn|net|org|cc|top|xyz|info)\b)"), QRegularExpression::CaseInsensitiveOption);
    static const QRegularExpression quality(QStringLiteral(R"(^\s*(?:(?:mp3|flac|wav|ape|aac|320\s*k(?:bps)?|无损|無損|高音质|高音質|高品质|高品質|下载|下載)\s*[-_·|/]?\s*)+$)"), QRegularExpression::CaseInsensitiveOption);
    auto matches = bracketPattern().globalMatch(text);
    QList<QPair<qsizetype, qsizetype>> removed;
    while (matches.hasNext()) {
        const auto match = matches.next();
        const auto inside = match.captured().mid(1, match.capturedLength() - 2);
        const bool version = !versions(inside).isEmpty();
        if ((editions && version) || (!version && (advertising.match(inside).hasMatch() || quality.match(inside).hasMatch())))
            removed.prepend({match.capturedStart(), match.capturedLength()});
    }
    for (const auto &span : removed) text.remove(span.first, span.second);
    return text.simplified();
}
}
QString cleanSearchText(const QString &text) {
    // Query-only cleanup preserves all original audio tags, identities and edition markers.
    return stripSearchBrackets(text.normalized(QString::NormalizationForm_KC), false).left(512);
}
QString baseTitle(const QString &text) {
    auto base = stripSearchBrackets(cleanSearchText(text), true);
    static const QRegularExpression suffix(QStringLiteral(R"(\s+[-–—]\s+(?:live|jazz(?:\s+version)?|acoustic|instrumental|remix|remastered?(?:\s+\d{4})?|radio edit|tv size|piano(?:\s+version)?)\s*$)"), QRegularExpression::CaseInsensitiveOption);
    base.remove(suffix);
    return base.simplified();
}
bool relatedTitle(const QString &candidate, const QString &wanted) {
    const auto target = normalized(baseTitle(wanted));
    if (target.isEmpty()) return false;
    if (normalized(baseTitle(candidate)) == target) return true;
    auto matches = bracketPattern().globalMatch(cleanSearchText(candidate));
    while (matches.hasNext()) {
        const auto part = matches.next().captured();
        if (normalized(part.mid(1, part.size() - 2)) == target) return true;
    }
    return false;
}
QJsonObject cleanQuery(QJsonObject query) {
    for (const auto &field : {"title", "artist", "album", "albumArtist"}) query[field] = cleanSearchText(query[field].toString());
    return query;
}
bool safeUrl(const QUrl &url, bool image) {
    if (!url.isValid() || url.scheme() != "https" || !url.userInfo().isEmpty() || url.hasFragment()) return false;
    if (url.port(-1) != -1 && url.port() != 443) return false;
    const auto host = url.host().toLower();
    if (safeProviderUrl(url, image)) return true;
    if (!image) return host == "lrclib.net" || host == "musicbrainz.org";
    return host == "coverartarchive.org" || host == "archive.org" || host.endsWith(".archive.org");
}
bool validContext(const QJsonObject &context) {
    return key(context["trackKey"].toString()) && key(context["identity"].toString())
        && (context["albumKey"].toString().isEmpty() || key(context["albumKey"].toString()))
        && context["path"].toString().startsWith('/') && context["path"].toString().size() <= 16384
        && context["title"].toString().size() <= 512 && context["artist"].toString().size() <= 512
        && context["album"].toString().size() <= 512;
}
QString bindingKey(const QJsonObject &context, const QString &kind) {
    if (kind == "cover" && context["scope"].toString() == "album" && key(context["albumKey"].toString()))
        return context["albumKey"].toString();
    return context["trackKey"].toString();
}
int automaticChoice(const QJsonArray &candidates) {
    if (candidates.isEmpty() || !candidates.first().toObject()["confident"].toBool()) return -1;
    for (qsizetype i = 1; i < candidates.size(); ++i)
        if (candidates[i].toObject()["confident"].toBool()) return -1;
    return 0;
}
bool validLyrics(const QString &text, double duration) {
    if (text.trimmed().isEmpty() || text.toUtf8().size() > LyricLimit || text.contains(QChar(0))) return false;
    const auto lines = text.split('\n');
    if (lines.size() > 10000) return false;
    const QRegularExpression stamp("\\[(\\d{1,3}):(\\d{2})(?:[.:](\\d{1,3}))?\\]");
    int expanded = 0;
    qsizetype bytes = 0;
    bool content = false;
    for (const auto &line : lines) {
        if (line.toUtf8().size() > 8192) return false;
        auto matches = stamp.globalMatch(line);
        int count = 0;
        while (matches.hasNext()) {
            const auto match = matches.next();
            if (match.captured(2).toInt() >= 60) return false;
            double seconds = match.captured(1).toInt() * 60 + match.captured(2).toInt();
            if (!match.captured(3).isEmpty()) seconds += match.captured(3).toDouble() / std::pow(10, match.captured(3).size());
            if (duration > 0 && seconds > duration + 30) return false;
            ++count;
        }
        expanded += std::max(1, count);
        bytes += line.toUtf8().size() * std::max(1, count);
        if (expanded > 10000 || bytes > 2 * LyricLimit) return false;
        QString remainder = line;
        remainder.remove(stamp);
        if (!remainder.trimmed().isEmpty() && !QRegularExpression("^\\[[a-zA-Z]+:.*\\]$").match(remainder.trimmed()).hasMatch()) content = true;
    }
    return content;
}
static QString quoted(QString value) {
    value.replace('\\', "\\\\");
    value.replace('"', "\\\"");
    return '"' + value + '"';
}
QUrl lyricsUrl(const QJsonObject &query) {
    QUrl url("https://lrclib.net/api/search");
    QUrlQuery parameters;
    if (known(query["artist"].toString())) {
        parameters.addQueryItem("track_name", query["title"].toString());
        parameters.addQueryItem("artist_name", query["artist"].toString());
    } else parameters.addQueryItem("q", query["title"].toString());
    url.setQuery(parameters);
    return url;
}
QUrl coversUrl(const QJsonObject &query) {
    const bool album = known(query["album"].toString());
    QUrl url(album ? "https://musicbrainz.org/ws/2/release/" : "https://musicbrainz.org/ws/2/recording/");
    QUrlQuery parameters;
    QString term = (album ? "release:" : "recording:") + quoted(query[album ? "album" : "title"].toString());
    const auto artist = album && known(query["albumArtist"].toString()) ? query["albumArtist"].toString() : query["artist"].toString();
    if (known(artist)) term += " AND artist:" + quoted(artist);
    parameters.addQueryItem("query", term);
    parameters.addQueryItem("fmt", "json");
    parameters.addQueryItem("limit", "20");
    url.setQuery(parameters);
    return url;
}
QList<QUrl> lookupPlan(const QJsonObject &original, const QString &kind) {
    const auto query = cleanQuery(original);
    QList<QUrl> plan;
    auto append = [&plan](const QUrl &url) { if (!plan.contains(url)) plan.append(url); };
    const auto title = query["title"].toString();
    const auto base = baseTitle(title);
    if (kind == "lyrics") {
        append(lyricsUrl(query));
        auto simpler = query;
        if (known(base) && base != title) { simpler["title"] = base; append(lyricsUrl(simpler)); }
        simpler = query; simpler["artist"] = "";
        append(lyricsUrl(simpler));
        if (known(base)) { simpler["title"] = base; append(lyricsUrl(simpler)); }
    } else {
        // Downloaded singles often copy the song title (including Live/Remix)
        // into the album tag. Resolve the recording first in this specific case
        // so an invented release title does not block the useful search route.
        const auto album = query["album"].toString();
        if (known(base) && !versions(title).isEmpty() && normalized(baseTitle(album)) == normalized(base)) {
            auto recording = query;
            recording["album"] = "";
            recording["title"] = base;
            append(coversUrl(recording));
        }
        append(coversUrl(query));
        auto addBrainz = [&append](const QString &entity, const QString &field, const QString &name, const QString &artist, bool aliases) {
            QUrl url("https://musicbrainz.org/ws/2/" + entity + '/');
            QString term = field + ':' + quoted(name);
            if (aliases) term = '(' + term + " OR alias:" + quoted(name) + ')';
            if (known(artist)) term += " AND artist:" + quoted(artist);
            QUrlQuery parameters;
            parameters.addQueryItem("query", term);
            parameters.addQueryItem("fmt", "json");
            parameters.addQueryItem("limit", "20");
            url.setQuery(parameters);
            append(url);
        };
        if (known(album)) addBrainz("release", "release", album, {}, true);
        if (known(base)) {
            addBrainz("recording", "recording", base, query["artist"].toString(), false);
            addBrainz("recording", "recording", base, {}, true);
        }
        if (known(album)) addBrainz("release-group", "releasegroup", album, {}, true);
    }
    return plan;
}
QString lookupCacheKey(const QJsonObject &original, const QString &kind) {
    const auto query = cleanQuery(original);
    QJsonObject fields;
    for (const auto &field : {"title", "artist", "album", "albumArtist"}) fields[field] = query[field];
    if (kind == "lyrics") fields["duration"] = query["duration"];
    fields["source"] = sourceSelection(query["_source"].toString(), kind);
    // Invalidate previous negative searches while preserving selected resources.
    return hash("matcher-v5\n" + kind.toUtf8() + '\n' + QJsonDocument(fields).toJson(QJsonDocument::Compact));
}
} // namespace liusheng::online
