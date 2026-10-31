fn main() {
    let sources = [
        "src/macos/answers.m",
        "src/macos/askpass.m",
        "src/macos/http.m",
        "src/macos/jshost.m",
        "src/macos/clipboard.m",
        "src/macos/drawing.m",
        "src/macos/drag.m",
        "src/macos/dragout.m",
        "src/macos/devices.m",
        "src/macos/files.m",
        "src/macos/icons.m",
        "src/macos/images.m",
        "src/macos/info.m",
        "src/macos/keychain.m",
        "src/macos/launcher.m",
        "src/macos/media.m",
        "src/macos/music.m",
        "src/macos/clean.m",
        "src/macos/menu.m",
        "src/macos/panel.m",
        "src/macos/people.m",
        "src/macos/promises.m",
        "src/macos/quicklook.m",
        "src/macos/shots.m",
        "src/macos/snippets.m",
        "src/macos/sounds.m",
        "src/macos/status.m",
        "src/macos/takeover.m",
        "src/macos/updates.m",
        "src/macos/windows.m",
        "src/macos/system.m",
        "src/macos/dictation.m",
        "src/macos/notch.m",
        "src/macos/typing.m",
        "src/macos/terminal.m",
    ];
    for source in sources {
        println!("cargo:rerun-if-changed={source}");
    }
    // (An API newer than MACOSX_DEPLOYMENT_TARGET, .cargo/config.toml's,
    // needs @available: without, it'd crash on an older macOS.)
    cc::Build::new().files(sources).flag("-fobjc-arc").flag("-Werror=unguarded-availability-new").compile("nf_macos");
    // @available asks clang's runtime (__isPlatformVersionAtLeast), which
    // rustc doesn't link by itself.
    let runtime = std::process::Command::new("clang").arg("--print-runtime-dir").output().expect("clang");
    println!("cargo:rustc-link-search=native={}", String::from_utf8_lossy(&runtime.stdout).trim());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
    for framework in [
        "AppKit",
        "AVFoundation",
        "AudioToolbox",
        "Carbon",
        "Contacts",
        "CoreAudio",
        "CoreBluetooth",
        "CoreImage",
        "CoreLocation",
        "CoreMedia",
        "CoreServices",
        "CoreWLAN",
        "EventKit",
        "IOBluetooth",
        "IOKit",
        "JavaScriptCore",
        "PDFKit",
        "QuickLookThumbnailing",
        "QuickLookUI",
        "Security",
        "ServiceManagement",
        "UniformTypeIdentifiers",
        "Vision",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    swift();
}

/// src/macos/translate.swift and speech.swift (Translation and
/// SpeechAnalyzer are Swift's only), as a static library: with Swift's
/// runtime (macOS's own: the SDK has its stubs, the toolchain what older
/// macOS lack), and Translation linked weakly (it's macOS 15's; F2 runs on
/// older).
fn swift() {
    const SOURCES: [&str; 2] = ["src/macos/translate.swift", "src/macos/speech.swift"];
    for source in SOURCES {
        println!("cargo:rerun-if-changed={source}");
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64".to_string(),
        Ok(arch) => arch.to_string(),
        Err(_) => "arm64".to_string(),
    };
    let target = format!("{arch}-apple-macos{}", std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "13.0".into()));
    let status = std::process::Command::new("xcrun")
        .args(["swiftc", "-parse-as-library", "-emit-library", "-static", "-O", "-swift-version", "5", "-module-name", "NFSwift", "-target", &target])
        .args(["-Xfrontend", "-disable-autolink-framework", "-Xfrontend", "Translation"])
        .args(SOURCES)
        .arg("-o")
        .arg(out.join("libnf_swift.a"))
        .status()
        .expect("swiftc");
    assert!(status.success(), "swiftc failed on {SOURCES:?}");
    let xcrun = |args: &[&str]| String::from_utf8_lossy(&std::process::Command::new("xcrun").args(args).output().expect("xcrun").stdout).trim().to_string();
    let sdk = xcrun(&["--show-sdk-path"]);
    let swiftc = std::path::PathBuf::from(xcrun(&["--find", "swiftc"]));
    let toolchain = swiftc.parent().and_then(|bin| bin.parent()).expect("toolchain");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=nf_swift");
    println!("cargo:rustc-link-search=native={sdk}/usr/lib/swift");
    println!("cargo:rustc-link-search=native={}", toolchain.join("lib/swift/macosx").display());
    println!("cargo:rustc-link-arg=-Wl,-weak_framework,Translation");
    // (Speech is older than F2's macOS; SpeechAnalyzer, in it, is 26's.)
    println!("cargo:rustc-link-lib=framework=Speech");
}
