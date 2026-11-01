# Spezifikation: Claude-Code-Scanner

Ergebnis von Phase 1 der [Roadmap](roadmap.md). Dieses Dokument legt fest, welche Dateien der Claude-Code-Scanner liest, welche Schlüssel er auswertet und wie er sie in Findings übersetzt.

## 1. Stand und Geltungsbereich

| | |
|---|---|
| Recherchedatum | 2026-11-01 |
| Geprüfte Dokumentation | bezieht sich auf Claude Code bis v2.1.311 |
| Lokal untersuchte Installation | Claude Code 2.1.309 (macOS, über die Claude-Desktop-App) |
| Unterstütztes Format | nur das aktuell dokumentierte (siehe Roadmap, Abschnitt 2) |

Quellen (offizielle Dokumentation):

- [Settings](https://code.claude.com/docs/en/settings)
- [Settings reference](https://code.claude.com/docs/en/settings-reference)
- [Managed settings](https://code.claude.com/docs/en/managed-settings)
- [Permissions](https://code.claude.com/docs/en/permissions)
- [MCP](https://code.claude.com/docs/en/mcp)
- [Managed MCP](https://code.claude.com/docs/en/managed-mcp)
- [Sandboxing](https://code.claude.com/docs/en/sandboxing)
- [Environment variables](https://code.claude.com/docs/en/env-vars)

### Einstufung von Quellen

- **Dokumentiert:** Datei und Schlüssel sind in der offiziellen Dokumentation beschrieben.
- **Heuristisch:** Datei oder Schlüssel ist nicht oder nur beiläufig dokumentiert, wurde aber in einer lokalen Installation beobachtet. Findings daraus erhalten höchstens `Confidence::Medium`.
- **Unbestätigt:** Angabe stammt aus einer zusammengefassten statt wörtlich gelesenen Dokumentationsstelle. Muss in Phase 4 gegen die Dokumentation oder eine Installation geprüft werden, bevor sie implementiert wird. Solche Stellen sind unten mit *(unbestätigt)* markiert.

## 2. Wie Claude Code Dateizugriff regelt

Für die Übersetzung in Findings sind diese dokumentierten Grundregeln maßgeblich:

1. **Arbeitsverzeichnis.** Claude Code darf ohne Rückfrage Dateien im Startverzeichnis und in zusätzlichen Verzeichnissen lesen. Schreiben hängt vom Permission-Modus ab.
2. **Zusätzliche Verzeichnisse** (`permissions.additionalDirectories`, `--add-dir`, `/add-dir`) werden wie das Arbeitsverzeichnis behandelt.
3. **Permission-Regeln** (`allow`, `ask`, `deny`) gelten für Claudes eigene Werkzeuge. Reihenfolge: zuerst `deny`, dann `ask`, dann `allow`. Eine Deny-Regel aus irgendeiner Ebene schlägt jede Allow-Regel.
4. **Read-/Edit-Deny-Regeln** gelten auch für erkannte Dateibefehle in Bash (`cat`, `head`, `sed`, Umleitungen), aber **nicht** für beliebige Unterprozesse (z. B. ein Python-Skript) oder Befehle ohne genannten Pfad (`grep -r muster .`). Bash-Regeln sind textbasiert und laut Dokumentation „keine Sicherheitsgrenze“.
5. **Die Sandbox** gilt nur für Shell-Befehle (Bash, PowerShell, Monitor) und deren Kindprozesse. Datei-Werkzeuge, MCP-Server und Hooks laufen außerhalb der Sandbox. Ohne Sandbox kann ein Shell-Befehl alles erreichen, was der Benutzer erreichen kann.
6. **Sandbox-Standard:** Schreiben im Arbeitsverzeichnis, im temporären Benutzerverzeichnis und in zusätzlichen Verzeichnissen; **Lesen auf dem gesamten Rechner** mit Ausnahme einzelner gesperrter Bereiche. Auf nativem Windows gibt es keine Sandbox.
7. **MCP-Server** sind eigene Prozesse mit den Rechten des Benutzers. Ihr Dateizugriff ergibt sich aus ihrem Programm und ihren Argumenten, nicht aus Claude-Code-Regeln.

Daraus folgt für den Scanner:

- Konfigurierte Pfade und Regeln → Findings mit Status `Configured`.
- Shell ohne Sandbox → ein zusammenfassendes `Potential`-Finding „effektiv Benutzerrechte“ (Roadmap 4.4).
- Shell mit Sandbox → `Potential`-Findings für die von der Sandbox erlaubten Bereiche; Lesen bleibt rechnerweit.
- Jeder MCP-Server mit lokalem Prozess → eigenes Finding mit Subjekt `McpServer`.

## 3. Konfigurationsorte

### 3.1 Settings-Dateien

| Ebene | Pfad | OS | Einstufung |
|---|---|---|---|
| Managed | `/Library/Application Support/ClaudeCode/managed-settings.json` | macOS | dokumentiert |
| Managed | `/etc/claude-code/managed-settings.json` | Linux, WSL | dokumentiert |
| Managed | `C:\Program Files\ClaudeCode\managed-settings.json` | Windows | dokumentiert |
| Managed (Drop-ins) | `managed-settings.d/*.json` im selben Verzeichnis, alphabetisch nach `managed-settings.json` zusammengeführt; versteckte Dateien und andere Endungen werden ignoriert | alle | dokumentiert |
| Benutzer | `~/.claude/settings.json` bzw. `$CLAUDE_CONFIG_DIR/settings.json` (Windows: `%USERPROFILE%\.claude\settings.json`) | alle | dokumentiert |
| Benutzer, lokal | `~/.claude/settings.local.json`, nur für Sitzungen im Home-Verzeichnis wirksam | alle | *(unbestätigt)* |
| Projekt | `<projekt>/.claude/settings.json` | alle | dokumentiert |
| Projekt, lokal | `<projekt>/.claude/settings.local.json` (bei Git-Repositories im Repository-Root, bei Worktrees im Haupt-Checkout) | alle | dokumentiert |

Hinweise:

- Der alte Windows-Pfad `C:\ProgramData\ClaudeCode\managed-settings.json` wird von Claude Code **nicht** mehr gelesen. Findet der Scanner dort eine Datei, erzeugt er eine Diagnose „veralteter Ort, nicht wirksam“, aber keine Findings.
- Managed-Dateien dürfen Symlinks sein. Zeigt ein Link außerhalb des Systemverzeichnisses, gilt die Datei trotzdem; der Scanner erzeugt eine Warnung.

### 3.2 Weitere Managed-Quellen

| Quelle | Ort | Behandlung im MVP |
|---|---|---|
| MDM, macOS | Managed-Preferences-Domäne `com.anthropic.claudecode` | erkennen und als Diagnose „Managed-Quelle vorhanden, nicht ausgewertet“ melden |
| Registry, Windows | `HKLM\SOFTWARE\Policies\ClaudeCode`, Wert `Settings` | wie MDM |
| Registry, Windows (Benutzer) | `HKCU\SOFTWARE\Policies\ClaudeCode`, Wert `Settings` | wie MDM |
| Server-managed | Admin-Konsole bzw. Gateway, beim Start geladen | lokal nicht sichtbar; bekannte Grenze |
| Policy-Helper | Schlüssel `policyHelper` (externes Programm) | nicht ausführen; Vorhandensein als Diagnose melden |
| SDK-Host | Option `managedSettings` einer einbettenden App | nicht sichtbar; bekannte Grenze |

Rangfolge der Managed-Quellen (dokumentiert): Server-managed, dann MDM/Registry HKLM, dann Managed-Dateien, dann HKCU. Standard ist `managedSourcesBehavior: "first-wins"`; mit `"merge"` werden sie kombiniert. Da der Scanner höhere Quellen nicht immer sehen kann, erhalten Findings aus Managed-Dateien einen Hinweis, dass eine höhere, unsichtbare Quelle sie überlagern kann.

### 3.3 `~/.claude.json`

Von Claude Code selbst geschriebene Datei (dokumentiert). Enthält Kontodaten, MCP-Server der Ebenen Benutzer und Lokal, projektbezogenen Zustand (z. B. Vertrauensentscheidungen) und globale Einstellungen.

| Aspekt | Festlegung | Einstufung |
|---|---|---|
| Ort | `~/.claude.json` | dokumentiert |
| Ort unter Windows | `%USERPROFILE%\.claude.json` | heuristisch |
| Ort mit `CLAUDE_CONFIG_DIR` | erst `$CLAUDE_CONFIG_DIR/.claude.json`, dann `~/.claude.json`; Diagnose, welche gelesen wurde | heuristisch |
| Gelesene Bereiche | ausschließlich `mcpServers` und `projects` | – |
| Nie gelesen oder ausgegeben | `oauthAccount`, `userID`, `machineID`, Caches und alle übrigen Schlüssel | – |

Ausgewertete Schlüssel unter `projects["<absoluter Pfad>"]`:

| Schlüssel | Bedeutung | Einstufung |
|---|---|---|
| Objektschlüssel selbst | bekannter Projektpfad | heuristisch (Struktur nur beiläufig dokumentiert) |
| `mcpServers` | MCP-Server der Ebene Lokal | dokumentiert |
| `hasTrustDialogAccepted` | Vertrauensdialog akzeptiert | dokumentiert |
| `enabledMcpjsonServers`, `disabledMcpjsonServers` | Freigabe einzelner `.mcp.json`-Server | heuristisch an diesem Ort (als Settings-Schlüssel dokumentiert) |
| `disabledMcpServers`, `enabledMcpServers` | per `/mcp` umgeschaltete Server | dokumentiert |
| `allowedTools` | ältere Ablage erlaubter Werkzeuge | heuristisch; nur Diagnose, wenn nicht leer |

Lokal beobachtete, nicht ausgewertete Projektschlüssel: `mcpContextUris`, `hasClaudeMdExternalIncludesApproved`, `hasClaudeMdExternalIncludesWarningShown`. Unbekannte Projektschlüssel erzeugen keine Diagnose, weil die Datei viele interne Felder enthält.

### 3.4 MCP-Konfiguration

| Ebene | Ort | Einstufung |
|---|---|---|
| Managed | `managed-mcp.json` im Managed-Verzeichnis (siehe 3.1); gleiches Format wie `.mcp.json`; wenn vorhanden, exklusiv | dokumentiert |
| Managed | Schlüssel `managedMcpServers` in Managed-Settings (nur `http`/`sse`) | dokumentiert |
| Lokal | `~/.claude.json` → `projects["<pfad>"].mcpServers` | dokumentiert |
| Projekt | `<projekt>/.mcp.json` → `mcpServers` | dokumentiert |
| Benutzer | `~/.claude.json` → `mcpServers` (oberste Ebene) | dokumentiert, Schlüsselpfad nur implizit |
| Plugin | `.mcp.json` im Plugin-Verzeichnis oder `mcpServers` in `plugin.json` | dokumentiert; **nicht im MVP** |

Rangfolge bei gleichem Servernamen: Lokal vor Projekt vor Benutzer vor Plugin; `managedMcpServers` über allen. `managed-mcp.json` schließt alle anderen Quellen aus.

Server-Felder: `type` (`stdio`, wenn kein `type` und keine `url`; sonst `http`, `streamable-http`, `sse`, `ws`), `command`, `args`, `env`, `url`, `headers`, `headersHelper`, `timeout`, `oauth`. Einträge mit `type: "sdk"` überspringt Claude Code; der Scanner ebenfalls.

Variablen: `${VAR}` und `${VAR:-default}` in `command`, `args`, `env`, `url`, `headers`. Der Scanner expandiert nur in `command` und `args` und nur über den `ScanContext`; nicht gesetzte Variablen bleiben wörtlich stehen und erzeugen eine Diagnose.

### 3.5 Sitzungsdaten

| Ort | Inhalt | Einstufung |
|---|---|---|
| `~/.claude/projects/<kodierter Pfad>/` bzw. `$CLAUDE_CONFIG_DIR/projects/…` | ein Verzeichnis pro Projekt mit Sitzungsprotokollen (`<uuid>.jsonl`) und `memory/` | heuristisch |
| `CLAUDE_CODE_PROJECT_DIR_NAME` | ändert den Unterordnernamen unter `projects/` | dokumentiert |

Der Verzeichnisname ersetzt Pfadtrenner durch `-`. Die Kodierung ist **nicht umkehrbar** (`ai-agent-access-inspector` und `ai/agent/access/inspector` ergeben denselben Namen). Der Scanner leitet Pfade daher nie aus dem Verzeichnisnamen ab, sondern liest aus der jeweils jüngsten `.jsonl`-Datei das Feld `cwd`:

- zeilenweise lesen, beim ersten Datensatz mit `cwd` abbrechen,
- nur `cwd` und `version` übernehmen, alle übrigen Felder sofort verwerfen,
- Ergebnis: Finding mit Status `Observed`, Confidence `Low` bis `Medium`.

## 4. Ausgewertete Settings-Schlüssel

### 4.1 Permissions

| Schlüssel | Bedeutung | Finding |
|---|---|---|
| `permissions.allow` | erlaubte Werkzeugaufrufe | `Read(…)` / `Edit(…)` → Configured, Effekt Allow |
| `permissions.ask` | Aufrufe mit Rückfrage | `Read(…)` / `Edit(…)` → Configured, Effekt Ask |
| `permissions.deny` | verbotene Aufrufe | `Read(…)` / `Edit(…)` → Configured, Effekt Deny |
| `permissions.additionalDirectories` | zusätzliche Arbeitsverzeichnisse | Configured, Effekt Allow, Zugriff Read; Write abhängig vom Modus |
| `permissions.defaultMode` | Startmodus: `default` (Alias `manual`), `acceptEdits`, `plan`, `auto`, `dontAsk`, `bypassPermissions` | Warnung bei `bypassPermissions` und `auto`; `acceptEdits` erhöht Schreibzugriff auf Arbeits- und Zusatzverzeichnisse |
| `permissions.disableBypassPermissionsMode` | `"disable"` sperrt den Bypass-Modus | entschärft die Bypass-Warnung |
| `permissions.disableAutoMode` | `"disable"` sperrt den Auto-Modus | entschärft die Auto-Warnung |
| `permissions.blockReadsOutsideWorkingDirectories` | `true`: Datei-Werkzeuge lesen nichts außerhalb der Arbeitsverzeichnisse (ab v2.1.257) | schränkt Read-Findings außerhalb ein; aus jeder Ebene wirksam |
| `allowManagedPermissionRulesOnly` | nur Managed-Regeln gelten | Regeln aus anderen Ebenen → Finding mit Hinweis „nicht wirksam“ |

Weitere Werkzeugregeln (`Bash(…)`, `WebFetch(…)`, `mcp__…`, `Agent(…)`) erzeugen keine Ordner-Findings. Ausnahmen:

- `Bash` oder `Bash(*)` in `allow` → Warnung „Shell ohne Rückfrage erlaubt“, verstärkt das Potential-Finding.
- `Bash`, `Read` oder `Edit` ohne Klammern in `deny` → Werkzeug ist vollständig entfernt; das schränkt das entsprechende Potential-Finding ein.

Regeln für `Write(…)`, `NotebookEdit(…)`, `Glob(…)` und `MultiEdit(…)` greifen laut Dokumentation nie. Der Scanner meldet sie als Diagnose „wirkungslose Regel“.

### 4.2 Sandbox

| Schlüssel | Bedeutung | Finding |
|---|---|---|
| `sandbox.enabled` | Sandbox für Shell-Befehle (Standard `false`) | bestimmt, ob das Potential-Finding „Benutzerrechte“ oder „Sandbox-Bereich“ lautet |
| `sandbox.filesystem.allowWrite` | zusätzliche beschreibbare Pfade | Potential, Effekt Allow, Zugriff Write |
| `sandbox.filesystem.denyWrite` | gesperrte Schreibpfade | Configured, Effekt Deny, Zugriff Write |
| `sandbox.filesystem.denyRead` | gesperrte Lesepfade (nicht für das Read-Werkzeug) | Configured, Effekt Deny, Zugriff Read |
| `sandbox.filesystem.allowRead` | Lesezugriff innerhalb von `denyRead` wieder öffnen | Configured, Effekt Allow, Zugriff Read |
| `sandbox.filesystem.disabled` | Dateisystem-Isolation aus (nur Benutzer-, Managed-Ebene, `--settings`) | Sandbox gilt fürs Dateisystem als aus; Warnung |
| `sandbox.excludedCommands` | Befehle außerhalb der Sandbox | Warnung je Eintrag |
| `sandbox.allowUnsandboxedCommands` | Wiederholung ohne Sandbox erlaubt (Standard `true`) | Hinweis am Sandbox-Finding |
| `sandbox.autoAllowBashIfSandboxed` | Sandbox-Befehle ohne Rückfrage (Standard `true`) | Hinweis am Sandbox-Finding |
| `sandbox.failIfUnavailable` | Start abbrechen, wenn keine Sandbox möglich | Hinweis |

Pfadsyntax der Sandbox-Schlüssel unterscheidet sich von Permission-Regeln: `/` ist absolut, `~/` relativ zum Home, `./` oder ohne Präfix relativ zum Projekt (Projekt-Settings) bzw. zu `~/.claude` (Benutzer-Settings) *(unbestätigt)*.

Auf nativem Windows meldet der Scanner `sandbox.enabled: true` mit Warnung „auf diesem System ohne Wirkung“.

### 4.3 MCP-Freigaben

| Schlüssel | Wirkung im Scanner |
|---|---|
| `enableAllProjectMcpServers` | alle Server aus `.mcp.json` gelten als freigegeben |
| `enabledMcpjsonServers` | genannte `.mcp.json`-Server gelten als freigegeben |
| `disabledMcpjsonServers` | genannte Server gelten als abgelehnt (aus jeder Ebene) |
| `allowedMcpServers`, `deniedMcpServers` | Allow-/Denylisten mit Einträgen `{"serverName"}`, `{"serverCommand"}`, `{"serverUrl"}` |
| `allowManagedMcpServersOnly` | nur Managed-Allowlist gilt |

`enableAllProjectMcpServers` und `enabledMcpjsonServers` aus `<projekt>/.claude/settings.json` gelten nur in vertrauten Projekten.

Freigabestatus eines `.mcp.json`-Servers: freigegeben, abgelehnt oder **offen** (Claude Code fragt beim Start). Offene Server erhalten Status `Unknown`.

### 4.4 Sonstige relevante Schlüssel

| Schlüssel | Behandlung |
|---|---|
| `hooks`, `disableAllHooks`, `allowManagedHooksOnly` | Hooks führen Befehle außerhalb der Sandbox aus → Warnung „Hooks konfiguriert“ mit Anzahl; Inhalte werden nicht ausgegeben |
| `env` | wird nicht gelesen (kann Geheimnisse enthalten); Ausnahme `CLAUDE_CONFIG_DIR` aus Benutzer- und Managed-Settings |
| `autoMemoryDirectory`, `plansDirectory` | vorerst nicht ausgewertet; in Phase 4 prüfen |

Alle übrigen Schlüssel werden ignoriert. Unbekannte Schlüssel innerhalb von `permissions` und `sandbox` erzeugen eine Diagnose.

## 5. Rangfolge und Zusammenführung

Rangfolge der Settings (dokumentiert, höchste zuerst):

1. Managed
2. Kommandozeile (`--settings`) – für den Scanner unsichtbar
3. Projekt, lokal (`.claude/settings.local.json`)
4. Projekt (`.claude/settings.json`)
5. Benutzer (`~/.claude/settings.json`)

Regeln für den Scanner:

- **Listen werden zusammengeführt**, nicht ersetzt (`permissions.allow`, `deny`, `ask`, `additionalDirectories`, Sandbox-Pfadlisten). Jeder Eintrag wird als eigenes Finding mit seiner Quelle geführt.
- **Einzelwerte** (`defaultMode`, `sandbox.enabled` usw.) nimmt der Scanner aus der höchsten Ebene, die sie setzt, und vermerkt überstimmte Werte.
- **Deny-Regeln** schlagen Allow-Regeln aus jeder Ebene; Managed-Deny kann durch nichts aufgehoben werden.
- **Einschränkungen für Projektdateien:**
  - `permissions.allow` und `permissions.additionalDirectories` aus `.claude/settings.json` wirken erst nach akzeptiertem Vertrauensdialog. Ist `hasTrustDialogAccepted` in `~/.claude.json` nicht `true`, erhält das Finding eine Warnung „noch nicht wirksam“.
  - `deny` und `ask` wirken sofort.
  - `defaultMode` mit `auto` oder `bypassPermissions` wirkt aus Projekt- und lokalen Settings nicht → Diagnose statt Warnung.
  - `CLAUDE_CONFIG_DIR` in Projekt- oder lokalen Settings wird ignoriert.
- **Restriktive Ausnahme:** `permissions.blockReadsOutsideWorkingDirectories: true` gilt aus jeder Ebene, auch wenn Managed `false` setzt.

## 6. Regelsyntax und Pfadauflösung

Format: `Werkzeug` oder `Werkzeug(Spezifizierer)`. `Read` und `Edit` verwenden gitignore-Muster.

| Präfix | Bedeutung | Beispiel |
|---|---|---|
| `//pfad` | absolut ab Dateisystemwurzel | `Read(//Users/alice/secrets/**)` |
| `~/pfad` | relativ zum Home-Verzeichnis | `Read(~/Documents/*.pdf)` |
| `/pfad` | relativ zur **Quelle der Einstellung** | siehe unten |
| `pfad`, `./pfad` | relativ zum aktuellen Verzeichnis | `Read(*.env)` |

Anker von `/pfad`:

| Quelle | Anker |
|---|---|
| `.claude/settings.json`, `.claude/settings.local.json` | primäres Arbeitsverzeichnis (Projekt) |
| `~/.claude/settings.json` | `~/.claude` |
| `--settings <datei>` | Verzeichnis der Datei |

Wichtige Fallstricke, für die der Scanner eine Erklärung im Finding erzeugt:

- `Read(/Users/alice/file)` ist **nicht** absolut. In Benutzer-Settings meint es `~/.claude/Users/alice/file`. Der Scanner erkennt dieses Muster (einzelner Schrägstrich, gefolgt von typischem Wurzelverzeichnis wie `Users`, `home`, `etc`) und warnt „vermutlich als absoluter Pfad gemeint“.
- Relative Muster in Allow-Regeln gelten nur ab dem Arbeitsverzeichnis (`Edit(src/**)` = `<cwd>/src/**`), in Deny-/Ask-Regeln in jeder Tiefe.
- `!muster` in Deny-/Ask-Regeln ist eine Negation, die vorhergehende relative Regeln ausnimmt.
- Unter Windows werden Pfade vor dem Abgleich in POSIX-Form gebracht; Laufwerke als `//c/…`.
- Symlinks: Allow-Regeln gelten nur, wenn Pfad **und** Ziel passen; Deny-Regeln, wenn Pfad **oder** Ziel passt.
- `.claudeignore` hat keine Wirkung und wird nicht ausgewertet.

Ableitung des Basispfads für Findings: der längste Präfix ohne Glob-Zeichen (`*`, `?`, `[`, `{`, `!`). `Read(~/projekte/**/*.md)` → Basispfad `~/projekte`, Musterart `Glob`.

Zugriffsart:

| Regel | `AccessKind` |
|---|---|
| `Read(…)` | Read |
| `Edit(…)` | Write (Edit-Regeln decken alle Datei-Schreibwerkzeuge ab) |
| `additionalDirectories` | Read, im Modus `acceptEdits`/`bypassPermissions` ReadWrite |
| `sandbox.filesystem.allowWrite` | Write |

## 7. Projektermittlung

Ein Projekt ist ein Verzeichnis, dessen Projekt-Settings und `.mcp.json` untersucht werden. Quellen in dieser Reihenfolge, dedupliziert:

1. `current_dir` aus dem `ScanContext`, falls angegeben
2. Schlüssel von `projects` in `~/.claude.json`
3. `cwd` aus `~/.claude/projects/*/` (siehe 3.5)

Bei Git-Repositories wird zusätzlich der Repository-Root auf `.claude/settings.local.json` geprüft. Nicht mehr vorhandene Projektverzeichnisse erzeugen eine Warnung, aber keinen Abbruch.

## 8. Erkennung der Installation

Claude Code gilt als **konfiguriert**, wenn mindestens eines existiert: `~/.claude/`, `~/.claude.json`, `$CLAUDE_CONFIG_DIR`, eine Managed-Datei.

Die Installation selbst (Programmdatei) wird im MVP nicht gesucht und `claude` wird **nicht ausgeführt**. Die Version wird, falls verfügbar, aus dem Feld `version` des jüngsten Sitzungsprotokolls gelesen (heuristisch).

## 9. Datenschutz beim Lesen

- Aus `~/.claude.json` nur `mcpServers` und `projects` (siehe 3.3).
- Aus Sitzungsprotokollen nur `cwd` und `version`, Abbruch nach dem ersten Treffer.
- MCP: `env`, `headers`, `headersHelper` und `oauth` werden nie ausgegeben. In `args` werden Werte redigiert, die wie Geheimnisse aussehen (Token-Präfixe, lange Zufallszeichenketten, Werte nach `--token`, `--api-key` u. ä.).
- Settings-Schlüssel `env` wird nicht gelesen.
- Belegausschnitte enthalten nur den ausgewerteten Wert, nie die umgebende Datei.

## 10. MCP-Server als Findings

Für jeden wirksamen MCP-Server:

| Servertyp | Finding |
|---|---|
| `stdio` | Subjekt `McpServer`, Status Potential, Zugriff Unknown, Erklärung „lokaler Prozess mit Benutzerrechten“ |
| `stdio` mit pfadartigen Argumenten | zusätzlich je Pfad ein Finding, Status Configured, Confidence Medium |
| bekannter Filesystem-Server (`@modelcontextprotocol/server-filesystem`) | Pfadargumente mit Zugriff ReadWrite, Confidence High |
| `http`, `sse`, `ws` | kein Ordner-Finding; Server wird in der Übersicht aufgeführt |

Pfadartig ist ein Argument, das mit `/`, `~`, `./`, `../`, einem Laufwerksbuchstaben oder `\\` beginnt und kein Schalter ist. Die Liste bekannter Server wird in Phase 4 festgelegt und bleibt bewusst kurz.

## 11. Bekannte Grenzen

Diese Punkte werden in der Anwendung und im README sichtbar dokumentiert:

- **Sitzungsbezogene Angaben** (`--add-dir`, `/add-dir`, `--settings`, `--allowedTools`, `--permission-mode`) werden nicht gespeichert und sind für den Scanner unsichtbar.
- **Server-managed Settings**, Policy-Helper und SDK-Host-Einstellungen sind lokal nicht sichtbar.
- **MDM-Profile und Registry-Werte** werden im MVP nur erkannt, nicht ausgewertet.
- **Bash-Regeln** sind keine Sicherheitsgrenze; die tatsächliche Erreichbarkeit hängt von der Sandbox ab.
- **Read-Deny-Regeln** verhindern keinen Zugriff durch beliebige Unterprozesse.
- **MCP-Server** können unabhängig von ihren Argumenten auf alles zugreifen, was der Benutzer erreicht.
- **Plugins** (inkl. Plugin-MCP-Server) werden im MVP nicht ausgewertet.
- **Heuristische Quellen** (`~/.claude.json`-Projektliste, Sitzungsprotokolle) können sich ohne Ankündigung ändern.

## 12. Fixture-Anforderungen

Für Phase 4 werden unter `fixtures/claude-code/` mindestens benötigt:

| Fixture | Inhalt |
|---|---|
| `macos/basic` | Benutzer-Settings, ein Projekt mit Projekt- und lokalen Settings, `~/.claude.json` mit einem Projekt |
| `linux/basic` | wie macOS, Linux-Pfade, Managed-Datei unter `/etc/claude-code` |
| `windows/basic` | Windows-Pfade, Laufwerks- und `//c/`-Muster, Managed-Pfad unter `Program Files`, alter `ProgramData`-Pfad |
| `managed/drop-ins` | `managed-settings.json` plus `managed-settings.d/` mit mehreren Dateien und einer versteckten Datei |
| `rules/anchors` | alle vier Präfixe, `/Users/…`-Fallstrick, Negation, Glob-Basispfade |
| `rules/conflicts` | Allow und Deny auf denselben Pfad aus verschiedenen Ebenen |
| `modes/bypass` | `defaultMode: bypassPermissions` in Benutzer- und Projekt-Settings, mit und ohne `disableBypassPermissionsMode` |
| `sandbox/on` und `sandbox/off` | Sandbox mit `allowWrite`, `denyRead`, `excludedCommands`; ohne Sandbox |
| `mcp/scopes` | Server in allen Ebenen, Namenskonflikt, Filesystem-Server, Server mit `env`-Geheimnis |
| `mcp/approval` | `.mcp.json` mit freigegebenen, abgelehnten und offenen Servern |
| `trust/untrusted` | Projekt mit `allow`-Regeln ohne akzeptierten Vertrauensdialog |
| `sessions/encoded` | `projects/`-Verzeichnis mit mehrdeutigem Namen und `cwd` in der `.jsonl` |
| `config-dir` | `CLAUDE_CONFIG_DIR` gesetzt, `.claude.json` an beiden möglichen Orten |
| `malformed/*` | ungültiges JSON, abgeschnittene Datei, falsche Typen, unbekannte Schlüssel, nicht lesbare Datei |

Fixtures enthalten nur erfundene Pfade und Werte.

## 13. Folgen für das Datenmodell (Phase 2)

- `FindingSubject` (Agent / MCP-Server) und `RuleEffect` (Allow / Ask / Deny) werden bestätigt.
- Zusätzlich nötig: ein Kennzeichen **„nicht wirksam“** mit Grund (Vertrauensdialog fehlt, nur Managed-Regeln erlaubt, Ebene darf Schlüssel nicht setzen, wirkungslose Regel). Statt eines neuen Status wird das als Warnung mit eigenem Typ modelliert.
- `FindingSource` braucht den **Anker** für relative Muster (Projekt, `~/.claude`, Datei-Verzeichnis).
- `AccessKind` reicht aus; `additionalDirectories` hängt zusätzlich vom Modus ab, das wird in der Erklärung festgehalten.
- Eine **Scan-Zusammenfassung** pro Agent (Modus, Sandbox an/aus, Anzahl MCP-Server, nicht ausgewertete Managed-Quellen) ergänzt die Finding-Liste.

## 14. Offene Prüfpunkte für Phase 4

Vor der Implementierung gegen Dokumentation oder Installation zu verifizieren:

- `~/.claude/settings.local.json` und seine Gültigkeit *(unbestätigt)*
- Pfadsyntax der Sandbox-Schlüssel *(unbestätigt)*
- Ort von `.claude.json` bei gesetztem `CLAUDE_CONFIG_DIR`
- genaue Typen von `enabledMcpjsonServers` / `disabledMcpjsonServers` (vermutlich Liste von Servernamen)
- vollständige Liste der Schlüssel, die nur in Managed-Settings wirken
