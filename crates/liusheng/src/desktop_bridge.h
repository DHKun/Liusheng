#pragma once
#include <QtCore/QCoreApplication>
#include <QtCore/QCryptographicHash>
#include <QtCore/QDir>
#include <QtCore/QElapsedTimer>
#include <QtCore/QFileInfo>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QLockFile>
#include <QtCore/QStandardPaths>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtGui/QFileOpenEvent>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtGui/QMouseEvent>
#include <QtGui/QKeyEvent>
#include <QtQuick/QQuickItem>
#include <QtWidgets/QSystemTrayIcon>
#include <QtNetwork/QLocalServer>
#include <QtNetwork/QLocalSocket>
#include <QtQml/qqml.h>
#include <QtQml/QQmlEngine>
#include <QtQuick/QQuickWindow>
#include <memory>

namespace liusheng {
class DesktopBridge : public QObject {
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool wayland READ isWayland CONSTANT)
public:
    explicit DesktopBridge(QObject* parent = nullptr) : QObject(parent) {
        clock_.start();
        if (qApp) qApp->installEventFilter(this);
        connect(&server_, &QLocalServer::newConnection, this, [this] {
            while (auto* socket = server_.nextPendingConnection()) {
                auto data = std::make_shared<QByteArray>();
                auto finished = std::make_shared<bool>(false);
                connect(socket, &QLocalSocket::readyRead, this, [this, socket, data, finished] {
                    if (*finished) return;
                    data->append(socket->readAll());
                    if (data->size() > 1024 * 1024) { *finished = true; socket->abort(); return; }
                    if (!data->contains('\n')) return;
                    *finished = true;
                    QJsonParseError error;
                    auto doc = QJsonDocument::fromJson(data->left(data->indexOf('\n')), &error);
                    if (error.error == QJsonParseError::NoError && doc.isArray() && doc.array().size() <= 4096) {
                        bool stringsOnly = true;
                        for (const auto value : doc.array()) stringsOnly &= value.isString();
                        if (stringsOnly) {
                            const QString payload = QString::fromUtf8(doc.toJson(QJsonDocument::Compact));
                            if (uiReady_) { emit openRequested(payload); emit raiseRequested(); }
                            else pending_.append(doc.array());
                            socket->write("OK\n"); socket->flush();
                        }
                    }
                    socket->disconnectFromServer();
                });
                connect(socket, &QLocalSocket::disconnected, socket, &QObject::deleteLater);
                QTimer::singleShot(3000, socket, [socket] { socket->abort(); socket->deleteLater(); });
            }
        });
    }
    static DesktopBridge* instance;
    static DesktopBridge* create(QQmlEngine*, QJSEngine*) { return instance; }

