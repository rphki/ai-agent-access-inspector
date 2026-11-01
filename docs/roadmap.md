# Roadmap: AI Agent Access Inspector

Diese Roadmap ist das maßgebliche Planungsdokument des Projekts. Sie enthält Ziel, Anforderungen, Architektur und Umsetzungsreihenfolge. Der ursprüngliche Umsetzungsplan liegt unter [`plan-initial.md`](plan-initial.md) und ist nur noch historisch von Interesse.

## 1. Ziel

Eine lokale, plattformübergreifende Desktop-Anwendung zeigt, welche Ordner laut Konfigurationen, Projektdateien und erkennbaren Einstellungen für lokale KI-Agenten freigegeben oder potenziell erreichbar sind.

Der erste unterstützte Agent ist **Claude Code**. Die Architektur ist so angelegt, dass später Claude Desktop, Codex, ChatGPT, Cursor und weitere Anwendungen als unabhängige Scanner ergänzt werden können.

Die Anwendung:

- läuft unter macOS, Windows und Linux,
- arbeitet vollständig lokal,
- benötigt kein Benutzerkonto und kein Backend,
- verändert keine Agentenkonfiguration,
- überträgt keine Dateipfade,
- erklärt zu jedem Fund, woher die Information stammt,
- unterscheidet zwischen konfiguriertem und nur potenziellem Zugriff.

## 2. Entscheidungen

