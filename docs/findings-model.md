# Datenmodell

Dieses Dokument beschreibt das agentenunabhängige Datenmodell in `crates/inspector-core/src/model/` und das JSON-Format, in dem es ausgegeben wird. Bis Version 1.0 kann sich das Format inkompatibel ändern; die Version steht im Feld `formatVersion` jedes Berichts.

## 1. JSON-Konventionen

- Feldnamen in `camelCase` (`agentId`, `basePath`).
- Aufzählungswerte in `camelCase` (`readWrite`, `projectLocal`, `notChecked`). Ausnahme: Betriebssystem `macos`, `linux`, `windows`.
- Varianten mit Daten tragen ein Unterscheidungsfeld: `kind` bei `subject` und `anchor`, `type` bei Warnungsarten.
- Nicht vorhandene Werte werden als `null` ausgegeben, nicht weggelassen. So hat jedes Objekt immer dieselben Felder.
- Pfade sind Zeichenketten im Format des gescannten Systems.

## 2. Aufbau eines Berichts

```text
ScanReport
├── formatVersion        "0.1"
├── tool                 { name, version }
├── operatingSystem      macos | linux | windows
└── results[]            ein ScanResult pro Scanner
    ├── scanner          { id, name, description, parserVersion }
    ├── detection        { status, evidence[], version }
    ├── findings[]       Finding
    ├── diagnostics[]    Diagnostic
    └── summary[]        { key, label, value }
```

- `detection.status`: `notDetected` oder `configured` (Konfiguration gefunden; das Programm selbst wird nicht gesucht).
- `summary` enthält Kernaussagen zur Konfiguration eines Agenten, z. B. Permission-Modus oder ob eine Sandbox aktiv ist. Schlüssel und Bezeichnungen legt der jeweilige Scanner fest.

## 3. Finding

Ein Finding ist ein Pfad oder eine Regel, die Dateizugriff für einen Agenten gewährt, einschränkt oder nahelegt.

| Feld | Bedeutung |
|---|---|
| `id` | Stabile Kennung: `<agentId>-<16 Hex-Zeichen>`, berechnet aus Agent, Subjekt, Quelldatei, JSON-Pointer und Rohwert. Unveränderte Konfiguration ergibt bei jedem Scan dieselbe Kennung. |
| `agentId`, `agentName` | Scanner, der das Finding erzeugt hat |
| `subject` | Wer den Zugriff hat: `{ "kind": "agent" }` oder `{ "kind": "mcpServer", "name": "…" }` |
| `target` | Pfad bzw. Regel, siehe 3.1 |
| `effect` | `allow`, `ask`, `deny` oder `notApplicable` (kein Regelursprung, z. B. beobachteter Projektpfad) |
| `access` | `read`, `write`, `execute`, `readWrite`, `unknown` |
| `status` | `configured`, `observed`, `potential`, `blocked`, `unknown` (Bedeutung siehe Roadmap 4.1) |
| `scope` | `managed`, `user`, `project`, `projectLocal`, `session`, `runtime`, `operatingSystem` |
| `source` | Herkunft, siehe 3.2 |
| `explanation` | Für Menschen verständliche Erklärung |
| `confidence` | `low`, `medium`, `high` |
| `pathState` | Zustand auf dem Datenträger, siehe 3.3 |
| `warnings` | Sicherheitsrelevante Hinweise, siehe 3.4 |

### 3.1 Target

| Feld | Bedeutung |
|---|---|
| `raw` | Wert wie in der Quelle, z. B. `Read(~/projekt/**)` |
| `patternKind` | `exactPath`, `directory`, `glob`, `everything` (alles, was das Subjekt erreicht, z. B. Shell ohne Sandbox), `unparsed` |
| `basePath` | Pfad, an dem die Regel verankert ist; `null`, wenn nicht bestimmbar |
| `displayPath` | Pfad für die Anzeige |

### 3.2 Source

| Feld | Bedeutung |
|---|---|
| `file` | Gelesene Datei; `null` bei abgeleiteten Findings |
| `configKind` | Art der Konfiguration, z. B. `claude-code/settings` |
| `documentation` | `documented` (offiziell beschrieben), `heuristic` (beobachtet, nicht oder kaum dokumentiert), `derived` (vom Inspector abgeleitet) |
| `pointer` | JSON-Pointer zum Wert in `file`, z. B. `/permissions/deny/0` |
| `rawValue` | Wert vor der Normalisierung |
| `anchor` | Bezugspunkt relativer Muster: `filesystemRoot`, `home`, `workingDirectory` oder `{ "kind": "directory", "path": "…" }` |
| `parserVersion` | Version der Parserlogik des Scanners |
| `excerpt` | Kurzer Beleg ohne Geheimnisse |

