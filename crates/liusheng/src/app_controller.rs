use cxx_qt_lib::QString;

#[derive(Default)]
pub struct AppControllerRust {
    status: QString,
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status)]
        #[namespace = "liusheng"]
        type AppController = super::AppControllerRust;
    }
}
