#include "policy.h"

namespace liusheng::online {
QJsonArray lyricsCandidates(const QJsonArray &input, const QJsonObject &original) {
    const auto query = cleanQuery(original);
    QList<QJsonObject> items;
    QSet<QString> seen;
    for (const auto &value : input) {
        if (items.size() >= 2000) break;
        const auto source = value.toObject();
        const auto id = source["id"].toInteger(-1);
        if (id < 0 || id > 9007199254740991LL) continue;
        const auto title = source["trackName"].toString();
        const auto artist = source["artistName"].toString();
        const auto album = source["albumName"].toString();
        if (title.isEmpty() || title.size() > 512 || artist.size() > 512 || album.size() > 512) continue;
        const double duration = source["duration"].toDouble(-1);
        const double localDuration = query["duration"].toDouble();
        if (!std::isfinite(duration) || duration < 0) continue;
        QString text = source["syncedLyrics"].toString();
        bool synced = !text.isEmpty() && QRegularExpression("\\[\\d{1,3}:\\d{2}").match(text).hasMatch() && validLyrics(text, duration);
        if (!synced) text = source["plainLyrics"].toString();
        const bool instrumental = source["instrumental"].toBool();
        if (!instrumental && !validLyrics(text, duration)) continue;
        if (instrumental) { text.clear(); synced = false; }
        const QString dedupe = hash((normalized(title) + "\n" + normalized(artist) + "\n" + normalized(album) + "\n" + QString::number(duration) + "\n" + text + QString::number(instrumental)).toUtf8());
        if (seen.contains(dedupe)) continue;
        seen.insert(dedupe);
        const bool titleMatch = known(query["title"].toString()) && normalized(title) == normalized(query["title"].toString());
        const bool artistMatch = known(query["artist"].toString()) && normalized(artist) == normalized(query["artist"].toString());
        const bool albumMatch = known(query["album"].toString()) && normalized(album) == normalized(query["album"].toString());
        const double delta = std::abs(localDuration - duration);
        const bool versionMatch = versions(title + " " + album) == versions(query["title"].toString() + " " + query["album"].toString());
        const bool durationMatch = localDuration > 0 && delta <= 2;
        const bool confident = input.size() <= 2000 && titleMatch && artistMatch && durationMatch && versionMatch && (!known(query["album"].toString()) || albumMatch);
        const int score = (titleMatch ? 35 : relatedTitle(title, query["title"].toString()) ? 20 : 0) + (artistMatch ? 30 : 0) + (albumMatch ? 15 : 0) + (durationMatch ? 15 : 0) + (synced ? 5 : 0);
        QString note = confident ? QStringLiteral("歌名、歌手、版本与时长匹配") : QStringLiteral("请核对演唱版本与时间轴");
        if (!versionMatch) note = QStringLiteral("版本标记不同，请确认 Live、Jazz 等版本及歌词时间轴");
        if (!artistMatch && known(query["artist"].toString())) note += QStringLiteral(" · 演唱者名称不同");
        items.append({{"id", QString::number(id)}, {"title", title}, {"artist", artist}, {"album", album},
            {"duration", duration}, {"delta", localDuration > 0 ? delta : -1}, {"synced", synced},
            {"instrumental", instrumental}, {"wordTimed", synced && QRegularExpression(R"(<\d{1,3}:\d{2}(?:[.,]\d{1,3})?>)").match(text).hasMatch()}, {"text", text}, {"score", score}, {"confident", confident},
            {"provider", "LRCLIB"}, {"sourceUrl", "https://lrclib.net/api/get/" + QString::number(id)},
            {"versionMatch", versionMatch}, {"artistMatch", artistMatch}, {"note", note}});
    }
    std::stable_sort(items.begin(), items.end(), [](const auto &a, const auto &b) { return a["score"].toInt() > b["score"].toInt(); });
    QJsonArray result;
    for (const auto &item : items) {
        if (result.size() >= 20) break;
        result.append(item);
    }
    return result;
}
static QString credit(const QJsonArray &credits) {
    QStringList names;
    for (const auto &value : credits) {
        const auto object = value.toObject();
        auto name = object["name"].toString();
        if (name.isEmpty()) name = object["artist"].toObject()["name"].toString();
        if (!name.isEmpty()) names << name;
    }
    return names.join(" / ");
}
static bool creditedArtistMatches(const QJsonArray &credits, const QString &wanted) {
    const auto name = normalized(cleanSearchText(wanted));
    if (!known(wanted)) return false;
    if (normalized(credit(credits)) == name) return true;
    if (credits.size() != 1) return false;
    const auto artist = credits.first().toObject()["artist"].toObject();
    if (normalized(artist["name"].toString()) == name) return true;
    for (const auto &alias : artist["aliases"].toArray())
        if (normalized(alias.toObject()["name"].toString()) == name) return true;
    return false;
}
QJsonArray coverCandidates(const QJsonObject &input, const QJsonObject &original) {
    const auto query = cleanQuery(original);
    QJsonArray releases = input["releases"].toArray();
    const bool recordingSearch = input.contains("recordings");
    for (const auto &value : input["release-groups"].toArray()) {
        auto group = value.toObject();
        group["group"] = group["id"];
        group["groupFallback"] = true;
        group["date"] = group["first-release-date"];
        group["release-group"] = QJsonObject{{"id", group["id"]}};
        releases.append(group);
    }
    if (recordingSearch) {
        for (const auto &value : input["recordings"].toArray()) {
            const auto recording = value.toObject();
            for (const auto &release : recording["releases"].toArray()) {
                auto item = release.toObject();
                if (item["artist-credit"].toArray().isEmpty()) item["artist-credit"] = recording["artist-credit"];
                item["recordingTitle"] = recording["title"];
                // Search results may name a recording in traditional Chinese while
                // its matching release track carries the requested simplified title.
                // Retain that explicit source alias instead of dropping the release.
                for (const auto &medium : item["media"].toArray()) {
                    for (const auto &track : medium.toObject()["track"].toArray()) {
                        const auto title = track.toObject()["title"].toString();
                        if (relatedTitle(title, query["title"].toString())) item["recordingTitle"] = title;
                    }
                }
                item["recordingArtist"] = credit(recording["artist-credit"].toArray());
                releases.append(item);
            }
        }
    }
    QList<QJsonObject> items;
    QSet<QString> seen;
    for (const auto &value : releases) {
        const auto release = value.toObject();
        const auto id = release["id"].toString();
        if (!mbid(id) || seen.contains(id)) continue;
        seen.insert(id);
        const auto title = release["title"].toString();
        const auto artist = credit(release["artist-credit"].toArray());
        if (title.isEmpty() || title.size() > 512 || artist.size() > 512) continue;
        const auto wantedArtist = known(query["albumArtist"].toString()) ? query["albumArtist"].toString() : query["artist"].toString();
        const bool albumMatch = !recordingSearch && normalized(title) == normalized(query["album"].toString());
        const bool artistMatch = creditedArtistMatches(release["artist-credit"].toArray(), wantedArtist);
        const bool editionMatch = versions(title + " " + release["disambiguation"].toString()) == versions(query["album"].toString());
        const bool official = release["status"].toString() == "Official";
        const auto group = release["release-group"].toObject()["id"].toString();
        items.append({{"id", id}, {"title", title}, {"artist", artist}, {"album", title},
            {"date", release["date"].toString().left(10)},
            {"note", (release["disambiguation"].toString().left(250) + " " + release["country"].toString().left(10)).trimmed()},
            {"recordingTitle", release["recordingTitle"].toString().left(512)},
            {"recordingArtist", release["recordingArtist"].toString().left(512)},
            {"groupFallback", release["groupFallback"].toBool()},
            {"group", mbid(group) ? group : QString()},
            {"score", (albumMatch ? 55 : 0) + (artistMatch ? 35 : 0) + (official ? 10 : 0)},
            {"confident", albumMatch && artistMatch && editionMatch && official},
            {"provider", "MusicBrainz / Cover Art Archive"}, {"sourceUrl", "https://musicbrainz.org/" + QString(release["groupFallback"].toBool() ? "release-group/" : "release/") + id}});
        if (items.size() >= 2000) break;
    }
    std::stable_sort(items.begin(), items.end(), [](const auto &a, const auto &b) { return a["score"].toInt() > b["score"].toInt(); });
    QJsonArray result;
    for (const auto &item : items) { if (result.size() >= 20) break; result.append(item); }
    return result;
}
} // namespace liusheng::online
