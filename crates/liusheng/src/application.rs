use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;

use cxx_qt_lib::QGuiApplication;

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qguiapplication.h");
        type QGuiApplication = cxx_qt_lib::QGuiApplication;
    }

    #[namespace = "liusheng"]
    unsafe extern "C++" {
        include!("application.h");

        #[rust_name = "new_qapplication"]
        fn newQApplication(encoded_args: &[u8]) -> UniquePtr<QGuiApplication>;
        fn claimDesktopInstance() -> bool;
    }
}

pub fn new() -> cxx::UniquePtr<QGuiApplication> {
    let encoded_args = encode_args(std::env::args_os());
    ffi::new_qapplication(&encoded_args)
}

fn encode_args(args: impl IntoIterator<Item = OsString>) -> Vec<u8> {
    let mut encoded = Vec::new();
    for arg in args {
        encoded.extend_from_slice(arg.as_os_str().as_bytes());
        encoded.push(0);
    }
    encoded
}

pub fn claim_instance() -> bool {
    ffi::claimDesktopInstance()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_arguments_are_nul_delimited() {
        let encoded = encode_args([OsString::from("liusheng"), OsString::from("--smoke-test")]);
        assert_eq!(encoded, b"liusheng\0--smoke-test\0");
    }
}
