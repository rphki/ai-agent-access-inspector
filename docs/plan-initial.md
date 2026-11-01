> **Historisch.** Dies ist der ursprüngliche Umsetzungsplan. Maßgeblich ist inzwischen die [Roadmap](roadmap.md).

# Umsetzungsplan: AI Agent Access Inspector

## 1. Ziel

Eine lokale, plattformübergreifende Desktop-Anwendung zeigt, welche Ordner laut Konfigurationen, Projektdateien und erkennbaren Einstellungen für lokale KI-Agenten freigegeben oder potenziell erreichbar sind.

Der erste unterstützte Agent ist **Claude Code**. Die Architektur wird so aufgebaut, dass später Claude Desktop, Codex, ChatGPT, Cursor und weitere Anwendungen als unabhängige Scanner ergänzt werden können.

Die Anwendung:

- läuft unter macOS, Windows und Linux,
- arbeitet vollständig lokal,
- benötigt kein Benutzerkonto und kein Backend,
- verändert keine Agentenkonfiguration,
- überträgt keine Dateipfade,
- erklärt zu jedem Fund, woher die Information stammt,
- unterscheidet zwischen konfiguriertem und nur potenziellem Zugriff.

## 2. Technologiestack

- **Desktop-Framework:** Tauri 2
- **Backend und Scanner:** Rust
- **Frontend:** React mit TypeScript
- **Build-System:** Vite
- **Paketmanager:** pnpm
- **Tests Backend:** Rust Unit- und Integrationstests
- **Tests Frontend:** Vitest und React Testing Library
- **End-to-End-Tests:** nach dem MVP
- **CI/CD:** GitHub Actions
- **Veröffentlichung:** GitHub Releases
- **Lizenz:** Apache-2.0 oder MIT; bevorzugt Apache-2.0
- **Repository:** öffentliches GitHub-Repository

## 3. Produktbegriffe

Die Anwendung verwendet bewusst nicht pauschal den Begriff „zugreifbar“. Jeder gefundene Pfad erhält einen Status:

- **Configured:** Der Pfad steht ausdrücklich in einer Konfiguration.
- **Observed:** Der Pfad wurde in einer gespeicherten Projekt- oder Sitzungsinformation gefunden.
- **Potential:** Der Pfad könnte aufgrund des Prozesskontexts oder der Betriebssystemrechte erreichbar sein.
- **Blocked:** Der Pfad ist konfiguriert, aber gegenwärtig nicht erreichbar.
- **Unknown:** Es liegen nicht genügend Informationen für eine belastbare Bewertung vor.

Zusätzlich wird die Geltungsebene ausgewiesen:

- Benutzerweit
- Projektbezogen
- Lokal im Projekt
- Sitzungsbezogen
- Laufzeitbezogen
- Betriebssystembezogen

## 4. Repository-Struktur

```text
ai-agent-access-inspector/
├── .github/
│   ├── workflows/
│   │   ├── ci.yml
│   │   └── release.yml
│   └── ISSUE_TEMPLATE/
├── docs/
│   ├── architecture.md
│   ├── findings-model.md
│   ├── adding-a-scanner.md
│   ├── privacy.md
│   └── release-process.md
├── fixtures/
│   └── claude-code/
│       ├── macos/
│       ├── linux/
│       ├── windows/
│       └── malformed/
├── src/
│   ├── components/
│   ├── pages/
│   ├── hooks/
│   ├── types/
│   └── App.tsx
├── src-tauri/
│   ├── src/
│   │   ├── commands/
│   │   ├── model/
│   │   ├── platform/
│   │   ├── scanners/
│   │   │   └── claude_code/
│   │   ├── analysis/
│   │   ├── export/
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── capabilities/
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
├── pnpm-lock.yaml
├── README.md
├── CONTRIBUTING.md
├── SECURITY.md
├── LICENSE
└── CHANGELOG.md
```

## 5. Gemeinsames Datenmodell

Der Rust-Kern stellt ein agentenunabhängiges Modell bereit.

```rust
pub struct Finding {
    pub id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub path: PathBuf,
    pub display_path: String,
    pub access: AccessKind,
    pub status: FindingStatus,
    pub scope: FindingScope,
    pub source: FindingSource,
    pub explanation: String,
    pub confidence: Confidence,
    pub path_state: PathState,
    pub warnings: Vec<Warning>,
}
```

