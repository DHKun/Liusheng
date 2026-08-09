use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("io.github.dhkun.Liusheng")
            .qml_file("qml/Main.qml")
            .qml_file("qml/NavButton.qml")
            .qml_file("qml/PlayerBar.qml")
            .qml_file("qml/VinylMark.qml"),
    )
    .qt_module("Network")
    .files(["src/app_controller.rs"])
    .build();
}
