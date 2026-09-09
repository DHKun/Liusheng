use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn collect_assets(root: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    let mut entries = std::fs::read_dir(root)
        .expect("asset directory")
        .map(|e| e.expect("asset entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_assets(&path, files);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
            files.push(path);
        }
    }
}
fn main() {
    println!("cargo:rerun-if-changed=src/macos_media.h");
    println!("cargo:rerun-if-changed=src/macos_media.mm");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .include("src")
            .file("src/macos_media.mm")
            .compile("liusheng_media");
        for framework in [
            "Foundation",
            "AppKit",
            "MediaPlayer",
            "ImageIO",
            "CoreGraphics",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    let mut assets = Vec::new();
    collect_assets(std::path::Path::new("qml/assets"), &mut assets);
    // SAFETY: the customization only exposes this crate's source directory to
    // the generated C++ bridge so it can include application.h.
    unsafe {
        CxxQtBuilder::new_qml_module(
            QmlModule::new("io.github.dhkun.Liusheng")
                .qml_file(QmlFile::from("qml/Theme.qml").singleton(true))
                .qml_file("qml/Main.qml")
                .qml_file("qml/CoverArt.qml")
                .qml_file("qml/QuietButton.qml")
                .qml_file("qml/QuietField.qml")
                .qml_file("qml/QuietTextArea.qml")
                .qml_file("qml/QuietSlider.qml")
                .qml_file("qml/QuietMenu.qml")
                .qml_file("qml/QuietMenuItem.qml")
                .qml_file("qml/QuietComboBox.qml")
                .qml_file("qml/QuietCheckBox.qml")
                .qml_file("qml/QuietDialog.qml")
                .qml_file("qml/Icon.qml")
                .qml_file("qml/NavigationItem.qml")
                .qml_file("qml/QuietSurface.qml")
                .qml_file("qml/QuietPopover.qml")
                .qml_file("qml/QuietScrollBar.qml")
                .qml_file("qml/LibraryPage.qml")
                .qml_file("qml/CollectionView.qml")
                .qml_file("qml/DetailPage.qml")
                .qml_file("qml/TrackTable.qml")
                .qml_file("qml/QueuePanel.qml")
                .qml_file("qml/OutputPopover.qml")
                .qml_file("qml/PlayerBar.qml")
                .qml_file("qml/ImmersivePlayer.qml")
                .qml_file("qml/ListeningPalette.qml")
                .qml_file("qml/AmbientBackdrop.qml")
                .qml_file("qml/LyricsView.qml")
                .qml_file("qml/CoverFlight.qml")
                .qml_file("qml/PlaylistsPage.qml")
                .qml_file("qml/SettingsDialog.qml")
                .qml_file("qml/PlaybackClock.qml")
                .qml_file("qml/ValidationHarness.qml"),
        )
        .cpp_file("src/desktop_bridge.h")
        .qt_module("Quick")
        .qt_module("Network")
        .qt_module("Widgets")
        .qrc_resources(assets)
        .files([
            "src/app_controller.rs",
            "src/application.rs",
            "src/models.rs",
        ])
        .cc_builder(|cc| {
            cc.include("src");
        })
        .build();
    }
}