    bool isWayland() const { return QGuiApplication::platformName().startsWith("wayland"); }
    Q_INVOKABLE bool trayAvailable() const { return QSystemTrayIcon::isSystemTrayAvailable(); }
    Q_INVOKABLE QString platformName() const { return QGuiApplication::platformName(); }
    Q_INVOKABLE int visibleNativePopups() const {
        int count = 0;
        for (QWindow* w : QGuiApplication::topLevelWindows())
            if (w->isVisible() && w->type() == Qt::Popup) ++count;
        return count;
    }
    Q_INVOKABLE double monotonicMs() const { return double(clock_.nsecsElapsed()) / 1000000.0; }
    Q_INVOKABLE QString initialFiles() {
        uiReady_ = true;
        QJsonArray files = arguments();
        for (const auto& batch : pending_) for (const auto& value : batch) files.append(value);
        pending_.clear();
        return QString::fromUtf8(QJsonDocument(files).toJson(QJsonDocument::Compact));
    }
    Q_INVOKABLE void captureForTest(QObject* object, const QString& name) const {
        const auto arguments = QCoreApplication::arguments();
        if (!arguments.contains("--ui-test")
            && !(arguments.contains("--functional-test") && !testDirectory().isEmpty())) return;
        const auto directory = qEnvironmentVariable("LIUSHENG_SCREENSHOT_DIR");
        if (directory.isEmpty()) return;
        auto* window = qobject_cast<QQuickWindow*>(object);
        if (window && name == QFileInfo(name).fileName()) {
            QDir().mkpath(directory);
            window->grabWindow().save(directory + "/" + name + ".png");
        }
    }
    Q_INVOKABLE QString testDirectory() const {
        if (!QCoreApplication::arguments().contains("--functional-test") && !QCoreApplication::arguments().contains("--ui-test")) return {};
        const auto directory = QDir::cleanPath(qEnvironmentVariable("LIUSHENG_QA_DIR"));
        if (directory == "." || !QDir::isAbsolutePath(directory) || !QFileInfo(directory + "/.liusheng-test-workspace").isFile()) return {};
        if (QDir::cleanPath(qEnvironmentVariable("XDG_DATA_HOME")) != directory + "/data") return {};
        return directory;
    }
    // Input helpers are enabled only by explicit validation flags in a marked, isolated fixture.
    Q_INVOKABLE QObject* testFind(QObject* root, const QString& name) const {
        if (testDirectory().isEmpty() || !root) return nullptr;
        return root->findChild<QObject*>(name);
    }
    Q_INVOKABLE bool testClick(QObject* target, int button = 1) const {
        if (testDirectory().isEmpty()) return false;
        auto* item = qobject_cast<QQuickItem*>(target);
        if (!item || !item->window() || !item->isVisible() || !item->isEnabled()) return false;
        auto* window = item->window();
        const auto position = item->mapToScene(QPointF(item->width() / 2, item->height() / 2));
        const auto global = QPointF(window->mapToGlobal(position.toPoint()));
        const auto mouseButton = button == 2 ? Qt::RightButton : Qt::LeftButton;
        const auto timestamp = static_cast<quint64>(clock_.elapsed());
        QMouseEvent move(QEvent::MouseMove, position, position, global, Qt::NoButton, Qt::NoButton, Qt::NoModifier);
        move.setTimestamp(timestamp);
        QCoreApplication::sendEvent(window, &move);
        QMouseEvent press(QEvent::MouseButtonPress, position, position, global, mouseButton, mouseButton, Qt::NoModifier);
        press.setTimestamp(timestamp + 1);
        QCoreApplication::sendEvent(window, &press);
        QMouseEvent release(QEvent::MouseButtonRelease, position, position, global, mouseButton, Qt::NoButton, Qt::NoModifier);
        release.setTimestamp(timestamp + 2);
        QCoreApplication::sendEvent(window, &release);
        return true;
    }
    Q_INVOKABLE void testKey(QObject* target, int key, int modifiers = 0) const {
        if (testDirectory().isEmpty()) return;
        auto* window = qobject_cast<QQuickWindow*>(target);
        if (!window) return;
        QKeyEvent press(QEvent::KeyPress, key, Qt::KeyboardModifiers(modifiers));
        QKeyEvent release(QEvent::KeyRelease, key, Qt::KeyboardModifiers(modifiers));
        QCoreApplication::sendEvent(window, &press);
        QCoreApplication::sendEvent(window, &release);
    }
    Q_INVOKABLE QString localPath(const QUrl& url) const { return url.toLocalFile(); }
    Q_INVOKABLE void profileMark(const QString& name) const {
        if (qEnvironmentVariableIsSet("LIUSHENG_PROFILE")) qInfo().noquote() << "[perf]" << name << monotonicMs();
    }
    bool claimInstance() {
        if (qEnvironmentVariableIsSet("LIUSHENG_ALLOW_MULTIPLE") || QCoreApplication::arguments().contains("--smoke-test")) return true;
        QString runtime = QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation);
        if (runtime.isEmpty()) runtime = QDir::homePath() + "/.cache";
        QDir().mkpath(runtime);
        const auto identity = QDir::homePath() + qEnvironmentVariable("XDG_DATA_HOME");
        const auto hash = QCryptographicHash::hash(identity.toUtf8(), QCryptographicHash::Sha256).toHex().left(16);
        const QString socketName = runtime + "/liusheng-" + QString::fromLatin1(hash);
        lock_ = std::make_unique<QLockFile>(socketName + ".lock");
        if (!lock_->tryLock(100)) {
            QLocalSocket socket;
            socket.connectToServer(socketName);
            if (!socket.waitForConnected(1500)) { qWarning("留声已有实例或实例锁不可用，无法传递打开请求"); return false; }
            socket.write(QJsonDocument(arguments()).toJson(QJsonDocument::Compact) + '\n');
            socket.waitForBytesWritten(1500);
            if (!socket.waitForReadyRead(1500)) qWarning("已有留声实例尚未确认请求");
            return false;
        }
        // The process owns the lock before removing a stale server endpoint.
        QLocalServer::removeServer(socketName);
        server_.setSocketOptions(QLocalServer::UserAccessOption);
        if (!server_.listen(socketName)) { qWarning() << "本地打开请求服务：" << server_.errorString(); return false; }
        return true;
    }
signals:
    void openRequested(const QString& pathsJson);
    void raiseRequested();
protected:
    bool eventFilter(QObject* object, QEvent* event) override {
        if (event->type() == QEvent::FileOpen) {
            const auto* fileEvent = static_cast<QFileOpenEvent*>(event);
            QJsonArray files {fileEvent->url().toString()};
            if (uiReady_) { emit openRequested(QString::fromUtf8(QJsonDocument(files).toJson(QJsonDocument::Compact))); emit raiseRequested(); }
            else pending_.append(files);
            return true;
        }
        return QObject::eventFilter(object, event);
    }
private:
    QJsonArray arguments() const {
        QJsonArray files;
        bool literal = false;
        for (const QString& arg : QCoreApplication::arguments().mid(1)) {
            if (arg == "--") { literal = true; continue; }
            if (!literal && arg.startsWith('-')) continue;
            const QUrl url(arg);
            if (url.isLocalFile()) files.append(url.toString());
            else if (url.scheme().isEmpty()) files.append(QUrl::fromLocalFile(QFileInfo(arg).absoluteFilePath()).toString());
        }
        return files;
    }
    QElapsedTimer clock_;
    QLocalServer server_;
    std::unique_ptr<QLockFile> lock_;
    QList<QJsonArray> pending_;
    bool uiReady_ = false;
};
inline DesktopBridge* DesktopBridge::instance = nullptr;
inline bool claimDesktopInstance() { return DesktopBridge::instance && DesktopBridge::instance->claimInstance(); }
} // namespace liusheng
