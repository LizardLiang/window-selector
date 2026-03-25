use std::process::Command;

fn main() {
    // Path to the Windows SDK resource compiler (rc.exe).
    // Matches the VS installation referenced in .cargo/config.toml.
    // On other machines, adjust to your VS/SDK installation path.
    let rc_exe = r"C:\Program Files\Microsoft Visual Studio\18\Community\SDK\ScopeCppSDK\vc15\SDK\bin\rc.exe";
    let rc_file = "resources/app.rc";
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let res_file = format!("{}/app.res", out_dir);

    println!("cargo:rerun-if-changed={}", rc_file);
    println!("cargo:rerun-if-changed=resources/app.ico");

    let status = Command::new(rc_exe)
        .arg("/fo")
        .arg(&res_file)
        .arg(rc_file)
        .status()
        .expect("Failed to run rc.exe");

    if !status.success() {
        panic!("rc.exe failed with status: {}", status);
    }

    println!("cargo:rustc-link-arg-bins={}", res_file);
}