#include "storage.h"
#include <QtCore/QFileInfo>
#include <stdexcept>

namespace liusheng::online {
static void failure(const QString &message) { throw std::runtime_error(message.toUtf8().constData()); }
void Store::initialize() {
    if (!QDir::isAbsolutePath(root_) || QFileInfo(root_).isSymLink()) failure(QStringLiteral("资料目录无效"));
    for (const auto &name : {QString(), QStringLiteral("objects"), QStringLiteral("bindings"), QStringLiteral("search"), QStringLiteral("previews")}) {
        const auto path = name.isEmpty() ? root_ : root_ + '/' + name;
        if (QFileInfo(path).isSymLink() || !QDir().mkpath(path)) failure(QStringLiteral("无法创建私有资料目录"));
        QFile::setPermissions(path, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
    }
    pruneCache();
}
void Store::pruneCache() const {
    QList<QFileInfo> entries;
    for (const auto &folder : {"search", "previews"}) entries.append(QDir(root_ + '/' + folder).entryInfoList(QDir::Files));
    std::sort(entries.begin(), entries.end(), [](const auto &a, const auto &b) { return a.lastModified() > b.lastModified(); });
    qint64 retained = 0;
    for (const auto &entry : entries) {
        if (entry.isSymLink() || retained + entry.size() > 128 * 1024 * 1024 || entry.lastModified().daysTo(QDateTime::currentDateTimeUtc()) > 7)
            QFile::remove(entry.absoluteFilePath());
        else retained += entry.size();
    }
}

void Store::write(const QString &relative, const QByteArray &bytes) const {
    const auto path = root_ + '/' + relative;
    if (QFileInfo(path).isSymLink()) failure(QStringLiteral("资料文件必须为普通文件"));
    QSaveFile output(path);
    output.setDirectWriteFallback(false);
    if (!output.open(QIODevice::WriteOnly)) failure(QStringLiteral("无法保存在线资料：") + output.errorString());
    output.setPermissions(QFile::ReadOwner | QFile::WriteOwner);
    if (output.write(bytes) != bytes.size() || !output.commit()) failure(QStringLiteral("在线资料写入失败"));
}
QJsonObject Store::readJson(const QString &relative, qsizetype limit) const {
    QFile file(root_ + '/' + relative);
    const QFileInfo info(file);
    if (info.isSymLink() || !info.isFile() || info.size() > limit || !file.open(QIODevice::ReadOnly)) return {};
    const auto bytes = file.read(limit + 1);
    if (bytes.size() > limit) return {};
    return QJsonDocument::fromJson(bytes).object();
}
void Store::writeJson(const QString &relative, const QJsonObject &value) const {
    write(relative, QJsonDocument(value).toJson(QJsonDocument::Compact));
}
QJsonObject Store::binding(const QJsonObject &context, const QString &kind) const {
    const auto id = bindingKey(context, kind);
    if (!key(id) || (kind != "cover" && kind != "lyrics")) return {};
    const auto value = readJson("bindings/" + id + '.' + kind + ".json", 32768);
    if (value["version"].toInt() != 1 || value["key"].toString() != id || value["kind"].toString() != kind) return {};
    if (id == context["trackKey"].toString() && value["identity"] != context["identity"]) return {};
    if (value["disabled"].toBool() || value["instrumental"].toBool()) return value;
    const auto blob = value["blob"].toString();
    const auto ext = kind == "cover" ? QStringLiteral(".jpg") : QStringLiteral(".lrc");
    if (!blob.endsWith(ext) || !key(blob.chopped(4))) return {};
    auto unavailable = value;
    unavailable["unavailable"] = true;
    const auto missing = value["pinned"].toBool() ? unavailable : QJsonObject();
    QFileInfo info(root_ + "/objects/" + blob);
    if (!info.isFile() || info.isSymLink() || info.size() == 0 || info.size() > ImageLimit) return missing;
    QFile bytes(info.absoluteFilePath());
    if (!bytes.open(QIODevice::ReadOnly) || hash(bytes.read(ImageLimit + 1)) != blob.chopped(4)) return missing;
    return value;
}
QJsonObject Store::clear(const QJsonObject &context, const QString &kind) {
    if (!validContext(context) || (kind != "cover" && kind != "lyrics")) failure(QStringLiteral("资料目标无效"));
    const auto id = bindingKey(context, kind);
    // A tombstone preserves the user's reset across later automatic scans.
    QJsonObject result{{"version", 1}, {"key", id}, {"identity", context["identity"]}, {"kind", kind}, {"disabled", true}, {"pinned", true}};
    writeJson("bindings/" + id + '.' + kind + ".json", result);
    return result;
}
void Store::cache(const QString &queryKey, const QJsonArray &candidates) const {
    if (++cacheWrites_ % 16 == 0) pruneCache();
    if (!key(queryKey)) return;
    writeJson("search/" + queryKey + ".json", {{"expires", QDateTime::currentDateTimeUtc().addDays(candidates.isEmpty() ? 7 : 1).toSecsSinceEpoch()}, {"candidates", candidates}});
}
QJsonObject Store::cached(const QString &queryKey) const {
    if (!key(queryKey)) return {};
    const auto result = readJson("search/" + queryKey + ".json");
    if (result["expires"].toInteger() <= QDateTime::currentSecsSinceEpoch()) return {};
    return result;
}
void Store::clearCache() const {
    for (const auto &folder : {"search", "previews"}) {
        const QDir directory(root_ + '/' + folder);
        for (const auto &name : directory.entryList(QDir::Files)) QFile::remove(directory.filePath(name));
    }
}
QJsonObject Store::save(const QJsonObject &context, const QString &kind, QJsonObject candidate, const QByteArray &image, bool pinned) {
    if (!validContext(context) || (kind != "cover" && kind != "lyrics")) failure(QStringLiteral("资料目标无效"));
    if (!pinned && !binding(context, kind).isEmpty()) failure(QStringLiteral("已有资料或手动偏好已保留"));
    const auto id = bindingKey(context, kind);
    const bool instrumental = kind == "lyrics" && candidate["instrumental"].toBool();
    QString blob, accent;
    if (kind == "cover") {
        const auto decoded = normalizeImage(image);
        blob = hash(decoded.jpeg) + ".jpg";
        accent = decoded.accent;
        write("objects/" + blob, decoded.jpeg);
    } else if (!instrumental) {
        const auto text = candidate["text"].toString();
        const bool verified = pinned && candidate["verifiedLyrics"].toBool() && candidate["id"].toString() == "local";
        if (text.isEmpty() || text.toUtf8().size() > LyricLimit || text.contains(QChar(0)) || (!verified && !validLyrics(text)))
            failure(QStringLiteral("歌词格式或大小超出限制"));
        const auto bytes = text.toUtf8();
        blob = hash(bytes) + ".lrc";
        write("objects/" + blob, bytes);
    }
    QJsonObject result{{"version", 1}, {"key", id}, {"identity", context["identity"]}, {"kind", kind},
        {"blob", blob}, {"accent", accent}, {"pinned", pinned}, {"disabled", false},
        {"instrumental", instrumental}, {"savedAt", QDateTime::currentDateTimeUtc().toString(Qt::ISODate)}};
    for (const auto &field : {"title", "artist", "album", "provider", "sourceUrl"}) result[field] = candidate[field].toString().left(1024);
    result["sourceId"] = candidate[candidate["groupFallback"].toBool() ? "group" : "id"].toString().left(100);
    writeJson("bindings/" + id + '.' + kind + ".json", result);
    return result;
}
QString Store::previewImage(const QByteArray &image) {
    pruneCache();
    const auto value = normalizeImage(image);
    const auto name = "previews/" + hash(value.jpeg) + ".jpg";
    write(name, value.jpeg);
    return QUrl::fromLocalFile(root_ + '/' + name).toString();
}
} // namespace liusheng::online
