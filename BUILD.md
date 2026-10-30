<!-- Joschys_Tree – Build-Anleitung – @ Joschy -->
# Bauen und Abnahme (@ Joschy)

## Lokal bauen

1. Rust installieren: <https://rustup.rs> (`rust-toolchain.toml` wählt stable, Mindestversion 1.95).
2. Windows: Visual Studio Build Tools (MSVC, "Desktop development with C++") installieren.
3. Im Repo-Root:
   ```
   cargo build --release
   ```
   Ergebnis: `target\release\JoschysTree.exe` (Windows) bzw. `target/release/JoschysTree` (Linux, nur Entwicklung).
4. Für eine exe ohne VC-Runtime: `set RUSTFLAGS=-C target-feature=+crt-static` vor dem Build (PowerShell: `$env:RUSTFLAGS="-C target-feature=+crt-static"`).
5. Prüfungen: `cargo fmt --all --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`.

Hinweis: Die `cfg(windows)`-Teile lassen sich nur unter Windows (bzw. in der CI) bauen und prüfen.

## CI (GitHub Actions)

Datei `.github/workflows/build.yml`, Trigger: Push auf `main`, Pull Requests, Tags `v*`.
- `linux-test`: fmt-check, clippy, `cargo test --workspace`.
- `windows-build`: clippy und Release-Build für `x86_64-pc-windows-msvc` mit `+crt-static`, Upload der `JoschysTree.exe` als Artefakt. Bei Tag `v*` zusätzlich ein GitHub-Release mit der exe.

## Manuelle Windows-Abnahme (12 Punkte)

1. Ohne Admin starten: Walk-Modus, keine Lösch-Menüpunkte, Fußzeile zeigt "Nur-Lesen".
2. Als Admin C:\ scannen: MFT-Modus, Zeit notieren (Ziel: unter 15 s bei 1 Mio. Dateien auf SSD). Summe mit Explorer-Eigenschaften vergleichen (±1 %).
3. `\\server\share` und `\\pc\c$` scannen: automatischer Walk-Fallback.
4. FAT32-/exFAT-USB-Stick: Fallback.
5. Pfad über 300 Zeichen anlegen: wird gefunden, Export enthält ihn, Explorer-Öffnen funktioniert.
6. Datei mit ADS (`echo x > a.txt:geheim`): Detailpanel zeigt den Stream.
7. Owner/ACL eines Systemordners wird angezeigt.
8. Löschen: Papierkorb funktioniert. Endgültig nur nach zweiter Bestätigung.
9. Theme- und Sprachwechsel, Neustart behält die Einstellung.
10. Exporte in Excel, Browser und PDF-Reader öffnen: "@ Joschy" im Fuß.
11. Snapshot speichern, Dateien ändern, neu scannen, Delta stimmt.
12. Server 2016 (RDP, ohne GPU): exe startet (wgpu/DX12-WARP bzw. Fallback).

---
Joschys_Tree @ Joschy
