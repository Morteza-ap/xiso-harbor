# Xiso Harbor — quality and security review

**Review date:** 2026-10-17 · **Build:** 1.0.1 · **Target:** aarch64-apple-darwin

This is an in-repository engineering review, automated dependency scan, and regression test pass. It is not an independent penetration test, security certification, or guarantee against every hostile image. The repository and release remain private for the owner's review.

## Findings and changes

| Finding | Evidence / risk | Resolution |
|---|---|---|
| Named-pipe input could block validation | The old harness timed out while opening a FIFO named `pipe.iso`, before the regular-file check ran. | Read-only source opening now uses `O_NONBLOCK`, `O_NOFOLLOW`, and `O_CLOEXEC`, then rejects non-regular files. A regression test exercises a real FIFO. |
| Upstream tree traversal did not reject repeated child pointers immediately | A malformed directory could consume the metadata-read budget and allocate a large intermediate list before global limits were checked. Full record bounds also needed checks against the containing table. | Local guarded traversal still uses upstream record decoding, but validates headers, names, overlap, repeated pointers, and remaining entry budget before accepting records. |
| Output ancestors could be followed if replaced by symlinks | External modification of an app-managed partial folder could redirect a later write. | Encountered ancestor symlinks are refused; root identity is rechecked per entry. A planted-link regression test confirms no write reaches its target. This does not claim full resistance to a concurrent malicious same-user process. |
| Cleanup trusted a generic `.partial` suffix | A forged recovery record with a matching directory identity could refer to an unrelated partial folder. | Cleanup now also requires an absolute app-managed UUID directory name. Both new and original app prefixes remain supported. |
| Recovery ownership and persistence needed hardening | Two app instances could contend for the same record. Reads were unbounded; writes did not sync the record before commit. | An OS file lock allows one owning instance. Record reads are capped at 16 KiB and reject non-regular/symlink files. Private temporary records are written, synced, and atomically renamed. |
| Finder opening used default file dispatch | A bundle-shaped output directory should never be dispatched for execution. | The native command validates a directory and reveals it using `/usr/bin/open -R`. |
| Long Unicode suggestions could exceed the byte limit | Character count and filesystem byte count differ. | Suggestions now fit a 200-byte budget, leaving room for unique suffixes. Unsafe suggestions fall back to a simple name. |
| Event permissions were broader than needed | The frontend only needs to listen to native state and drop events. | Capabilities now allow listen/unlisten, without frontend event emission. |
| Vite 7.1.7 matched high-severity development-server advisories | The development dependency was affected; the packaged app embeds static assets and does not ship a Vite server. | Updated and pinned to 7.3.7. The repeat npm audit reports zero vulnerabilities. |

## Verification

| Check | Result |
|---|---|
| TypeScript strict checking and production frontend build | Passed |
| Prettier and Rust formatting checks | Passed |
| Clippy on first-party targets, `--no-deps -- -D warnings` | Passed |
| Native main test target | **22 passed**: 19 engine tests and 3 storage/locking tests |
| Developer example target | Its shared engine tests also passed; these are repeated executions, not additional distinct tests |
| Deterministic malformed-metadata smoke corpus | 256 mutated fixture variants, no parser panic |
| Raw XISO / XGD1 / XGD2 / XGD3 | Extraction passed; every file matched an independent SHA-256 hash |
| Many-file fixture | 10,025 files, all hashes matched |
| Multi-gigabyte fixture | 2,151,682,461 output bytes, including a file over 2 GiB; all hashes matched |
| Cancellation during native streaming | Cancelled without a final destination; partial output remained |
| Collision, source-change, symlink and unrelated-folder regressions | Passed |

The fixed-size file copy buffer is 1 MiB; metadata memory scales with the capped entry manifest. No peak-memory benchmark or continuous fuzzing campaign was performed. The smoke corpus is a repeatable regression test, not proof that arbitrary input cannot fail.

Clippy treats first-party warnings as errors. The unchanged vendored xdvdfs code still produces one compiler lifetime-syntax warning; it is not a reported memory-safety failure and is recorded in the logs.

## Dependency scan

Both scans were refreshed during this review. They reflect their advisory databases at that time and must be repeated as dependencies or advisories change.

- **npm:** zero reported vulnerabilities after the Vite update.
- **RustSec / cargo-audit 0.22.2:** zero vulnerability entries, with three informational warnings in the cross-platform lockfile. Those warnings are retained below and in the raw report; none was hidden through an ignore list.

| Advisory | Dependency | Applicability / decision |
|---|---|---|
| [RUSTSEC-2025-0141](https://rustsec.org/advisories/RUSTSEC-2025-0141.html) | bincode 1.3.3, unmaintained | Used by the pinned native xdvdfs library. No patched version is listed. Retained as a disclosed maintenance risk; replacing upstream serialization requires its own compatibility review. |
| [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html) | proc-macro-error 1.0.4, unmaintained | Present in the cross-platform lockfile, absent from the Apple Silicon normal/build dependency graph. |
| [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) | glib 0.18.5, unsound iterator implementation | Present in the Linux-side lockfile graph, absent from the Apple Silicon normal/build dependency graph. This review does not approve a Linux release. |

`cargo tree --target aarch64-apple-darwin --edges normal,build -i glib` and the equivalent command for `proc-macro-error` report no reverse dependency in this target. These platform findings do not erase the advisories from the lockfile scan.

## Native app and distribution

The renamed app is built as an actual Apple Silicon `.app`, with the native engine and production frontend bundled. The original bundle identifier is preserved for recovery compatibility. Release artifacts and their hashes are checked before the private upload.

The app remains ad-hoc signed, without Developer ID or Apple notarization. It is not macOS App Sandbox enabled. Historical 1.0.0 GUI extraction/cancellation/recovery evidence is in [TESTING.md](../TESTING.md); current 1.0.1 checks confirmed native file/folder selection, a five-file extraction with matching SHA-256 hashes, default.xex detection, and Finder revealing the selected output directory. The README screenshot was captured from the actual renamed app.

## Remaining limits

- No commercial game ISOs were supplied or tested.
- Older macOS releases, full VoiceOver use, physical drive removal, real disk exhaustion, and sudden power loss remain unverified.
- A malicious process already running as the same user is outside the defended threat model. The code rejects observed symlinks but does not use a fully directory-handle-relative filesystem implementation for every operation.
- Source size/modification-time and output size/type checks cannot authenticate an image or detect all pre-existing source corruption.
- An independent audit, continuous fuzzing, and a clean second-Mac Gatekeeper installation were not performed.
- A project license and a private security-reporting channel need decisions before any public release. Shared logs omit machine paths. The [repository privacy review](PRIVACY.md) records sanitized branch/tag history and the remaining archived-download and retained-reference limits.

## Reproduce and inspect

From a prepared developer environment, run `sh scripts/check-quality.sh`. Run `npm audit --package-lock-only` and `cargo audit --file src-tauri/Cargo.lock` separately for current advisory data. Setup is described in [DEVELOPMENT.md](../DEVELOPMENT.md).

Evidence: [native test log](review/native-tests.txt), [Clippy log](review/clippy.txt), [extraction/hash results](review/workflow.json), [npm audit JSON](review/npm-audit.json), [RustSec audit JSON](review/cargo-audit.json).