| Thema | Entscheidung |
|---|---|
| Repository | öffentliches GitHub-Repository [`rphki/ai-agent-access-inspector`](https://github.com/rphki/ai-agent-access-inspector) |
| Lokaler Ordner | wird von `agent-access-inspector` in `ai-agent-access-inspector` umbenannt |
| Lizenz | Apache-2.0 |
| Projektstruktur | Cargo-Workspace mit getrennten Crates für Kern, CLI und Tauri |
| Reihenfolge | Kern und CLI zuerst (`v0.1.0`), Oberfläche danach (`v0.2.0`) |
| Recherche | Claude-Code-Recherche vor dem Datenmodell, weil sie dessen Form bestimmt |
| MCP-Server | werden im MVP mitgescannt |
| Claude-Code-Versionen | unterstützt wird nur das aktuell offiziell dokumentierte Konfigurationsformat; ältere Formate werden nicht gezielt ausgewertet, führen aber nie zum Abbruch (siehe 8.1) |
| Vorgehen | phasenweise; nach jeder Phase Abnahme und Abstimmung |

## 3. Technologiestack

- **Kern und Scanner:** Rust (Crate `inspector-core`, ohne Tauri-Abhängigkeit)
- **CLI:** Rust (Crate `inspector-cli`)
- **Desktop-Framework:** Tauri 2
- **Frontend:** React mit TypeScript
- **Build-System Frontend:** Vite
- **Paketmanager Frontend:** pnpm
- **Tests Backend:** Rust Unit- und Integrationstests
- **Tests Frontend:** Vitest und React Testing Library
- **End-to-End-Tests:** nach dem MVP
- **CI/CD:** GitHub Actions
- **Veröffentlichung:** GitHub Releases

## 4. Produktbegriffe

Die Anwendung verwendet bewusst nicht pauschal den Begriff „zugreifbar“.

### 4.1 Status eines Findings

- **Configured:** Der Pfad bzw. die Regel steht ausdrücklich in einer Konfiguration.
- **Observed:** Der Pfad wurde in einer gespeicherten Projekt- oder Sitzungsinformation gefunden.
- **Potential:** Der Pfad könnte aufgrund des Prozesskontexts oder der Betriebssystemrechte erreichbar sein (siehe 4.4).
- **Blocked:** Der Pfad ist konfiguriert, aber gegenwärtig nicht erreichbar.
- **Unknown:** Es liegen nicht genügend Informationen für eine belastbare Bewertung vor.

### 4.2 Geltungsebene

- **Managed:** systemweite, administrativ verwaltete Einstellungen (höchste Priorität)
- **Benutzerweit**
- **Projektbezogen**
- **Lokal im Projekt**
- **Sitzungsbezogen**
- **Laufzeitbezogen**
- **Betriebssystembezogen**

### 4.3 Wirkung einer Regel

- **Allow:** erlaubt den Zugriff ohne Rückfrage.
- **Ask:** erlaubt den Zugriff nach Rückfrage.
- **Deny:** verbietet den Zugriff.

Deny-Regeln sind kein Zugriff. Sie werden trotzdem als Findings geführt, weil sie andere Findings einschränken und für die Erkennung widersprüchlicher Regeln nötig sind.

### 4.4 Bedeutung von „Potential“

Ohne Sandbox kann ein Agent über Shell-Werkzeuge alles erreichen, was der Benutzer erreichen kann. Statt jeden Ordner einzeln als `Potential` auszuweisen, erzeugt der Scanner ein zusammenfassendes Finding, z. B. „Shell-Werkzeug ohne Sandbox – effektiv Benutzerrechte“. Eine konfigurierte Sandbox schränkt dieses Finding ein und erzeugt stattdessen Findings für die von ihr erlaubten Bereiche.

## 5. Projektstruktur

```text
ai-agent-access-inspector/
├── .github/
│   ├── workflows/
│   │   ├── ci.yml
│   │   └── release.yml
│   └── ISSUE_TEMPLATE/
├── Cargo.toml                     # Workspace
├── crates/
│   ├── inspector-core/            # ohne Tauri-Abhängigkeit
│   │   └── src/
│   │       ├── model/             # Finding, Diagnose, ScanResult
│   │       ├── analysis/          # Pfadanalyse, Deduplizierung, Risiko
│   │       ├── platform/          # Verzeichnisse und Eigenheiten je OS
│   │       ├── scanners/
│   │       │   └── claude_code/
│   │       ├── export/            # JSON, Markdown, Anonymisierung
│   │       └── lib.rs
│   └── inspector-cli/             # CLI-Diagnosemodus
│       └── src/main.rs
├── src-tauri/                     # dünne Schicht: Tauri-Kommandos
│   ├── src/
│   │   ├── commands/
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── capabilities/
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                           # React-Frontend
│   ├── components/
│   ├── pages/
│   ├── hooks/
│   ├── types/
│   └── App.tsx
├── fixtures/
│   └── claude-code/
│       ├── macos/
│       ├── linux/
│       ├── windows/
│       └── malformed/
├── docs/
│   ├── roadmap.md
│   ├── plan-initial.md            # historisch
│   ├── architecture.md
│   ├── findings-model.md
│   ├── claude-code-scanner.md
│   ├── adding-a-scanner.md
│   ├── privacy.md
│   └── release-process.md
├── package.json
├── pnpm-lock.yaml
├── README.md
├── CONTRIBUTING.md
├── SECURITY.md
├── LICENSE
└── CHANGELOG.md
```

`src-tauri/` und `src/` entstehen erst in Phase 5.

## 6. Datenmodell

Der Kern stellt ein agentenunabhängiges Modell bereit. Die folgende Skizze ist vorläufig; die endgültige Form wird in Phase 2 auf Basis der Recherche aus Phase 1 festgelegt.

```rust
pub struct Finding {
    pub id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub subject: FindingSubject,      // Agent selbst oder z. B. ein MCP-Server
    pub target: FindingTarget,        // Pfad bzw. Regelmuster
    pub effect: RuleEffect,
    pub access: AccessKind,
    pub status: FindingStatus,
    pub scope: FindingScope,
    pub source: FindingSource,
    pub explanation: String,
    pub confidence: Confidence,
    pub path_state: PathState,
    pub warnings: Vec<Warning>,
}

pub struct FindingTarget {
    pub raw: String,                  // ursprünglicher Wert, z. B. "Read(~/projekt/**)"
    pub pattern_kind: PatternKind,    // exakter Pfad, Verzeichnis, Glob
    pub base_path: Option<PathBuf>,   // normalisierter Basispfad
    pub display_path: String,
}

pub enum FindingSubject {
    Agent,
    McpServer { name: String },
}

pub enum PatternKind {
    ExactPath,
    Directory,
    Glob,
    Unparsed,
}

pub enum RuleEffect {
    Allow,
    Ask,
    Deny,
    NotApplicable,                    // z. B. Observed-Findings ohne Regel
}

pub enum AccessKind {
    Read,
    Write,
    Execute,
    ReadWrite,
    Unknown,
}

pub enum FindingStatus {
    Configured,
    Observed,
    Potential,
    Blocked,
    Unknown,
}

pub enum FindingScope {
    Managed,
    User,
    Project,
    ProjectLocal,
    Session,
    Runtime,
    OperatingSystem,
}

pub enum Confidence {
    High,
    Medium,
    Low,
}
```

Die Quelle eines Findings (`FindingSource`) enthält:

- Pfad der Konfigurationsdatei,
- Konfigurationstyp,
- Einstufung der Quelle als dokumentiert oder heuristisch,
- JSON-Schlüssel beziehungsweise JSON Pointer,
- ursprünglichen, noch nicht normalisierten Wert,
- Parser-Version,
- optional einen kurzen Belegausschnitt ohne sensible Inhalte.

Neben Findings liefert ein Scan **Diagnosen** (z. B. ungültiges JSON, nicht lesbare Datei, unbekannter Schlüssel). Diagnosen führen nie zum Abbruch des Scans.

## 7. Scanner-Schnittstelle und Scan-Kontext

### 7.1 Scanner

Jeder Agent erhält einen eigenen Scanner.

```rust
pub trait AgentScanner {
    fn metadata(&self) -> ScannerMetadata;
    fn detect(&self, context: &ScanContext) -> DetectionResult;
    fn scan(&self, context: &ScanContext) -> ScanResult;
}
```

Ein Scanner ist verantwortlich für:

- Erkennung der Agenteninstallation,
- Ermittlung relevanter Konfigurationsorte,
- fehlertolerantes Einlesen,
- Übersetzung agentenspezifischer Einstellungen in das gemeinsame Datenmodell,
- Warnungen bei unbekannten oder nicht unterstützten Formaten.

Ein Scanner ist nicht verantwortlich für:

- Darstellung,
- globale Deduplizierung,
- allgemeine Pfadnormalisierung,
- Export,
- Telemetrie,
- Änderungen an den gefundenen Konfigurationen.

### 7.2 Scan-Kontext

Scanner dürfen nicht direkt auf das echte Home-Verzeichnis oder die echte Umgebung zugreifen, sondern nur über den Kontext.

```rust
pub struct ScanContext {
    pub home_dir: PathBuf,
    pub current_dir: Option<PathBuf>,
    pub config_dirs: PlatformConfigDirs,
    pub operating_system: OperatingSystem,
    pub env: EnvSnapshot,             // z. B. CLAUDE_CONFIG_DIR, APPDATA
}
```

Dadurch können alle Plattformvarianten auf jedem Entwicklungssystem mit Fixtures getestet werden.

Der Kern unterstützt zwei Modi:

- **Systemscan:** verwendet die tatsächlichen Verzeichnisse und Umgebungsvariablen des Benutzers.
- **Fixture-Scan:** verwendet ein kontrolliertes Testverzeichnis und eine vorgegebene Umgebung.

## 8. Claude-Code-Scanner

### 8.1 Spezifikation

Vor der Implementierung werden die aktuellen Konfigurationsorte und -ebenen aus offizieller Dokumentation und überprüfbaren lokalen Installationen erfasst und in `docs/claude-code-scanner.md` dokumentiert:

- die Claude-Code-Version, gegen die die Spezifikation geprüft wurde, und das Datum der Recherche,
- Konfigurationsorte pro Betriebssystem,
- Managed-, Benutzer-, Projekt- und lokale Konfiguration sowie deren Priorität,
- zusätzliche Verzeichnisse,
- Allow-/Ask-/Deny-Regeln und deren Mustersyntax,
- gefährliche Permission-Modi,
- Sandbox-Einstellungen,
- MCP-Server-Konfiguration,
- gespeicherte Projekt- oder Sitzungsinformationen,
- bekannte Grenzen des Scanners.

Unterstützt wird nur das aktuell dokumentierte Konfigurationsformat. Schlüssel oder Dateien aus älteren Formaten werden nicht gezielt interpretiert; sie erscheinen als Diagnose (z. B. „unbekannter Schlüssel“) oder als Finding mit Status `Unknown`. Ändert Claude Code sein Format, wird die Spezifikation aktualisiert.

Undokumentierte Dateien werden ausdrücklich als heuristische Quelle gekennzeichnet.

Die Spezifikation liegt in [`claude-code-scanner.md`](claude-code-scanner.md) vor (Stand 2026-11-01). Sie ist für den Scanner maßgeblich; die Abschnitte 8.2 bis 8.4 sind nur eine Übersicht.

### 8.2 Zu untersuchende Quellen

- Managed/Enterprise-Settings je Betriebssystem
- Benutzerkonfiguration (`~/.claude/settings.json` und ggf. weitere)
- `.claude/settings.json` im Projekt
- `.claude/settings.local.json` im Projekt
- Umgebungsvariable `CLAUDE_CONFIG_DIR` und andere ortsverändernde Variablen
- zusätzliche Verzeichnisse (z. B. `additionalDirectories`)
- dateisystembezogene Allow-/Ask-/Deny-Regeln
- Permission-Modi, insbesondere Einstellungen zum Überspringen von Rückfragen und deren Sperre
- Sandbox-Einstellungen
- MCP-Konfiguration (`.mcp.json` im Projekt, benutzerweite und projektbezogene MCP-Einträge) mit Pfadargumenten
- `~/.claude.json` (Projektliste und projektbezogene Einträge) – voraussichtlich heuristisch
- `~/.claude/projects/` (gespeicherte Sitzungen) – voraussichtlich heuristisch, Status `Observed`
- sitzungsbezogene Angaben wie `--add-dir`, sofern sie überhaupt persistiert werden

`CLAUDE.md` wird nicht als Ordnerfreigabe gewertet. Enthaltene Pfade dürfen höchstens später als unverifizierte Referenzen ausgewiesen werden.

Aus Sitzungsdaten werden nur Pfade bzw. Projektzuordnungen gelesen, keine Inhalte.

### 8.3 Verhalten

Der Scanner:

1. erkennt, ob Claude Code installiert oder konfiguriert ist,
2. ermittelt die Konfigurationsorte unter Berücksichtigung von Umgebungsvariablen,
3. liest Managed- und Benutzerkonfiguration,
4. ermittelt bekannte Projekte,
5. untersucht deren Projekt- und lokale Konfigurationen,
6. extrahiert Regeln, zusätzliche Verzeichnisse, MCP-Server, Modi und Sandbox-Einstellungen,
7. übergibt Pfade und Muster an die Pfadanalyse,
8. erzeugt Findings, Warnungen und Diagnosen,
9. erzeugt das zusammenfassende `Potential`-Finding gemäß 4.4.

Die Deduplizierung inhaltlich identischer Ergebnisse übernimmt die gemeinsame Analysekomponente.

### 8.4 Fehlerbehandlung

Die Anwendung darf in folgenden Fällen nicht abbrechen:

- ungültiges JSON,
- teilweise geschriebene Konfigurationsdateien,
- nicht lesbare Dateien,
- nicht vorhandene Pfade,
- unbekannte Konfigurationsschlüssel,
- nicht interpretierbare Regelmuster,
- Symlink-Schleifen,
- unterschiedliche Groß-/Kleinschreibung,
- ungültige Windows- oder UNC-Pfade.

Stattdessen wird ein Diagnoseeintrag erzeugt.

## 9. Pfadanalyse

Eine gemeinsame Analysekomponente in `inspector-core` übernimmt:

- Auflösung von `~`,
- Auflösung unterstützter Umgebungsvariablen,
- Umwandlung relativer in absolute Pfade (relativ zu Projekt bzw. Konfigurationsdatei, je nach Regel),
- Ableitung des Basispfads aus Regelmustern,
- lexikalische Normalisierung,
- optionale kanonische Auflösung,
- Erkennung von Symlinks,
- Prüfung, ob ein Pfad existiert,
- Prüfung, ob er Datei oder Verzeichnis ist,
- Erkennung verschachtelter Freigaben,
- Deduplizierung,
- plattformspezifische Behandlung der Groß-/Kleinschreibung,
- Erkennung besonders breiter Pfade.

Warnungen werden unter anderem erzeugt für:

- Home-Verzeichnis,
- Dateisystemwurzel,
- gesamtes Laufwerk,
- Benutzerverzeichnis mit persönlichen Unterordnern,
- nicht mehr vorhandenen Pfad,
- auf einen anderen Bereich zeigenden Symlink,
- widersprüchliche Allow-/Deny-Regeln,
- aktivierte gefährliche Permission-Modi,
- Shell-Zugriff ohne Sandbox,
- MCP-Server mit breitem Dateisystemzugriff.

## 10. CLI-Diagnosemodus

Vor der GUI wird ein CLI bereitgestellt:

```text
ai-agent-access-inspector scan --agent claude-code --format json
```

Optionen:

- alternatives Home-Verzeichnis,
- Fixture-Verzeichnis,
- JSON-Ausgabe,
- Markdown-Ausgabe,
- ausführliche Diagnosen.

Das CLI bleibt auch nach Einführung der GUI erhalten und dient als Test- und Diagnosewerkzeug.

## 11. Tauri-Kommandos

Das Frontend erhält nur klar begrenzte Kommandos:

```text
list_scanners
run_scan
get_scan_details
export_scan
open_path_in_file_manager
open_source_file
get_app_info
```

Die Kommandos sind eine dünne Schicht über `inspector-core`. Für den MVP gibt es keine Kommandos zum Ändern oder Löschen von Agentenkonfigurationen. Die Tauri-Capabilities werden auf die notwendigen lokalen Funktionen beschränkt; externe Netzwerkzugriffe sind nicht erforderlich.

## 12. Benutzeroberfläche

### 12.1 Hauptansicht

- erkannte Agenten,
- Zeitpunkt des letzten Scans,
- Anzahl der Findings,
- Anzahl der Warnungen,
- Schaltfläche „Erneut scannen“.

### 12.2 Ergebnisansicht

Die Ergebnisliste zeigt:

- Agent bzw. MCP-Server,
- Ordner bzw. Regel,
- Wirkung,
- Status,
- Zugriffstyp,
- Geltungsbereich,
- Vertrauensniveau,
- Warnungen.

Filter:

- Agent,
- Status,
- Wirkung,
- Zugriffstyp,
- Geltungsbereich,
- nur Warnungen,
- nur existierende Pfade.

### 12.3 Detailansicht

Zu jedem Finding:

- normalisierter Pfad,
- ursprünglicher Wert bzw. Regelmuster,
- Konfigurationsquelle und deren Einstufung (dokumentiert/heuristisch),
- genaue Position in der Quelle,
- Erklärung,
- Vertrauensniveau,
- Existenz und Pfadtyp,
- Symlink-Informationen,
- übergeordnete oder untergeordnete Findings,
- einschränkende oder widersprechende Regeln,
- mögliche Sicherheitsrelevanz.

Aktionen:

- Ordner im Dateimanager öffnen,
- Konfigurationsdatei öffnen,
- Pfad kopieren,
- Finding als JSON kopieren.

### 12.4 Zustände

Definierte Darstellungen für:

- kein Agent gefunden,
- Claude Code gefunden, aber keine relevanten Konfigurationen,
- Scan läuft,
- Scan teilweise erfolgreich,
- Konfiguration nicht lesbar,
- ungültige Konfiguration,
- Scan abgeschlossen,
- unerwarteter interner Fehler.

Die Bedienung ist barrierearm (Tastaturbedienung, Screenreader-taugliche Beschriftungen, ausreichende Kontraste).

## 13. Export

Der MVP unterstützt JSON und Markdown.

Der Export enthält standardmäßig die vollständigen Pfade. Vor dem Export wird eine Option angeboten:

- vollständige Pfade,
- Home-Verzeichnis als `~` darstellen,
- Benutzernamen anonymisieren.

Spätere Versionen können CSV und einen maschinenlesbaren Sicherheitsreport ergänzen.

## 14. Datenschutz und Sicherheit

Verbindliche Eigenschaften:

- keine Telemetrie im MVP,
- keine Cloud-Anbindung,
- keine automatische Übermittlung von Fehlermeldungen,
- keine Erfassung von Dateiinhalten außerhalb notwendiger Konfigurationsdateien,
- keine Speicherung vollständiger Konfigurationsdateien,
- keine Anzeige von Tokens, API-Schlüsseln oder Umgebungsgeheimnissen (betrifft insbesondere `env`-Einträge von MCP-Servern),
- sensible Werte werden beim Erzeugen von Findings und Diagnosen redigiert,
- Exporte erfolgen ausschließlich auf ausdrückliche Benutzeraktion.

Das Repository enthält `docs/privacy.md` und `SECURITY.md`.

## 15. Teststrategie

### 15.1 Unit-Tests

- Pfadnormalisierung
- Home-Auflösung und Umgebungsvariablen
- relative Pfade
- Windows-Laufwerkspfade
- UNC-Pfade
- Symlinks
- verschachtelte Verzeichnisse
- Regelmuster und Ableitung des Basispfads
- Deduplizierung
- Risikoklassifizierung
- Redigieren sensibler Werte

### 15.2 Scanner-Tests

Fixtures (ohne persönliche Daten) für:

- macOS, Linux, Windows
- mehrere Projekte
- Managed-, globale, projektbezogene und lokale Einstellungen
- widersprüchliche Regeln
- MCP-Server mit Pfadargumenten
- Sandbox aktiv/inaktiv
- nicht vorhandene Ordner
- ungültiges JSON
- unbekannte Konfigurationsfelder
- veraltete oder unbekannte Konfigurationsschlüssel (erzeugen Diagnosen, keinen Abbruch)

### 15.3 Frontend-Tests

- Darstellung von Findings
- Filter
- Detailansicht
- leere Zustände
- Warnungen
- Exportdialog
- teilweise fehlgeschlagener Scan

### 15.4 Manuelle Plattformtests

Vor dem ersten stabilen Release:

- macOS auf Apple Silicon
- Windows 11
- Ubuntu LTS
- Pfade mit Leerzeichen
- Pfade mit Umlauten und Unicode
- sehr lange Pfade
- Netzlaufwerke beziehungsweise gemountete Volumes

## 16. Continuous Integration

Bei jedem Pull Request führt `ci.yml` aus:

- Rust-Formatprüfung
- Clippy
- Rust-Tests
- TypeScript-Typprüfung (ab Phase 5)
- ESLint (ab Phase 5)
- Frontend-Tests (ab Phase 5)
- Produktionsbuild
- Abhängigkeits- und Sicherheitsprüfung (z. B. `cargo audit`/`cargo deny`, `pnpm audit`)

Betriebssystemmatrix:

```yaml
matrix:
  os:
    - macos-latest
    - windows-latest
    - ubuntu-latest
```

Rust-Tests und Builds laufen auf allen drei Systemen. Frontend-Tests müssen nicht dreimal laufen.

## 17. GitHub Releases

### 17.1 Auslöser

Ein Release wird durch einen Git-Tag ausgelöst (`v0.1.0`, `v0.2.0`, …, `v1.0.0`).

### 17.2 Artefakte

Ab `v0.1.0`: CLI-Binaries für macOS, Windows und Linux.

Ab Einführung der GUI:

- macOS Apple Silicon
- macOS Intel, solange sinnvoll
- Windows x64
- optional Windows ARM64
- Linux x64 AppImage
- Linux x64 `.deb`

### 17.3 Workflow (`release.yml`)

1. Repository auschecken
2. Rust installieren
3. Node.js und pnpm installieren (ab GUI)
4. plattformabhängige Abhängigkeiten installieren
5. Tests ausführen
6. CLI bzw. Tauri-Anwendung bauen
7. Installer erzeugen
8. Prüfsummen erzeugen
9. GitHub Release anlegen
10. Artefakte hochladen
11. Release Notes aus Changelog oder Git-Historie erzeugen

### 17.4 Signierung

Die erste Entwicklungsphase darf unsignierte Builds erzeugen. Vor einem stabilen öffentlichen Release:

- macOS-Code-Signing einrichten,
- macOS-Anwendung notarisiert veröffentlichen,
- Windows-Code-Signing-Zertifikat einrichten,
- Zertifikate ausschließlich als GitHub-Secrets hinterlegen,
- keine privaten Schlüssel im Repository speichern.

## 18. Versionen

| Version | Inhalt |
|---|---|
| `0.1.0` | CLI-Kern und Claude-Code-Scanner |
| `0.2.0` | erste Tauri-Oberfläche |
| `0.3.0` | Exporte und verbesserte Pfadanalyse |
| `0.4.0` | plattformübergreifende GitHub-Releases der GUI |
| `0.5.0` | Claude-Desktop-Scanner |
| `0.6.0` | Codex-/ChatGPT-Scanner |
| `0.7.0` | Cursor-Scanner |
| `1.0.0` | stabile Datenstruktur, signierte Releases, dokumentierte Erweiterungsschnittstelle |

Bis `1.0.0` dürfen Scanner-Schnittstelle und Exportformat inkompatibel geändert werden.

## 19. Phasen

Jede Phase endet mit einer Abnahme. Erst danach wird die nächste Phase im Detail geplant.

### Phase 0 – Grundlage

- Rust-Toolchain installieren (rustup, stable), optional `gh`
- lokalen Ordner in `ai-agent-access-inspector` umbenennen
- Git-Repository initialisieren, GitHub-Repository `rphki/ai-agent-access-inspector` anlegen
- Cargo-Workspace mit `inspector-core` und `inspector-cli`
- `LICENSE` (Apache-2.0), `README.md`, `.gitignore`, `CHANGELOG.md`
- `rustfmt`, `clippy`, erste Tests
- CI: Format, Clippy und Tests auf macOS, Windows, Linux

**Abnahme:** `cargo test` läuft lokal und in der CI auf allen drei Systemen grün.

### Phase 1 – Claude-Code-Recherche

- offizielle Dokumentation zu Settings, Permissions, MCP und Sandbox auswerten
- lokale Installation untersuchen (nur Struktur, keine Inhalte übernehmen)
- jede Quelle als dokumentiert oder heuristisch einstufen
- Prioritäten und Zusammenführung der Ebenen festhalten
- Fixture-Anforderungen ableiten

**Abnahme:** `docs/claude-code-scanner.md` dokumentiert jede zu lesende Datei und jeden ausgewerteten Schlüssel mit Quelle.

### Phase 2 – Datenmodell und Scanner-Schnittstelle

- Finding-Typen gemäß Abschnitt 6, angepasst an die Recherche
- `AgentScanner`-Trait, `ScanContext`, Diagnose- und Fehlermodell
- JSON-Serialisierung
- Fixture-Scanner und CLI-Grundgerüst (`scan --format json`)
- `docs/findings-model.md`, `docs/architecture.md`

**Abnahme:** Das CLI gibt definierte Findings eines Fixture-Scanners als JSON aus.

### Phase 3 – Pfadanalyse

- Auflösung von `~`, Umgebungsvariablen, relativen Pfaden
- Ableitung von Basispfaden aus Regelmustern
- Normalisierung, Symlinks, Existenzprüfung
- Verschachtelung, Deduplizierung, Risikowarnungen
- Windows-, UNC- und Groß-/Kleinschreibungsfälle

**Abnahme:** Alle Pfadfälle aus Abschnitt 15.1 sind automatisiert getestet und laufen auf allen drei Systemen.

### Phase 4 – Claude-Code-Scanner und CLI → `v0.1.0`

- Erkennung, Konfigurationsorte, Einlesen aller Ebenen
- Permissions, zusätzliche Verzeichnisse, MCP-Server, Modi, Sandbox
- fehlertolerantes Verhalten mit Diagnosen
- Fixtures für macOS, Linux, Windows und fehlerhafte Fälle
- CLI vollständig gemäß Abschnitt 10, inkl. Markdown-Ausgabe
- Release-Workflow für CLI-Binaries

**Abnahme:** Der Scanner liefert aus allen Fixtures reproduzierbare, erklärte Findings; ein Systemscan über das CLI funktioniert.

### Phase 5 – Tauri und Oberfläche → `v0.2.0`

- Tauri 2 mit React/TypeScript, Vite, pnpm
- Kommandos gemäß Abschnitt 11, eingeschränkte Capabilities
- Scan asynchron ausführen, Fortschritts- und Fehlerzustände übertragen
- Öffnen von Ordnern und Quelldateien
- Haupt-, Ergebnis- und Detailansicht, Filter, Zustände gemäß Abschnitt 12
- Frontend-Tests, CI um Frontend-Prüfungen erweitern

**Abnahme:** Alle Informationen eines Findings sind ohne Roh-JSON verständlich.

### Phase 6 – Export und Datenschutz → `v0.3.0`

- JSON- und Markdown-Export mit Anonymisierungsoptionen gemäß Abschnitt 13
- Secret-Redaction
- `docs/privacy.md`, `SECURITY.md`

**Abnahme:** Exporte enthalten keine erkannten API-Schlüssel oder Zugangsdaten.

### Phase 7 – Releases und Veröffentlichung → `v0.4.0`

- Release-Workflow für die GUI mit Plattformmatrix, Installern und Prüfsummen
- `docs/release-process.md`
- README mit Screenshots, Installationshinweisen und bekannten Einschränkungen
- `CONTRIBUTING.md`, Issue-Templates, `docs/adding-a-scanner.md`
- erster öffentlicher Release

**Abnahme:** Ein Git-Tag erzeugt herunterladbare Artefakte für macOS, Windows und Linux; eine außenstehende Person kann die Anwendung installieren, Findings verstehen und einen neuen Scanner vorbereiten.

### Danach

Weitere Scanner (Claude Desktop, Codex/ChatGPT, Cursor) und `1.0.0` gemäß Abschnitt 18.

## 20. Definition of Done für den MVP

Der MVP ist fertig, wenn:

- Claude Code erkannt wird,
- Managed-, globale und projektbezogene Konfigurationen untersucht werden,
- MCP-Server mit Dateisystemzugriff erfasst werden,
- gefundene Pfade und Regeln mit Quelle und Erklärung erscheinen,
- konfigurierte und potenzielle Zugriffe getrennt dargestellt werden,
- nicht vorhandene Pfade und breite Freigaben Warnungen erzeugen,
- keine Konfiguration verändert wird,
- JSON- und Markdown-Export funktionieren,
- automatische Tests vorhanden sind,
- Builds für macOS, Windows und Linux über GitHub Releases bereitstehen,
- bekannte Grenzen gut sichtbar dokumentiert sind.

## 21. Nicht Bestandteil des MVP

- automatische Änderung von Permissions
- Widerrufen von Freigaben
- Laufzeitüberwachung laufender Agentenprozesse
- Dateisystem-Tracing
- Netzwerküberwachung
- garantierter Nachweis einer echten Sandbox
- Cloud-Dashboard
- Benutzerkonto
- Telemetrie
- Auto-Update
- Plugin-System mit dynamisch geladenem Fremdcode
- vollständige Unterstützung aller Claude-Produkte
- Auswertung von Plugins und Plugin-MCP-Servern
- Auswertung von MDM-Profilen und Registry-Richtlinien (werden nur erkannt)

Diese Funktionen werden erst bewertet, wenn der reine, nachvollziehbare Konfigurationsscanner stabil funktioniert.

## 22. Offene Punkte

Derzeit keine.

## 23. Status

| Phase | Status |
|---|---|
| 0 – Grundlage | abgeschlossen (CI grün auf macOS, Windows, Linux) |
| 1 – Claude-Code-Recherche | abgeschlossen ([Spezifikation](claude-code-scanner.md); offene Prüfpunkte für Phase 4 in Abschnitt 14) |
| 2 – Datenmodell | offen |
| 3 – Pfadanalyse | offen |
| 4 – Claude-Code-Scanner und CLI | offen |
| 5 – Tauri und Oberfläche | offen |
| 6 – Export und Datenschutz | offen |
| 7 – Releases und Veröffentlichung | offen |