Benötigte Typen:

```rust
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

Die Quelle eines Funds enthält:

- Pfad der Konfigurationsdatei,
- Konfigurationstyp,
- JSON-Schlüssel beziehungsweise JSON Pointer,
- ursprünglichen, noch nicht normalisierten Wert,
- Parser-Version,
- optional einen kurzen Belegausschnitt ohne sensible Inhalte.

## 6. Scanner-Schnittstelle

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

## 7. Scan-Kontext und Testbarkeit

Scanner dürfen im Test nicht direkt vom echten Home-Verzeichnis abhängig sein.

```rust
pub struct ScanContext {
    pub home_dir: PathBuf,
    pub current_dir: Option<PathBuf>,
    pub config_dirs: PlatformConfigDirs,
    pub operating_system: OperatingSystem,
}
```

Dadurch können alle Plattformvarianten auf jedem Entwicklungssystem mit Fixtures getestet werden.

Das Backend unterstützt intern zwei Modi:

- **Systemscan:** verwendet die tatsächlichen Verzeichnisse des Benutzers.
- **Fixture-Scan:** verwendet ein kontrolliertes Testverzeichnis.

## 8. Claude-Code-Scanner

### 8.1 Recherche und Spezifikation

Vor der Implementierung werden die aktuellen Claude-Code-Konfigurationsorte und Konfigurationsebenen aus offizieller Dokumentation und überprüfbaren lokalen Installationen erfasst.

Die Ergebnisse werden in `docs/claude-code-scanner.md` dokumentiert:

- unterstützte Claude-Code-Versionen,
- Konfigurationsorte pro Betriebssystem,
- Benutzer-, Projekt- und lokale Konfiguration,
- zusätzliche Verzeichnisse,
- Allow-/Deny-Regeln,
- gefährliche Permission-Modi,
- gespeicherte Projekt- oder Sitzungsinformationen,
- bekannte Grenzen des Scanners.

Undokumentierte Dateien werden ausdrücklich als heuristische Quelle gekennzeichnet.

### 8.2 Zu untersuchende Quellen

Der Scanner untersucht mindestens:

- Benutzerkonfiguration von Claude Code,
- `.claude/settings.json`,
- `.claude/settings.local.json`,
- bekannte Claude-Projektinformationen,
- zusätzliche Verzeichnisse,
- dateisystembezogene Allow-/Deny-Regeln,
- Einstellungen zum Überspringen von Permission-Abfragen,
- gespeicherte Projektpfade, sofern zuverlässig interpretierbar.

`CLAUDE.md` wird nicht automatisch als Ordnerfreigabe gewertet. Enthaltene Pfade dürfen höchstens später als unverifizierte Referenzen ausgewiesen werden.

### 8.3 Verhalten

Der Scanner:

1. erkennt, ob Claude Code installiert oder konfiguriert ist,
2. sucht globale Konfigurationen,
3. ermittelt bekannte Projekte,
4. untersucht deren Projektkonfigurationen,
5. extrahiert relevante Pfade und Regeln,
6. normalisiert die Pfade,
7. prüft Existenz und grundlegende Dateisystemrechte,
8. erzeugt Findings und Warnungen,
9. fasst inhaltlich identische Ergebnisse zusammen.

### 8.4 Fehlerbehandlung

Die Anwendung darf bei folgenden Fällen nicht abbrechen:

- ungültiges JSON,
- teilweise geschriebene Konfigurationsdateien,
- nicht lesbare Dateien,
- nicht vorhandene Pfade,
- unbekannte Konfigurationsschlüssel,
- Symlink-Schleifen,
- unterschiedliche Groß-/Kleinschreibung,
- ungültige Windows- oder UNC-Pfade.

Stattdessen wird ein Diagnoseeintrag erzeugt.

## 9. Pfadanalyse

Eine gemeinsame Analysekomponente übernimmt:

- Auflösung von `~`,
- Auflösung unterstützter Umgebungsvariablen,
- Umwandlung relativer in absolute Pfade,
- lexikalische Normalisierung,
- optionale kanonische Auflösung,
- Erkennung von Symlinks,
- Prüfung, ob ein Pfad existiert,
- Prüfung, ob er Datei oder Verzeichnis ist,
- Erkennung verschachtelter Freigaben,
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
- aktivierte gefährliche Permission-Modi.

## 10. Tauri-Kommandos

Das Frontend erhält nur klar begrenzte Tauri-Kommandos:

```text
list_scanners
run_scan
get_scan_details
export_scan
open_path_in_file_manager
open_source_file
get_app_info
```

Für den MVP gibt es keine Kommandos zum Ändern oder Löschen von Agentenkonfigurationen.

Die Tauri-Capabilities werden auf die notwendigen lokalen Funktionen beschränkt. Externe Netzwerkzugriffe sind nicht erforderlich.

## 11. Benutzeroberfläche

### 11.1 Hauptansicht

Die Hauptansicht enthält:

- erkannte Agenten,
- Zeitpunkt des letzten Scans,
- Anzahl der Findings,
- Anzahl der Warnungen,
- Schaltfläche „Erneut scannen“.

### 11.2 Ergebnisansicht

Die Ergebnisliste zeigt:

- Agent,
- Ordner,
- Status,
- Zugriffstyp,
- Geltungsbereich,
- Vertrauensniveau,
- Warnungen.

Filter:

- Agent,
- Status,
- Zugriffstyp,
- Geltungsbereich,
- nur Warnungen,
- nur existierende Pfade.

### 11.3 Detailansicht

Zu jedem Finding werden angezeigt:

- normalisierter Pfad,
- ursprünglicher Wert,
- Konfigurationsquelle,
- genaue Position in der Quelle,
- Erklärung,
- Vertrauensniveau,
- Existenz und Pfadtyp,
- Symlink-Informationen,
- übergeordnete oder untergeordnete Findings,
- mögliche Sicherheitsrelevanz.

Aktionen:

- Ordner im Dateimanager öffnen,
- Konfigurationsdatei öffnen,
- Pfad kopieren,
- Finding als JSON kopieren.

### 11.4 Zustände

Die Oberfläche benötigt definierte Darstellungen für:

- kein Agent gefunden,
- Claude gefunden, aber keine relevanten Konfigurationen,
- Scan läuft,
- Scan teilweise erfolgreich,
- Konfiguration nicht lesbar,
- ungültige Konfiguration,
- Scan abgeschlossen,
- unerwarteter interner Fehler.

## 12. Export

Der MVP unterstützt:

- JSON
- Markdown

Der Export enthält standardmäßig die vollständigen Pfade. Vor dem Export wird eine Option angeboten:

- vollständige Pfade,
- Home-Verzeichnis als `~` darstellen,
- Benutzernamen anonymisieren.

Eine spätere Version kann CSV und einen maschinenlesbaren Sicherheitsreport ergänzen.

## 13. Datenschutz und Sicherheit

Verbindliche Eigenschaften:

- keine Telemetrie im MVP,
- keine Cloud-Anbindung,
- keine automatische Übermittlung von Fehlermeldungen,
- keine Erfassung von Dateiinhalten außerhalb notwendiger Konfigurationsdateien,
- keine Speicherung vollständiger Konfigurationsdateien,
- keine Anzeige von Tokens, API-Schlüsseln oder Umgebungsgeheimnissen,
- sensible Werte werden beim Erzeugen von Diagnosen redigiert,
- Exporte erfolgen ausschließlich auf ausdrückliche Benutzeraktion.

Das Repository enthält `docs/privacy.md` und `SECURITY.md`.

## 14. Teststrategie

### Unit-Tests

- Pfadnormalisierung
- Home-Auflösung
- relative Pfade
- Windows-Laufwerkspfade
- UNC-Pfade
- Symlinks
- verschachtelte Verzeichnisse
- Deduplizierung
- Risikoklassifizierung
- Redigieren sensibler Werte

### Scanner-Tests

Fixtures für:

- macOS
- Linux
- Windows
- mehrere Projekte
- globale und lokale Einstellungen
- widersprüchliche Regeln
- nicht vorhandene Ordner
- ungültiges JSON
- unbekannte Konfigurationsfelder
- ältere und neuere Konfigurationsvarianten

### Frontend-Tests

- Darstellung von Findings
- Filter
- Detailansicht
- leere Zustände
- Warnungen
- Exportdialog
- teilweise fehlgeschlagener Scan

### Manuelle Plattformtests

Vor dem ersten stabilen Release:

- macOS auf Apple Silicon
- Windows 11
- Ubuntu LTS
- Pfade mit Leerzeichen
- Pfade mit Umlauten und Unicode
- sehr lange Pfade
- Netzlaufwerke beziehungsweise gemountete Volumes

## 15. Continuous Integration

Bei jedem Pull Request führt `ci.yml` aus:

- Rust-Formatprüfung
- Clippy
- Rust-Tests
- TypeScript-Typprüfung
- ESLint
- Frontend-Tests
- Produktionsbuild
- Abhängigkeits- und Sicherheitsprüfung

Die Tests laufen als Betriebssystemmatrix auf:

```yaml
matrix:
  os:
    - macos-latest
    - windows-latest
    - ubuntu-latest
