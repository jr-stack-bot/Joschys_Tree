<!-- Joschys_Tree – README – @ Joschy -->
# Joschys_Tree

Schneller, portabler Festplatten-Analyzer für Windows (Vorbilder: WizTree und TreeSize Pro). Eine einzelne `JoschysTree.exe`, geschrieben in Rust mit egui/eframe, nur GUI, ohne Installation.

> Screenshot folgt: `docs/screenshot.png` (Platzhalter)

## Funktionen

**Kern**
- Schneller Scan von Laufwerken und Netzfreigaben (lokal NTFS: MFT-Schnellscan, sonst paralleler Verzeichnis-Scan)
- Baumansicht mit Größe, belegtem Speicher, Dateizahl, Prozent und Datum
- Treemap-Ansicht
- Top 100 der größten Dateien
- Dateitypen-Statistik
- Suche mit Filtern (Name/Glob, Größe, Alter, Endung)
- Export als CSV, XLSX, HTML und PDF

**Erweitert**
- Duplikat-Suche (Größe, Teil-Hash, Voll-Hash)
- Snapshots speichern (`.jtscan`), Delta-Vergleich und Wachstumsverlauf
- Detailpanel mit Besitzer, Zugriffsrechten (ACL) und alternativen Datenströmen (ADS)
- Lange Pfade (über 260 Zeichen), Reparse-Points werden nicht verfolgt
- Dunkles und helles Design, Deutsch und Englisch

## Administrator oder nicht

- **Mit Administratorrechten:** MFT-Schnellscan auf lokalen NTFS-Laufwerken und Löschen (Papierkorb oder endgültig, mit zweistufiger Bestätigung).
- **Ohne Administratorrechte:** Nur-Lese-Modus mit Verzeichnis-Scan, keine Lösch-Menüpunkte. Die Fußzeile zeigt den Modus an.
- Netzfreigaben, FAT32/exFAT und Fehlerfälle fallen automatisch auf den Verzeichnis-Scan zurück.
- Gelöscht werden kann nur in einem frischen Scan dieses Rechners, nie in einem geladenen Snapshot (`.jtscan`).
- "In den Papierkorb" löscht nie endgültig: Laufwerke ohne Papierkorb (Netzfreigaben, USB-Sticks) melden einen Hinweis, dann bleibt nur "Endgültig löschen" mit zweiter Bestätigung.

## Bekannte Einschränkungen

- Hardlinks: Der Verzeichnis-Scan (Walk) zählt eine Datei mit mehreren Hardlinks mehrfach, der MFT-Scan nur einmal (erster Name). Die Summen beider Scan-Arten können daher abweichen.

## Download

Fertige Builds gibt es bei GitHub unter **Actions** (Artefakt `JoschysTree-windows-x64`) und unter **Releases** (bei Versions-Tags `v*`).

**Hinweis:** Die exe ist nicht signiert. Windows SmartScreen kann daher eine Warnung anzeigen ("Weitere Informationen", dann "Trotzdem ausführen").

Bauen aus dem Quelltext: siehe [BUILD.md](BUILD.md).

## Lizenz

GNU Affero General Public License v3.0 oder später (AGPL-3.0-or-later), Volltext in [LICENSE](LICENSE).

## English

Joschys_Tree is a fast, portable disk space analyzer for Windows (in the spirit of WizTree and TreeSize Pro): a single GUI executable written in Rust. With administrator rights it uses the MFT fast scan on local NTFS volumes and enables deleting; without admin rights it runs read-only with a regular directory scan. The exe is unsigned, so SmartScreen may warn. Download builds from GitHub Actions artifacts or Releases. Licensed under AGPL-3.0-or-later. Build instructions: see [BUILD.md](BUILD.md).

---
Joschys_Tree @ Joschy
