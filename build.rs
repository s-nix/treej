//! Embeds a Windows application manifest that declares `longPathAware`, so
//! Win32 APIs accept paths longer than MAX_PATH (260 characters) when long
//! paths are enabled on the machine. Does nothing on other targets.

use embed_manifest::manifest::{ActiveCodePage, Setting};
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let manifest = new_manifest("treej")
            .long_path_aware(Setting::Enabled)
            .active_code_page(ActiveCodePage::Utf8);
        embed_manifest(manifest).expect("unable to embed the Windows manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
