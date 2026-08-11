mod app_controller;
mod application;
mod mpris;

use std::pin::Pin;

use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQmlEngine, QString, QUrl};

fn main() {
    let mut app = application::new();
    QGuiApplication::set_desktop_file_name(&QString::from("io.github.dhkun.Liusheng"));
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("留声"));
        app.as_mut()
            .set_application_display_name(&QString::from("留声"));
        app.as_mut()
            .set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
        app.as_mut()
            .set_organization_domain(&QString::from("github.com"));
        app.as_mut().set_organization_name(&QString::from("DHKun"));
    }
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(
            "qrc:/qt/qml/io/github/dhkun/Liusheng/qml/Main.qml",
        ));
    }
    if let Some(engine) = engine.as_mut() {
        let engine: Pin<&mut QQmlEngine> = engine.upcast_pin();
        engine.on_quit(|_| {}).release();
    }
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
