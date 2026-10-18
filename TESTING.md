# Build and test record

Verification dated 2026-10-16 covers an Apple Silicon macOS environment. The release target is `aarch64-apple-darwin`; the configured minimum OS is macOS 12.0. The full supported OS range has not been tested. Rust 1.90.0 and Apple Command Line Tools were used.

## Completed checks

- TypeScript strict checking and production frontend build passed.
- Eleven native Rust tests passed. They cover a native roundtrip, case-insensitive `default.xex` detection, unsafe filenames/path traversal, malformed/truncated images, bounded directory-cycle handling, insufficient-space preflight, destination collisions including a late commit collision, cancellation, preservation of unrelated files, refusal to delete a replacement symlink, disc-name/suffix suggestions, case-colliding image paths, and source changes before commit. Some tests cover several related conditions.
- Independent synthetic XISO fixtures were extracted by the **same engine module used by the app**, using a developer-only harness. SHA-256 hashes for every output matched independently generated fixture payloads.

| Extraction fixture | Result |
|---|---|
| Raw XISO | 5 files / 4,492 bytes; all hashes matched |
| XGD1 partition at 405,798,912 bytes | 5 files; all hashes matched |
| XGD2 partition at 265,879,552 bytes | 5 files; all hashes matched |
| XGD3 partition at 34,078,720 bytes | 5 files; all hashes matched |
| Many files | 10,025 files / 133,662 bytes; all hashes matched |
| Multi-gigabyte | 6 files / 2,151,682,461 bytes, including a single file over 2 GiB; all hashes matched |
| Cancellation while streaming | Worker stopped; no final directory; partial output retained |
| Existing destination | Refused for each completed fixture; existing contents preserved |
| Unsupported source | Rejected without extracting |

Host paths in these tests included non-ASCII text, spaces, and an apostrophe. Images included nested directories, a Windows-1252 accented filename, mixed filename case, and an empty file. The large source is sparse; its extracted output was actually streamed to disk and then fully hashed. Peak resident memory was not separately measured. Timing results are machine-specific and recorded in `tests/workflow-test-results.json`; they are not performance promises.

## Desktop GUI checks

The genuine production `.app` was launched through macOS Launch Services, loading embedded assets from `tauri://localhost`.

- Native ISO picker selected `Sample Disc.iso`; preflight detected five files.
- Native folder picker selected a dedicated test output directory.
- **Extract Game** completed successfully; **default.xex found** appeared.
- **Open Extracted Folder** opened the expected folder in Finder; all five GUI-extracted files then matched their expected SHA-256 hashes.
- A second session of the final release loaded the 10,025-file fixture after the development server had been stopped. Real progress, current-file names, counts, and elapsed time updated while the app remained responsive.
- Quitting was attempted while extraction was visibly active; the app remained running.
- **Cancel Extraction** stopped the worker and displayed its unique `.partial` folder with Keep/Delete choices.
- The app was quit with the cancelled partial unresolved. Relaunch recovered that pending folder and required a choice before another image could be selected.
- **Copy Error Details** used the native macOS clipboard and confirmed success.
- **Keep Partial Files** preserved the partial folder and restored the normal controls. No user-owned game files were used or deleted.

The cancellation/recovery scenario tests restart with an unresolved cancelled job. A forced process crash or sudden power loss was not simulated. Actual deletion is verified by native tests against disposable temporary fixtures; the GUI deletion button was not clicked.

## Package verification

- A real `.app` was built and ad-hoc signed. `file` identifies its executable as Mach-O **arm64**.
- `codesign --verify --deep --strict` passed.
- `otool -L` shows only macOS system libraries/frameworks; there is no external Rust, Node, Cargo, Homebrew, or xdvdfs runtime dependency.
- The final `.app` contains one application executable, embedded frontend assets, an icon, and third-party notices/licenses. The developer harness is an example target and is not bundled.
- No Developer ID credentials were available, so Apple notarization and a clean-download Gatekeeper installation on a second Mac were not tested.

## Unverified or limited

- Commercial game images were not supplied or tested. Synthetic layout compatibility does not prove compatibility with any particular commercial dump.
- Full accessibility testing with VoiceOver, an actual Finder-to-window drag gesture, and native dark-mode visual inspection remain unverified. Native drop handling and semantic accessible controls are implemented; the accessibility tree was inspected during GUI checks.
- Physical drive removal, real disk exhaustion during a copy, macOS privacy denial, hardware read failures, Intel Macs, and older macOS versions were not exercised. Free-space rejection and malformed/truncated-source cases were tested. OS I/O errors have actionable mappings, but hardware faults cannot be repaired.
- No claim is made that every Xbox image format or damaged image is recoverable. This release accepts raw ISO/XISO with the partition layouts supported by xdvdfs 0.8.3; compressed/split images are outside this MVP.
- Output validation checks successful native completion, file presence/type/size, and source stability. It cannot authenticate a commercial game or detect all pre-existing source corruption without known reference checksums.

## Test your own image

Keep your original ISO unchanged. Extract one disc to a new folder on a reliable local drive with ample free space. On completion, inspect the file tree and `default.xex` status. If another trusted tool successfully extracts the same image, compare output hashes if practical. For a failure, use **Copy Error Details**, review the report, and share it together with the app version and drive/filesystem type. No ISO upload is needed.
