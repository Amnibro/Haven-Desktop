fn main() {
    std::env::set_var("CXX", std::env::var("CXX").unwrap_or_else(|_| "g++".into()));

    let mut build = cc::Build::new();
    build.cpp(true);
    build.std("c++17");
    build.compiler(std::env::var("CXX").unwrap_or_else(|_| "g++".into()));
    build.file("native/src/c_api.cpp");
    build.include("native/src");

    #[cfg(target_os = "linux")]
    {
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
    }

    #[cfg(target_os = "windows")]
    {
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
        build.file("native/src/null_capture.cpp");
        build.define("PLATFORM_UNSUPPORTED", None);
    }

    build.compile("haven_audio");
    println!("cargo:rustc-link-search=native=/usr/lib/gcc/x86_64-linux-gnu/13");
    println!("cargo:rustc-link-lib=stdc++");
    tauri_build::build()
}