### 3.3 PathState

Wird von der Pfadanalyse (Phase 3) gefüllt; bis dahin `notChecked`.

| Feld | Werte |
|---|---|
| `existence` | `exists`, `missing`, `unknown` (Prüfung fehlgeschlagen), `notChecked` |
| `kind` | `file`, `directory`, `other`, `unknown` |
| `symlinkTarget` | Ziel, falls der Pfad ein Symlink ist |

### 3.4 Warning

```json
{ "kind": { "type": "notEffective", "reason": "trustNotAccepted" }, "severity": "info", "message": "…" }
```

| `kind.type` | Bedeutung |
|---|---|
| `homeDirectory` | Freigabe des gesamten Home-Verzeichnisses |
| `filesystemRoot` | Freigabe der Dateisystemwurzel |
| `wholeDrive` | Freigabe eines ganzen Laufwerks |
| `broadPath` | sonstiger besonders breiter Pfad |
| `missingPath` | Pfad existiert nicht |
| `symlinkElsewhere` | Symlink zeigt in einen anderen Bereich |
| `conflictingRules` | widersprüchliche Allow-/Deny-Regeln |
| `dangerousMode` | gefährlicher Permission-Modus |
| `unsandboxedShell` | Shell-Befehle ohne Sandbox |
| `likelyMisconfigured` | vermutlicher Konfigurationsfehler, z. B. falsch verankerter Pfad |
| `notEffective` | Konfiguration vorhanden, aber nicht wirksam; Grund in `reason` |

Gründe für `notEffective`: `trustNotAccepted`, `managedRulesOnly`, `scopeNotAllowed`, `ruleNeverMatches`, `overriddenByHigherScope`, `platformUnsupported`.

`severity`: `info`, `low`, `medium`, `high`.

„Nicht wirksam“ ist bewusst eine Warnung und kein eigener Status: Das Finding bleibt `configured`, weil es so in der Konfiguration steht, und die Warnung erklärt, warum es derzeit nicht greift.

## 4. Diagnostic

Diagnosen beschreiben Probleme beim Scannen. Sie brechen den Scan nie ab.

| Feld | Bedeutung |
|---|---|
| `level` | `info`, `warning`, `error` |
| `code` | `unreadableFile`, `invalidJson`, `unexpectedType`, `unknownKey`, `unparsableRule`, `unresolvedVariable`, `deprecatedLocation`, `unevaluatedSource`, `internal`, `other` |
| `message` | Beschreibung |
| `file`, `pointer` | betroffene Datei und Stelle, sofern bekannt |

Ein Scanner, der unerwartet abstürzt, erzeugt eine Diagnose mit Code `internal`; die übrigen Scanner laufen weiter.

## 5. Beispiel

Ausgabe von `ai-agent-access-inspector scan --agent demo --home /home/alice` (gekürzt):

```json
{
  "formatVersion": "0.1",
  "tool": { "name": "ai-agent-access-inspector", "version": "0.1.0" },
  "operatingSystem": "macos",
  "results": [
    {
      "scanner": { "id": "demo", "name": "Demo", "description": "…", "parserVersion": "1" },
      "detection": { "status": "configured", "evidence": [], "version": null },
      "findings": [
        {
          "id": "demo-de0a8d3dac098aed",
          "agentId": "demo",
          "agentName": "Demo",
          "subject": { "kind": "agent" },
          "target": {
            "raw": "~/projekte",
            "patternKind": "directory",
            "basePath": "/home/alice/projekte",
            "displayPath": "/home/alice/projekte"
          },
          "effect": "allow",
          "access": "readWrite",
          "status": "configured",
          "scope": "user",
          "source": {
            "file": "/home/alice/.demo/settings.json",
            "configKind": "demo/settings",
            "documentation": "documented",
            "pointer": "/permissions/additionalDirectories/0",
            "rawValue": "~/projekte",
            "anchor": null,
            "parserVersion": "1",
            "excerpt": null
          },
          "explanation": "Additional working directory: …",
          "confidence": "high",
          "pathState": { "existence": "notChecked", "kind": "unknown", "symlinkTarget": null },
          "warnings": []
        }
      ],
      "diagnostics": [
        { "level": "info", "code": "other", "message": "Demo scanner: all values are examples.", "file": null, "pointer": null }
      ],
      "summary": [
        { "key": "permissionMode", "label": "Permission mode", "value": "default" },
        { "key": "sandbox", "label": "Sandbox", "value": "off" }
      ]
    }
  ]
}
```
