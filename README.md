<!-- Joschys_Tree – README – @ Joschy -->
# Joschys_Tree

Schneller, portabler Festplatten-Analyzer für Windows (Vorbilder: WizTree und TreeSize Pro). Eine einzelne `JoschysTree.exe`, geschrieben in Rust mit egui/eframe, nur GUI, ohne Installation.

## Download

**[Neueste Version laden (GitHub Releases)](https://github.com/jr-stack-bot/joschys_tree/releases/latest)**

- Datei `JoschysTree.exe` laden, keine Installation nötig.
- **SmartScreen:** Die exe ist nicht signiert, Windows kann warnen. Dann "Weitere Informationen → Trotzdem ausführen".
- **Prüfsumme prüfen** (optional): `JoschysTree.exe.sha256` aus dem Release laden und mit dieser PowerShell-Ausgabe vergleichen:
  `Get-FileHash .\JoschysTree.exe -Algorithm SHA256`
  Hinweis: `Get-FileHash` gibt den Hash in Großbuchstaben aus, die `.sha256`-Datei enthält ihn in Kleinbuchstaben. Verglichen wird ohne Beachtung der Groß-/Kleinschreibung, z. B. `(Get-FileHash .\JoschysTree.exe -Algorithm SHA256).Hash -eq (Get-Content .\JoschysTree.exe.sha256).Split(' ')[0]` (PowerShell `-eq` ignoriert Groß-/Kleinschreibung).
- **Administrator:** Nur mit Adminrechten gibt es den MFT-Schnellscan und das Löschen, sonst Nur-Lese-Modus.

Warum Releases? Actions-Artefakte lädt GitHub nur eingeloggten Nutzern aus, Release-Dateien sind dagegen für jeden ohne Login herunterladbar.

## Screenshots

Baumansicht, dunkles Design (Standard):

![Joschys_Tree Baumansicht, dunkel](docs/screenshot-dark.png)

Baumansicht, helles Design:

![Joschys_Tree Baumansicht, hell](docs/screenshot-light.png)

Treemap, Farben nach Dateityp:

![Joschys_Tree Treemap](docs/screenshot-treemap.png)

<sub>Aufgenommen unter Linux (Scan von `/usr/lib`), daher Linux-Pfade und Ersatzschriften; unter Windows erscheinen Consolas und Bahnschrift.</sub>

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

Bauen aus dem Quelltext: siehe [BUILD.md](BUILD.md).

## Lizenz

GNU Affero General Public License v3.0 oder später (AGPL-3.0-or-later), Volltext in [LICENSE](LICENSE).

## English

Joschys_Tree is a fast, portable disk space analyzer for Windows (in the spirit of WizTree and TreeSize Pro): a single GUI executable written in Rust. With administrator rights it uses the MFT fast scan on local NTFS volumes and enables deleting; without admin rights it runs read-only with a regular directory scan. The exe is unsigned, so SmartScreen may warn. Download the exe from [GitHub Releases](https://github.com/jr-stack-bot/joschys_tree/releases/latest) (no login needed). Licensed under AGPL-3.0-or-later. Build instructions: see [BUILD.md](BUILD.md).

---
Joschys_Tree @ Joschy
