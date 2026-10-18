# Experimental release verification — 1.0.2

**Date:** 2026-10-18 · **Target:** Apple Silicon macOS · **Distribution:** ZIP containing `Xiso Harbor.app`

This is an engineering check of the packaged experiment, not an independent security audit or Apple notarization. The executable was rebuilt from the public source with the packaging wrapper; it embeds the production frontend and native extractor and needs no development server or separately installed extraction tool.

## Results

| Check | Result |
|---|---|
| Prettier, TypeScript and production frontend build | Passed |
| Rust formatting and first-party Clippy with warnings denied | Passed |
| Native regression suite | 22 tests passed; the developer example also repeated 19 shared engine tests |
| Independent extraction/hash workflow | All 8 checks passed: raw XISO, XGD1/2/3, 10,025 files, invalid input, >2 GiB output, streaming cancellation |
| Final ZIP app launch and native pickers | Passed after unpacking the ZIP with macOS `ditto` |
| GUI extraction | 5 files, 4,492 bytes; every output SHA-256 matched independent synthetic payloads |
| Completion and Finder action | Completion and `default.xex` detection displayed; Finder revealed the output directory |
| App signature after ZIP extraction | `codesign --verify --deep --strict` passed; ad-hoc signature |
| Archive round trip | Every app file and permission matched the signed build; executable is arm64 |
| Self-contained runtime | Linked libraries are macOS system libraries/frameworks; engine and frontend are bundled |
| License distribution | MIT project license, third-party notices and dependency license texts present in app resources |
| Targeted source/history and artifact privacy scan | Passed; checked developer paths, local identity strings, credentials and image metadata |
| ZIP metadata | Only app bundle entries; no AppleDouble files, ownership/extended-attribute extra fields, or local timestamps |
| Private vulnerability reporting | Enabled on GitHub |

The initial GUI check exposed a destination sheet that did not appear after a disc sheet closed. Both native pickers now explicitly use the main application window as their parent. The rebuilt ZIP passed the same disc → destination → extraction sequence. The streaming extraction engine was unchanged.

## Dependency findings

A refreshed npm audit reported **zero vulnerabilities**. cargo-audit 0.22.2 reported **zero vulnerability entries and six informational warnings**, with none ignored:

- **bincode 1.3.3:** unmaintained, RUSTSEC-2025-0141. Used by the pinned extractor; retained as a disclosed maintenance risk.
- **proc-macro-error, glib, zbus_names, zvariant and zvariant_derive:** maintenance/unsoundness warnings in the cross-platform lockfile. `cargo tree --target aarch64-apple-darwin --edges normal,build` found no reverse dependencies for these packages in this target. This review does not approve Linux or other platform builds.

The vendored xdvdfs library still emits its previously documented lifetime-syntax compiler warning. First-party Clippy passed. Advisory results are a snapshot and may change.

## Privacy scope

Production source/configuration was inspected for telemetry, remote assets, updater and network-client behavior. No such runtime service is configured; the app processes images locally. Recovery paths and filesystem identity are stored locally, and diagnostic copying is user initiated. See [Privacy](../PRIVACY.md). These checks are not a packet-capture study or a guarantee of anonymity; GitHub still displays repository ownership.

## Limits of this release check

- Tested in one Apple Silicon environment. macOS 12 is the minimum configured target; the entire OS range and Intel Macs were not tested. Intel is not a supported download target.
- No commercial game images were used. Fixtures contain no executable game code.
- No clean second-Mac/browser-quarantine Gatekeeper installation was performed. The app is ad-hoc signed, not Developer-ID-signed or Apple-notarized; a first-launch exception may be required. Managed Macs can restrict exceptions.
- Full VoiceOver operation, physical drive removal, disk exhaustion, sudden power loss, continuous fuzzing and an independent audit were not performed.
- The app is not App Sandbox enabled. Same-user filesystem races and the limits of image authenticity/output verification remain as described in [Security](../../SECURITY.md).

## Evidence

- [Extraction workflow](release-1.0.2-workflow.json)
- [Dependency audit summary](release-1.0.2-audits.json)
- [GUI/package verification and hashes](release-1.0.2-package.json)
- [Historical security review](../QUALITY-SECURITY-REVIEW.md)

Reproduce the automated suite with `sh scripts/check-quality.sh`, build with `npm run package`, and archive with `python3 scripts/zip-app.py <app> <output.zip>`. Detailed prerequisites are in [Development](../../DEVELOPMENT.md). Public release assets include a SHA-256 checksum for the ZIP.
