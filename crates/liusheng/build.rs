use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    // SAFETY: the customization only exposes this crate's source directory to
    // the generated C++ bridge so it can include application.h.
    unsafe {
        CxxQtBuilder::new_qml_module(
            QmlModule::new("io.github.dhkun.Liusheng")
                .qml_file("qml/ActionButton.qml")
                .qml_file("qml/AlbumCard.qml")
                .qml_file("qml/HardwareVolumeControl.qml")
                .qml_file("qml/ImmersivePlayer.qml")
                .qml_file("qml/Main.qml")
                .qml_file("qml/NavButton.qml")
                .qml_file("qml/OutputModeSwitch.qml")
                .qml_file("qml/PlayerBar.qml")
                .qml_file("qml/TrackListRow.qml")
                .qml_file("qml/VinylMark.qml"),
        )
        .qt_module("Network")
        .qt_module("Widgets")
        .qrc_resources(["qml/assets/tray.svg"])
        .files(["src/app_controller.rs", "src/application.rs"])
        .cc_builder(|cc| {
            cc.include("src");
        })
        .build();
    }
}
