fn main() {
    // Compile the Windows resource file (app icon, version info).
    // embed-resource finds rc.exe automatically via vswhere / Windows SDK paths.
    embed_resource::compile("resources/app.rc", embed_resource::NONE);
}