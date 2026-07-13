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
        res.set_icon("assets/autogen/cfait.ico");

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

    // Get the git commit hash for display in about dialog
    let git_hash = if let Ok(hash) = std::process::Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
    {
        if let Ok(hash_str) = String::from_utf8(hash.stdout) {
            hash_str.trim().to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);
}
