# Data model

This document describes the agent-independent data model in `crates/inspector-core/src/model/` and the JSON format it is serialised to. Until version 1.0 the format may change incompatibly; the version is in the `formatVersion` field of every report.

## 1. JSON conventions

- Field names in `camelCase` (`agentId`, `basePath`).
- Enum values in `camelCase` (`readWrite`, `projectLocal`, `notChecked`). Exception: operating system `macos`, `linux`, `windows`.
- Variants with data carry a discriminator field: `kind` for `subject` and `anchor`, `type` for warning kinds.
- Missing values are written as `null`, not omitted, so every object always has the same fields.
- Paths are strings in the format of the scanned system.

## 2. Report structure

```text
ScanReport
├── formatVersion        "0.1"
├── tool                 { name, version }
├── operatingSystem      macos | linux | windows
└── results[]            one ScanResult per scanner
    ├── scanner          { id, name, description, parserVersion }
    ├── detection        { status, evidence[], version }
    ├── findings[]       Finding
    ├── diagnostics[]    Diagnostic
    └── summary[]        { key, label, value }
```

- `detection.status`: `notDetected` or `configured` (configuration found; the program itself is not looked for).
- `summary` holds key facts about an agent's configuration, e.g. permission mode or whether a sandbox is active. Each scanner defines its own keys and labels.

## 3. Finding

A finding is a path or rule that grants, restricts or suggests file access for an agent.

| Field | Meaning |
|---|---|
| `id` | Stable identifier: `<agentId>-<16 hex characters>`, computed from agent, subject, source file, JSON pointer and raw value. Unchanged configuration yields the same identifier on every scan. |
| `agentId`, `agentName` | scanner that produced the finding |
| `subject` | who holds the access: `{ "kind": "agent" }` or `{ "kind": "mcpServer", "name": "…" }` |
| `target` | path or rule, see 3.1 |
| `effect` | `allow`, `ask`, `deny` or `notApplicable` (not from a rule, e.g. an observed project path) |
| `access` | `read`, `write`, `execute`, `readWrite`, `unknown` |
| `status` | `configured`, `observed`, `potential`, `blocked`, `unknown` (meaning see roadmap 4.1) |
| `scope` | `managed`, `user`, `project`, `projectLocal`, `session`, `runtime`, `operatingSystem` |
| `source` | origin, see 3.2 |
| `explanation` | human-readable explanation |
| `confidence` | `low`, `medium`, `high` |
| `pathState` | state on disk, see 3.3 |
| `warnings` | security-relevant remarks, see 3.4 |

### 3.1 Target

| Field | Meaning |
|---|---|
| `raw` | value as in the source, e.g. `Read(~/project/**)` |
| `patternKind` | `exactPath`, `directory`, `glob`, `everything` (everything the subject can reach, e.g. a shell without sandbox), `unparsed` |
| `basePath` | path the rule is anchored at; `null` if it cannot be determined |
| `displayPath` | path for display |

### 3.2 Source

| Field | Meaning |
|---|---|
| `file` | file that was read; `null` for derived findings |
| `configKind` | kind of configuration, e.g. `claude-code/settings` |
| `documentation` | `documented` (officially described), `heuristic` (observed, not or barely documented), `derived` (computed by the inspector) |
| `pointer` | JSON pointer to the value in `file`, e.g. `/permissions/deny/0` |
| `rawValue` | value before normalisation |
| `anchor` | base for relative patterns: `filesystemRoot`, `home`, `workingDirectory` or `{ "kind": "directory", "path": "…" }` |
| `parserVersion` | version of the scanner's parsing logic |
| `excerpt` | short evidence without secrets |

### 3.3 PathState

Filled in by the path analysis (phase 3); until then `notChecked`.

| Field | Values |
|---|---|
| `existence` | `exists`, `missing`, `unknown` (check failed), `notChecked` |
| `kind` | `file`, `directory`, `other`, `unknown` |
| `symlinkTarget` | target, if the path is a symlink |

### 3.4 Warning

```json
{ "kind": { "type": "notEffective", "reason": "trustNotAccepted" }, "severity": "info", "message": "…" }
```

| `kind.type` | Meaning |
|---|---|
| `homeDirectory` | the entire home directory is granted |
| `filesystemRoot` | the filesystem root is granted |
| `wholeDrive` | a whole drive is granted |
| `broadPath` | another particularly broad path |
| `missingPath` | the path does not exist |
| `symlinkElsewhere` | a symlink points to another area |
| `conflictingRules` | conflicting allow/deny rules |
| `dangerousMode` | dangerous permission mode |
| `unsandboxedShell` | shell commands without sandbox |
| `likelyMisconfigured` | probable configuration mistake, e.g. a misanchored path |
| `notEffective` | configuration present but not effective; reason in `reason` |

Reasons for `notEffective`: `trustNotAccepted`, `managedRulesOnly`, `scopeNotAllowed`, `ruleNeverMatches`, `overriddenByHigherScope`, `platformUnsupported`.

`severity`: `info`, `low`, `medium`, `high`.

"Not effective" is deliberately a warning, not a status of its own: the finding stays `configured` because that is what the configuration says, and the warning explains why it currently does not apply.

## 4. Diagnostic

Diagnostics describe problems while scanning. They never abort the scan.

| Field | Meaning |
|---|---|
| `level` | `info`, `warning`, `error` |
| `code` | `unreadableFile`, `invalidJson`, `unexpectedType`, `unknownKey`, `unparsableRule`, `unresolvedVariable`, `deprecatedLocation`, `unevaluatedSource`, `internal`, `other` |
| `message` | description |
| `file`, `pointer` | affected file and location, if known |

A scanner that crashes unexpectedly produces a diagnostic with code `internal`; the other scanners keep running.

## 5. Example

Output of `ai-agent-access-inspector scan --agent demo --home /home/alice` (shortened):

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
          "id": "demo-093040fc34e02283",
          "agentId": "demo",
          "agentName": "Demo",
          "subject": { "kind": "agent" },
          "target": {
            "raw": "~/projects",
            "patternKind": "directory",
            "basePath": "/home/alice/projects",
            "displayPath": "/home/alice/projects"
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
            "rawValue": "~/projects",
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
