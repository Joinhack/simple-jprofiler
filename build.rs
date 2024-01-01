fn main() {
    let mut builder = cc::Build::new();
    #[cfg(target_os = "macos")]
    {
        builder.file("src/os/os_macos.c");
        builder.compile("native_utils");
        println!("cargo:rustc-link-search={}", "native_utils");
    }
}
