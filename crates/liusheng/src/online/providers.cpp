#include "providers.h"
#include <QtCore/QStringDecoder>
#include <stdexcept>

namespace liusheng::online {
namespace {
bool numberId(const QString &id) { return QRegularExpression("^[1-9][0-9]{0,15}$").match(id).hasMatch(); }
bool qqId(const QString &id) { return QRegularExpression("^[A-Za-z0-9]{8,32}$").match(id).hasMatch(); }
QString numericId(const QJsonValue &value) {
    const qint64 id = value.toInteger(-1);
    return id > 0 && id <= 9007199254740991LL ? QString::number(id) : QString();
}
QString names(const QJsonArray &artists) {
    QStringList result;
    for (const auto &artist : artists) {
        const auto name = artist.toObject()["name"].toString();
        if (!name.isEmpty()) result.append(name);
        if (result.size() >= 16) break;
    }
    return result.join(" / ");
}
QUrl queryUrl(const QString &base, const QList<QPair<QString, QString>> &parameters) {
    QUrl url(base); QUrlQuery query;
    for (const auto &pair : parameters) query.addQueryItem(pair.first, pair.second);
    url.setQuery(query); return url;
}
void checkCode(const QString &provider, const QJsonObject &document) {
    if (provider == "deezer") {
        if (document.contains("error") || !document["data"].isArray()) throw std::runtime_error("Deezer 来源响应无效");
    } else {
        const int expected = provider == "netease" ? 200 : 0;
        if (!document["code"].isDouble() || document["code"].toInt(-1) != expected)
            throw std::runtime_error((providerName(provider) + QStringLiteral(" 暂时不可用")).toUtf8().constData());
    }
}
QString decodeLyrics(const QString &encoded) {
    if (encoded.toUtf8().size() > 2 * LyricLimit) throw std::runtime_error("歌词超过大小限制");
    const auto decoded = QByteArray::fromBase64Encoding(encoded.toLatin1(), QByteArray::AbortOnBase64DecodingErrors);
    if (!decoded) throw std::runtime_error("歌词编码无效");
    QStringDecoder utf8(QStringDecoder::Utf8); const QString text = utf8(decoded.decoded);
    if (utf8.hasError()) throw std::runtime_error("歌词编码无效");
    return text;
}
}
QString providerName(const QString &provider) {
    if (provider == "netease") return QStringLiteral("网易云音乐");
    if (provider == "qqmusic") return QStringLiteral("QQ 音乐");
    if (provider == "deezer") return "Deezer";
    return provider == "lrclib" ? "LRCLIB" : "MusicBrainz / Cover Art Archive";
}
QString sourceSelection(const QString &source, const QString &kind) {
    if (QStringList{"all", "primary", "netease", "qqmusic"}.contains(source)) return source;
    if (source == "deezer" && kind == "cover") return source;
    return "primary";
}
QStringList providerOrder(const QString &selection, const QString &kind, const QJsonObject &query) {
    const auto primary = kind == "lyrics" ? QStringLiteral("lrclib") : QStringLiteral("musicbrainz");
    if (selection == "primary") return {primary};
    if (selection != "all") return {sourceSelection(selection, kind)};
    const bool cjk = QRegularExpression(QStringLiteral("[\\x{3000}-\\x{9fff}]")).match(query["title"].toString() + query["artist"].toString()).hasMatch();
    QStringList sources = cjk ? QStringList{"qqmusic", "netease"} : QStringList{primary, "qqmusic", "netease"};
    if (kind == "cover") sources.append("deezer");
    if (!sources.contains(primary)) sources.append(primary);
    return sources;
}
QList<QUrl> providerQueries(const QString &provider, const QString &kind, const QJsonObject &original) {
    const auto query = cleanQuery(original);
    if (provider == "lrclib" || provider == "musicbrainz") return lookupPlan(query, kind);
    const auto title = baseTitle(query["title"].toString());
    const auto album = query["album"].toString();
    const auto artist = query["artist"].toString();
    const auto term = (known(artist) ? artist + ' ' : QString()) + (known(title) ? title : album);
    if (provider == "netease") return {queryUrl("https://music.163.com/api/search/get", {{"s", term}, {"type", "1"}, {"limit", "20"}, {"offset", "0"}})};
    if (provider == "qqmusic") {
        QList<QUrl> urls{queryUrl("https://c.y.qq.com/soso/fcgi-bin/client_search_cp", {{"w", term}, {"format", "json"}, {"n", "20"}, {"p", "1"}, {"inCharset", "utf-8"}, {"outCharset", "utf-8"}})};
        if (kind == "lyrics") urls.append(queryUrl("https://c.y.qq.com/splcloud/fcgi-bin/smartbox_new.fcg", {{"key", term}, {"format", "json"}, {"inCharset", "utf-8"}, {"outCharset", "utf-8"}}));
        return urls;
    }
    return {queryUrl("https://api.deezer.com/search/album", {{"q", (known(artist) ? artist + ' ' : QString()) + (known(album) ? baseTitle(album) : title)}, {"limit", "10"}, {"output", "json"}})};
}
bool safeProviderUrl(const QUrl &url, bool image) {
    const auto host = url.host().toLower(), path = url.path();
    if (image) {
        return (QRegularExpression("^p[1-9][0-9]?\\.music\\.126\\.net$").match(host).hasMatch() && path.endsWith(".jpg", Qt::CaseInsensitive))
            || (host == "y.gtimg.cn" && QRegularExpression("^/music/photo_new/T002R[0-9]+x[0-9]+M000[A-Za-z0-9]{8,32}(?:_[0-9])?\\.jpg$").match(path).hasMatch())
            || (host == "cdn-images.dzcdn.net" && path.startsWith("/images/cover/") && path.endsWith(".jpg"));
    }
    return (host == "music.163.com" && QStringList{"/api/search/get", "/api/song/lyric", "/api/song/detail/"}.contains(path))
        || (host == "c.y.qq.com" && QStringList{"/soso/fcgi-bin/client_search_cp", "/splcloud/fcgi-bin/smartbox_new.fcg", "/lyric/fcgi-bin/fcg_query_lyric_new.fcg"}.contains(path))
        || (host == "api.deezer.com" && path == "/search/album");
}
bool safeSourcePage(const QUrl &url) {
    if (!url.isValid() || url.scheme() != "https" || !url.userInfo().isEmpty() || url.hasFragment() || (url.port() != -1 && url.port() != 443)) return false;
    const auto host = url.host(), path = url.path();
    return safeUrl(url, false)
        || (host == "music.163.com" && path == "/song" && numberId(QUrlQuery(url).queryItemValue("id")))
        || (host == "y.qq.com" && path.startsWith("/n/ryqq/songDetail/") && qqId(path.section('/', -1)))
        || (host == "www.deezer.com" && path.startsWith("/album/") && numberId(path.section('/', -1)));
}
QUrl trustedImageUrl(const QString &provider, QUrl url) {
    if (url.scheme() == "http" && safeProviderUrl(url, true)) url.setScheme("https");
    if (!safeUrl(url, true) || url.toString().size() > 2048) return {};
    const auto host = url.host();
    if (provider == "netease" && host.endsWith(".music.126.net")) return url;
    if (provider == "qqmusic" && host == "y.gtimg.cn") return url;
    if (provider == "deezer" && host == "cdn-images.dzcdn.net") return url;
    return {};
}
QJsonArray mergeCandidates(const QJsonArray &left, const QJsonArray &right) {
    QList<QJsonObject> rows; QSet<QString> seen;
    for (const auto &array : {left, right}) for (const auto &value : array) {
        auto row = value.toObject();
        const auto id = row["provider"].toString() + ':' + row["id"].toString();
        if (seen.contains(id)) continue;
        seen.insert(id); rows.append(row);
    }
    std::stable_sort(rows.begin(), rows.end(), [](const auto &a, const auto &b) { return a["score"].toInt() > b["score"].toInt(); });
    QJsonArray result; for (const auto &row : rows) { if (result.size() >= 40) break; result.append(row); } return result;
}
QJsonArray catalogCandidates(const QString &provider, const QString &kind, const QJsonObject &document, const QJsonObject &original) {
    checkCode(provider, document);
    const auto query = cleanQuery(original);
    QJsonArray entries;
    if (provider == "netease") {
        const auto result = document["result"].toObject();
        if (!document["result"].isObject()) throw std::runtime_error("网易云音乐 返回格式无效");
        if (!result["songs"].isArray() && result["songCount"].toInt(-1) != 0) throw std::runtime_error("网易云音乐 返回格式无效");
        entries = result["songs"].toArray();
    } else if (provider == "qqmusic") {
        const auto song = document["data"].toObject()["song"].toObject();
        if (!song["list"].isArray() && !song["itemlist"].isArray()) throw std::runtime_error("QQ 音乐 返回格式无效");
        entries = song.contains("list") ? song["list"].toArray() : song["itemlist"].toArray();
    } else entries = document["data"].toArray();
    QJsonArray candidates;
    qsizetype inspected = 0;
    for (const auto &value : entries) {
        if (++inspected > 2000) break;
        const auto entry = value.toObject();
        QString id, title, artist, album, albumId, image; double duration = 0;
        if (provider == "netease") {
            id = numericId(entry["id"]); title = entry["name"].toString(); artist = names(entry["artists"].toArray());
            const auto record = entry["album"].toObject(); album = record["name"].toString(); albumId = numericId(record["id"]);
            image = trustedImageUrl(provider, QUrl(record["picUrl"].toString())).toString(); duration = entry["duration"].toDouble() / 1000.0;
        } else if (provider == "qqmusic") {
            id = entry.value("songmid").toString(entry["mid"].toString()); if (!qqId(id)) continue;
            title = entry.value("songname").toString(entry["name"].toString());
            artist = entry["singer"].isString() ? entry["singer"].toString() : names(entry["singer"].toArray());
            album = entry["albumname"].toString(); albumId = entry["albummid"].toString(); duration = entry["interval"].toDouble();
            if (qqId(albumId)) image = "https://y.gtimg.cn/music/photo_new/T002R500x500M000" + albumId + ".jpg";
        } else {
            id = numericId(entry["id"]); title = album = entry["title"].toString(); albumId = id;
            artist = entry["artist"].toObject()["name"].toString();
            image = trustedImageUrl(provider, QUrl(entry["cover_big"].toString())).toString();
        }
        if (id.isEmpty() || title.isEmpty() || title.size() > 512 || artist.size() > 512 || album.size() > 512 || !std::isfinite(duration) || duration < 0) continue;
        const bool titleMatch = normalized(title) == normalized(query["title"].toString()) && known(title);
        const bool related = relatedTitle(title, query["title"].toString());
        const bool albumMatch = known(album) && known(query["album"].toString()) && normalized(album) == normalized(query["album"].toString());
        if (!related && !albumMatch && provider != "deezer") continue;
        if (kind == "cover" && (albumId.isEmpty() || (provider != "netease" && image.isEmpty()))) continue;
        const bool artistMatch = known(artist) && normalized(artist) == normalized(query["artist"].toString());
        const bool versionMatch = versions(title + ' ' + album) == versions(query["title"].toString() + ' ' + query["album"].toString());
        const double delta = duration > 0 && query["duration"].toDouble() > 0 ? std::abs(duration - query["duration"].toDouble()) : -1;
        const bool certain = entries.size() <= 2000 && titleMatch && artistMatch && versionMatch && delta >= 0 && delta <= 2 && (!known(query["album"].toString()) || albumMatch);
        const int score = (titleMatch ? 35 : related ? 20 : 0) + (artistMatch ? 30 : 0) + (albumMatch ? 15 : 0) + (delta >= 0 && delta <= 2 ? 15 : 0);
        QString note;
        if (!artistMatch) note = QStringLiteral("歌手名称不同，请核对");
        else if (!versionMatch || !titleMatch) note = QStringLiteral("请核对演出版本与时间轴");
        else if (known(query["album"].toString()) && !albumMatch) note = QStringLiteral("专辑信息不同，请核对");
        const auto sourceUrl = provider == "netease" ? "https://music.163.com/song?id=" + id : provider == "qqmusic" ? "https://y.qq.com/n/ryqq/songDetail/" + id : "https://www.deezer.com/album/" + id;
        candidates.append(QJsonObject{{"id", id}, {"providerId", provider}, {"provider", providerName(provider)}, {"sourceUrl", sourceUrl},
            {"title", title}, {"artist", artist}, {"album", album}, {"albumId", albumId}, {"imageUrl", image}, {"duration", duration}, {"delta", delta},
            {"score", score}, {"confident", certain}, {"manualOnly", !certain}, {"note", note}, {"versionMatch", versionMatch}, {"artistMatch", artistMatch},
            {"lyricsPending", kind == "lyrics"}, {"synced", false}, {"instrumental", false}});
    }
    QJsonArray result;
    for (const auto &candidate : mergeCandidates({}, candidates)) { if (result.size() >= 20) break; result.append(candidate); }
    return result;
}
QUrl providerLyricUrl(const QJsonObject &candidate) {
    const auto provider = candidate["providerId"].toString(), id = candidate["id"].toString();
    if (provider == "netease" && numberId(id)) return queryUrl("https://music.163.com/api/song/lyric", {{"id", id}, {"lv", "-1"}, {"kv", "-1"}, {"tv", "-1"}, {"os", "pc"}});
    if (provider == "qqmusic" && qqId(id)) return queryUrl("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg", {{"songmid", id}, {"g_tk", "5381"}, {"format", "json"}, {"inCharset", "utf-8"}, {"outCharset", "utf-8"}});
    return {};
}
QJsonObject readProviderLyrics(QJsonObject candidate, const QJsonObject &document) {
    const auto provider = candidate["providerId"].toString(); checkCode(provider, document);
    const bool instrumental = provider == "netease" && document["nolyric"].toBool();
    QString text, translation;
    if (!instrumental) {
        text = provider == "netease" ? document["lrc"].toObject()["lyric"].toString() : decodeLyrics(document["lyric"].toString());
        translation = provider == "netease" ? document["tlyric"].toObject()["lyric"].toString() : decodeLyrics(document["trans"].toString());
        if (!validLyrics(text, candidate["duration"].toDouble())) throw std::runtime_error("此条目暂无可用歌词");
        if (!translation.isEmpty() && validLyrics(translation, candidate["duration"].toDouble()) && translation.trimmed() != text.trimmed()) text += '\n' + translation;
        if (!validLyrics(text, candidate["duration"].toDouble())) throw std::runtime_error("歌词超过大小限制");
    }
    candidate["text"] = text; candidate["instrumental"] = instrumental; candidate["lyricsPending"] = false;
    candidate["synced"] = QRegularExpression("\\[\\d{1,3}:\\d{2}").match(text).hasMatch();
    return candidate;
}
QUrl neteaseDetailUrl(const QJsonObject &candidate) {
    const auto id = candidate["id"].toString();
    return numberId(id) ? queryUrl("https://music.163.com/api/song/detail/", {{"id", id}, {"ids", '[' + id + ']'}}) : QUrl();
}
QUrl neteaseCoverUrl(const QJsonObject &candidate, const QJsonObject &document) {
    checkCode("netease", document);
    for (const auto &value : document["songs"].toArray()) {
        const auto song = value.toObject();
        if (numericId(song["id"]) != candidate["id"].toString()) continue;
        const auto album = song["album"].toObject();
        if (numericId(album["id"]) != candidate["albumId"].toString()) continue;
        return trustedImageUrl("netease", QUrl(album["picUrl"].toString()));
    }
    return {};
}
}
