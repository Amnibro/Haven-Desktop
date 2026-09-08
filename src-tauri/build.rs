fn main() {
    let mut build = cc::Build::new();
    build.cpp(true);
    build.std("c++17");
    build.file("native/src/c_api.cpp");
    build.include("native/src");

    #[cfg(target_os = "linux")]
    {
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

    #[cfg(target_os = "windows")]
    {
        // Let cc-rs pick MSVC (cl.exe) on windows-msvc targets.
        // Do not force g++ — MinGW lacks Windows SDK headers like
        // audioclientactivationparams.h.
        if let Ok(cxx) = std::env::var("CXX") {
            if !cxx.is_empty() {
                build.compiler(cxx);
            }
        }
        build.file("native/src/win/wasapi_capture.cpp");
        build.define("PLATFORM_WINDOWS", None);
        println!("cargo:rustc-link-lib=ole32");
        println!("cargo:rustc-link-lib=mmdevapi");
        println!("cargo:rustc-link-lib=uuid");
        println!("cargo:rustc-link-lib=Avrt");
        println!("cargo:rustc-link-lib=Psapi");
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
        build.compiler(&cxx);
        build.file("native/src/null_capture.cpp");
        build.define("PLATFORM_UNSUPPORTED", None);
        println!("cargo:rustc-link-lib=stdc++");
    }

    build.compile("haven_audio");
    tauri_build::build()
}
