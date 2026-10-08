fn main() {
    println!("cargo:rerun-if-changed=assets/app-icon.ico");
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/app-icon.ico")
                .set("FileDescription", "DevLogica Tools")
                .set("ProductName", "DevLogica Tools")
                .set("CompanyName", "DevLogica");
            // Se manca il compilatore di risorse l'app si compila lo stesso, solo senza icona.
            if let Err(e) = res.compile() {
                println!("cargo:warning=Icona non incorporata: {e}");
            }
        }
    }
}
