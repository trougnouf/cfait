// build.rs
//
// Keep this build script minimal: only compile the Windows resource when
// building for the Windows target. Android string generation is handled by
// the Gradle build script (`android/app/build.gradle.kts`) and must not be
// duplicated here to avoid build-time conflicts and duplication of behavior.

fn main() {
    // Only compile the resource for Windows targets
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/cfait.ico");

        // Optional: Set file properties visible in Windows "Properties -> Details"
        res.set("ProductName", "Cfait");
        res.set(
            "FileDescription",
            "A powerful, fast and elegant CalDAV task manager",
        );

        if let Err(e) = res.compile() {
            println!("cargo:warning=Failed to compile windows resource: {}", e);
        }
    }
}
