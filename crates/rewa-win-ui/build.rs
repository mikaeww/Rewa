fn main() {
    println!("cargo:rerun-if-changed=../../packaging/windows/rewa.ico");
    println!("cargo:rerun-if-changed=../../packaging/windows/rewa.manifest");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("../../packaging/windows/rewa.ico")
            .set_manifest_file("../../packaging/windows/rewa.manifest")
            .set("ProductName", "Rewa")
            .set("FileDescription", "Rewa replay recorder")
            .compile()
            .expect("failed to embed the Rewa Windows icon");
    }
}
