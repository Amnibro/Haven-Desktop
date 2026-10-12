fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let mut build = cc::Build::new();
    build.cpp(true);
    build.std("c++17");
    build.file("native/src/c_api.cpp");
    build.include("native/src");

    if target_os == "linux" {
        let cxx = std::env::var("CXX").unwrap_or_else(|_| "g++".into());
        build.compiler(&cxx);
        build.file("native/src/linux/pulse_capture.cpp");
        build.define("PLATFORM_LINUX", None);
        build.flag_if_supported("-fexceptions");
        if let Ok(lib) = pkg_config::Config::new().probe("libpulse") {
            for p in lib.include_paths {
                build.include(p);
            }
        }
        if let Ok(lib) = pkg_config::Config::new().probe("libpulse-simple") {
            for p in lib.include_paths {
                build.include(p);
            }
        }
        println!("cargo:rustc-link-lib=pulse");
        println!("cargo:rustc-link-lib=pulse-simple");
        println!("cargo:rustc-link-lib=stdc++");
    }

    else if target_os == "windows" {
        // Let cc-rs pick MSVC (cl.exe) on windows-msvc targets.
        // Do not force g++ — MinGW lacks Windows SDK headers like
        // audioclientactivationparams.h.
        if let Ok(cxx) = std::env::var("CXX") {
            if !cxx.is_empty() {
                build.compiler(&cxx);
            }
        }
        build.file("native/src/win/wasapi_capture.cpp");
        build.define("PLATFORM_WINDOWS", None);
        // /MT: the C++ library goes into the exe instead of MSVCP140.dll.
        // tauri-build already links vcruntime statically against the
        // system UCRT, so a clean Windows needs no VC++ Redistributable.
        build.static_crt(true);
        println!("cargo:rustc-link-lib=ole32");
        println!("cargo:rustc-link-lib=mmdevapi");
        println!("cargo:rustc-link-lib=uuid");
        println!("cargo:rustc-link-lib=avrt");
        println!("cargo:rustc-link-lib=psapi");
    }

    else if target_os == "macos" {
        let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
        build.compiler(&cxx);
        build.file("native/src/null_capture.cpp");
        build.define("PLATFORM_UNSUPPORTED", None);
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .compiler(&cxx)
            .flag("-fobjc-arc")
            .file("native/src/macos/macos_ssl.mm")
            .compile("haven_macos");
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=framework=WebKit");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Security");
    }

    else {
        let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
        build.compiler(&cxx);
        build.file("native/src/null_capture.cpp");
        build.define("PLATFORM_UNSUPPORTED", None);
        println!("cargo:rustc-link-lib=stdc++");
    }

    build.compile("haven_audio");
    println!("cargo:rerun-if-changed=native/src");
    tauri_build::build()
}
