#include <QtCore/QElapsedTimer>
#include <QtCore/QObject>
#include <QtQml/qqml.h>
#include <QtQuickTest/quicktest.h>

// Only the clock dependency is supplied by this runner. The components and
// their timing, popup, focus and input behavior are the production QML files.
class TestClockSource : public QObject {
    Q_OBJECT
public:
    explicit TestClockSource(QObject *parent = nullptr) : QObject(parent) { clock_.start(); }
    Q_INVOKABLE qreal monotonicMs() const { return clock_.nsecsElapsed() / 1000000.0; }
private:
    QElapsedTimer clock_;
};
class Setup : public QObject {
    Q_OBJECT
public slots:
    void applicationAvailable() {
        qmlRegisterSingletonType<TestClockSource>("io.github.dhkun.Liusheng", 1, 0, "DesktopBridge",
            [](QQmlEngine *, QJSEngine *) -> QObject * { return new TestClockSource; });
    }
};
int main(int argc, char **argv) {
    Setup setup;
    return quick_test_main_with_setup(argc, argv, "liusheng_controls", nullptr, &setup);
}
#include "control_test_main.moc"
