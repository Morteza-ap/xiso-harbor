# Sequential extraction queue review

## Scope

Multiple ISO/XISO images can be selected in one native picker or dropped together. A single native worker processes them in order, each into a separate folder. Batch folder names derive from the ISO filename; existing folders and matching filenames receive numbered suffixes without replacement. Single-image custom naming remains available.

Cancellation or an extraction error stops the queue. Partial files must be kept or deleted before continuing. Continuing retries unfinished images and skips completed images. Choosing a new selection replaces the queue. The queue is session-only; after relaunch, existing partial-file recovery is available but unfinished images must be selected again.

## Automated verification — 2026-11-11

- Full `scripts/check-quality.sh` passed: frontend format/type/build checks, Rust format, Clippy, native tests, and independent extraction/hash checks.
- 26 native application tests passed (including four queue tests). The example harness also passed its 19 shared engine tests.
- Queue tests cover ordered execution, skipping completed items, stopping on error, cancellation between images, continuation, and real extraction of same-named discs into distinct folders while preserving existing contents.
- Independent synthetic extraction checks passed for raw XISO, XGD1/XGD2/XGD3 offsets, 10,025 files, a file over 2 GiB, unsupported input, destination collision, and cancellation during streaming. Successful outputs matched SHA-256 hashes of independently generated fixture payloads.
- Source/history privacy scan passed before commit. Publication also requires a scan of the staged files and final commit metadata.
- The existing vendored xdvdfs lifetime-syntax warning remains; project Clippy checks passed. No vendored dependency changes were made.

No commercial game images were used. Queue persistence across app restarts is outside this change.

## Packaged app and native interface verification

- Built the self-contained Apple Silicon app using `npm run package`; signature verification and the finished-app privacy scan passed.
- A first package scan found local object/archive paths in Apple linker debug-map records. The wrapper now strips debug data only from the finished executable, restores the existing ad-hoc signature on the executable and bundle, verifies it, and scans the result. Build-time proc-macro dylibs remain untouched.
- Opened the packaged app through macOS and selected three synthetic ISOs together with the native multi-file picker.
- Started extraction, observed the first image complete and the second extracting while the third remained pending, then selected Stop Queue.
- Confirmed on disk: first output committed, second left only a managed partial folder, third had no output.
- Chose Keep Partial Files and Continue Queue. The interface returned to the second image and eventually reported 3 / 3 complete.
- Every one of the 10,035 output files across the three final folders matched the SHA-256 hash of independently generated fixture payloads. The first output's inode and directory modification time were unchanged, and no duplicate first output was created. The explicitly kept partial folder remained intact.
- Created a local preview ZIP using the existing metadata-neutral archive helper; the ZIP privacy scan passed. No GitHub release was created.
- Native multi-file drag-and-drop uses the same selection command, but an actual Finder drag gesture was not separately exercised.