```

Nicht jeder Frontend-Test muss dreimal laufen. Plattformabhängige Rust- und Buildtests werden dagegen auf allen drei Systemen ausgeführt.

## 16. GitHub Releases

### Release-Auslöser

Ein Release wird durch einen Git-Tag ausgelöst:

```text
v0.1.0
v0.2.0
v1.0.0
```

### Erzeugte Artefakte

- macOS Apple Silicon
- macOS Intel, solange sinnvoll
- Windows x64
- optional Windows ARM64
- Linux x64 AppImage
- Linux x64 `.deb`

### Release-Workflow

`release.yml`:

1. Repository auschecken
2. Rust installieren
3. Node.js und pnpm installieren
4. Plattformabhängige Abhängigkeiten installieren
5. Tests ausführen
6. Tauri-Anwendung bauen
7. Installer erzeugen
8. Prüfsummen erzeugen
9. GitHub Release anlegen
10. Artefakte hochladen
11. Release Notes aus Changelog oder Git-Historie erzeugen

### Signierung

Die erste Entwicklungsphase darf unsignierte Builds erzeugen.

Vor einem stabilen öffentlichen Release:

- macOS-Code-Signing einrichten,
- macOS-Anwendung notarisiert veröffentlichen,
- Windows-Code-Signing-Zertifikat einrichten,
- Zertifikate ausschließlich als GitHub-Secrets hinterlegen,
- keine privaten Schlüssel im Repository speichern.

## 17. Versions- und Release-Strategie

- `0.1.0`: CLI-Kern und Claude-Code-Scanner
- `0.2.0`: erste Tauri-Oberfläche
- `0.3.0`: Exporte und verbesserte Pfadanalyse
- `0.4.0`: plattformübergreifende GitHub-Releases
- `0.5.0`: Claude-Desktop-Scanner
- `0.6.0`: Codex-/ChatGPT-Scanner
- `0.7.0`: Cursor-Scanner
- `1.0.0`: stabile Datenstruktur, signierte Releases und dokumentierte Erweiterungsschnittstelle

Bis Version `1.0.0` dürfen Scanner-Schnittstelle und Exportformat noch inkompatibel geändert werden.

## 18. Arbeitspakete

### Arbeitspaket 1: Projektgrundlage

- Git-Repository initialisieren
- Tauri-2-Projekt mit React und TypeScript erstellen
- Rust-Modulstruktur anlegen
- Lizenz und Basisdokumentation hinzufügen
- Formatierung, Linting und Tests konfigurieren
- grundlegende CI einrichten

**Abnahme:** Leere Tauri-Anwendung lässt sich auf allen drei Betriebssystemen bauen.

### Arbeitspaket 2: Datenmodell

- gemeinsame Finding-Typen implementieren
- Scanner-Schnittstelle definieren
- ScanContext einführen
- Diagnose- und Fehlermodell definieren
- JSON-Serialisierung implementieren
- Dokumentation des Datenmodells schreiben

**Abnahme:** Fixture-Scanner kann definierte Findings an Frontend oder CLI liefern.

### Arbeitspaket 3: Pfadanalyse

- Normalisierung implementieren
- Existenzprüfung implementieren
- Symlink-Analyse implementieren
- Verschachtelung und Deduplizierung implementieren
- Risikowarnungen implementieren
- Plattform-Fixtures und Tests ergänzen

**Abnahme:** Alle definierten Pfadfälle funktionieren automatisiert unter macOS, Windows und Linux.

### Arbeitspaket 4: Claude-Code-Recherche

- offizielle Konfigurationsquellen erfassen
- lokale Beispielinstallationen untersuchen
- Quellen nach dokumentiert und heuristisch klassifizieren
- Fixtures ohne persönliche Daten erstellen
- Scanner-Spezifikation dokumentieren

**Abnahme:** Jede zu lesende Datei und jeder ausgewertete Schlüssel ist dokumentiert.

### Arbeitspaket 5: Claude-Code-Scanner

- Installation erkennen
- Konfigurationsdateien lokalisieren
- globale Einstellungen lesen
- Projekte und Projekteinstellungen erkennen
- zusätzliche Verzeichnisse extrahieren
- Permissions und Warnungen übersetzen
- beschädigte Konfigurationen tolerieren
- Fixture-Tests implementieren

**Abnahme:** Der Scanner liefert aus allen Fixtures reproduzierbare, erklärte Findings.

### Arbeitspaket 6: CLI-Diagnosemodus

Vor der GUI wird ein interner CLI- oder Debug-Modus bereitgestellt:

```text
ai-agent-access-inspector scan --agent claude-code --format json
```

Optionen:

- alternatives Home-Verzeichnis,
- Fixture-Verzeichnis,
- JSON-Ausgabe,
- Markdown-Ausgabe,
- ausführliche Diagnosen.

**Abnahme:** Der Claude-Code-Scanner kann unabhängig von der GUI vollständig getestet werden.

### Arbeitspaket 7: Tauri-Integration

- Tauri-Kommandos implementieren
- Scan asynchron ausführen
- Fortschritts- und Fehlerzustände übertragen
- Capabilities einschränken
- Öffnen von Ordnern und Quelldateien implementieren

**Abnahme:** Die Oberfläche kann einen Scan starten und strukturierte Ergebnisse empfangen.

### Arbeitspaket 8: Benutzeroberfläche

- Hauptansicht erstellen
- Ergebnisliste erstellen
- Filter implementieren
- Detailansicht erstellen
- Warnungen gestalten
- leere und fehlerhafte Zustände ergänzen
- barrierearme Bedienung sicherstellen

**Abnahme:** Alle Informationen eines Findings sind ohne Zugriff auf Roh-JSON verständlich.

### Arbeitspaket 9: Export und Datenschutz

- JSON-Export
- Markdown-Export
- Pfadanonymisierung
- Secret-Redaction
- Privacy-Dokumentation
- Security-Dokumentation

**Abnahme:** Exporte enthalten keine erkannten API-Schlüssel oder Zugangsdaten.

### Arbeitspaket 10: Release-Automatisierung

- Release-Workflow erstellen
- Plattformmatrix konfigurieren
- Installer und Prüfsummen erzeugen
- GitHub Releases automatisch befüllen
- Releaseprozess dokumentieren
- Testrelease `v0.1.0` durchführen

**Abnahme:** Ein Git-Tag erzeugt herunterladbare Artefakte für macOS, Windows und Linux.

### Arbeitspaket 11: Öffentliche Veröffentlichung

- README mit Screenshots
- Installationshinweise
- bekannte Einschränkungen
- Beitragsrichtlinien
- Issue-Templates
- Scanner-Erweiterungsanleitung
- Roadmap
- erster öffentlicher Release

**Abnahme:** Eine außenstehende Person kann die Anwendung installieren, Findings verstehen und einen neuen Scanner vorbereiten.

## 19. Definition of Done für den ersten MVP

Der erste MVP ist fertig, wenn:

- Claude Code erkannt wird,
- globale und projektbezogene Konfigurationen untersucht werden,
- gefundene Pfade mit Quelle und Erklärung erscheinen,
- konfigurierte und potenzielle Zugriffe getrennt dargestellt werden,
- nicht vorhandene Pfade und breite Freigaben Warnungen erzeugen,
- keine Konfiguration verändert wird,
- JSON- und Markdown-Export funktionieren,
- automatische Tests vorhanden sind,
- Builds für macOS, Windows und Linux über GitHub Releases bereitstehen,
- bekannte Grenzen gut sichtbar dokumentiert sind.

## 20. Nicht Bestandteil des ersten MVP

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

Diese Funktionen werden erst bewertet, wenn der reine, nachvollziehbare Konfigurationsscanner stabil funktioniert.