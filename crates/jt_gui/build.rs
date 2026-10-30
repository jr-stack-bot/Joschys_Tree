// Joschys_Tree – Build-Skript: Windows-Manifest einbetten (longPathAware, asInvoker, PerMonitorV2) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

fn main() {
    // embed-manifest ist nur auf Windows-Hosts als Build-Abhängigkeit vorhanden;
    // zusätzlich muss das Ziel Windows sein.
    #[cfg(windows)]
    {
        use embed_manifest::manifest::{DpiAwareness, ExecutionLevel, Setting};
        use embed_manifest::{embed_manifest, new_manifest};
        if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
            embed_manifest(
                new_manifest("Joschy.JoschysTree")
                    .long_path_aware(Setting::Enabled)
                    .requested_execution_level(ExecutionLevel::AsInvoker)
                    .dpi_awareness(DpiAwareness::PerMonitorV2),
            )
            .expect("Manifest konnte nicht eingebettet werden");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
