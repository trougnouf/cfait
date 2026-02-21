// build.rs
fn main() {
    // Only compile the resource for Windows targets
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
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
