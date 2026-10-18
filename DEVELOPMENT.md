# Development and reproducible builds

## Architecture

Tauri 2 hosts a small React/TypeScript interface. Native Rust commands own dialogs, parsing, streaming disk I/O, output verification, cancellation, and Finder opening. No browser file API, WebAssembly extraction, network service, shell plugin, or arbitrary command runner is used. File paths are never interpolated into a shell command. Finder is invoked with `open -R` to reveal a directory rather than dispatch its contents for execution.

The upstream repository was inspected at tag `v0.8.3`, commit `1cc850bf1b3487fad7ec7c9eed01d83e8fc75ba4`. It includes a Tauri 1 desktop app with broad permissions and a shared Yew browser frontend. Its native unpacker and the CLI were reviewed. Their parsing/streaming approach is retained through the vendored core library; the shell and orchestration are implemented for Tauri 2 to provide the required safety, state model, and UX. No macOS binary is provided by that upstream release. The library is linked into the executable, so a separate sidecar is unnecessary.

The library's raw-image offset detector supports raw XISO and the upstream XGD1/XGD2/XGD3 offsets. We pre-scan directory metadata for a real file/byte total. The worker copies 1 MiB at a time, checks cancellation between chunks, syncs each file, checks every output type and size, and confirms that the open source file's size/modification time did not change. Successful extraction commits with macOS `renamex_np(RENAME_EXCL)`, so even a late destination collision cannot replace a folder. There are no engine-provided source checksums; output size/type validation is not proof that a source image is an authentic or uncorrupted commercial disc.

Safety bounds: 200,000 directory entries, 64 nested directory levels, 1,000,000 metadata reads, 900-byte relative output paths, and 240-byte individual names. Directory cycles, path traversal, control characters, separators/colons, and case/Unicode-normalization collisions are rejected. Metadata memory scales with entry count under these bounds; file contents use the fixed copy buffer. Host paths support Unicode; filenames stored in XDVDFS follow upstream Windows-1252 decoding.

The custom command interface only exposes this utility's actions. Tauri's capability grants cover local events; there is no general filesystem or shell plugin. Native pickers choose paths; dropped ISO paths are validated. The app is not App-Sandbox-enabled and uses normal user permissions. A strict production CSP prevents remote scripts, frames, and network connections. No telemetry, updater, or online assets are configured. The only spawned runtime executable is the system `/usr/bin/open`, with a structured absolute path argument, to open Finder.

A single worker owns the session, and a process lock prevents a second app instance from overwriting recovery state. A recovery record in the app's local Application Support directory stores the temporary folder's path, device, and inode. Cleanup checks that identity and never follows a replacement root symlink. An external process modifying app-owned temporary files during extraction is outside the normal trust model; do not edit them until work stops.

## Prerequisites for developers only

- An Apple Silicon Mac with Xcode Command Line Tools (`xcode-select --install`).
- Rust **1.90.0**, target `aarch64-apple-darwin`.
- Node **22.12+** and npm.
- Python 3.9+ for fixture/verification helpers and package privacy checks.

The end user needs none of these tools. The packaged Mac app is self-contained.

## Build

From this source folder:

```sh
npm ci
sh scripts/build-icons.sh
cargo fetch --locked --manifest-path src-tauri/Cargo.toml --target aarch64-apple-darwin
python3 scripts/collect-licenses.py
npm run package
```

The `.app` is written to `src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Xiso Harbor.app`. Both dependency lockfiles are included. The vendored xdvdfs manifest is made standalone with explicit upstream metadata; its Rust implementation is unchanged. Local guarded directory traversal uses upstream record decoding and adds complete record bounds, cycle checks, and early entry limits.

`npm run package` resolves local directories at build time and remaps Rust source paths to neutral `/build/` prefixes. It checks the finished bundle for common developer paths, local identity strings, credential markers, and image metadata. Use this command for distributable builds; a direct `tauri build` bypasses that wrapper. See [repository and build privacy](docs/PRIVACY.md) for scope and limitations.

`npm run desktop` starts a development window. `npm run dev` alone is only a browser view; native extraction controls are deliberately disabled there. Production bundles load embedded frontend assets and require no development server.

## Tests

```sh
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --bin xbox-iso-extractor
cargo build --locked --manifest-path src-tauri/Cargo.toml --example engine-check
python3 scripts/test-workflow.py --large --report workflow-test-results.json
```

The developer-only `engine-check` executable imports the exact production engine module and is not bundled in the `.app`. Synthetic fixtures are generated independently of xdvdfs. The large test writes and hashes an actual >2 GiB extracted file, plus many-file and partition-offset fixtures, and needs several GiB of free space. Temporary test data is removed automatically. To create a fixture for GUI testing:

```sh
python3 scripts/make-fixture.py 'Sample Disc.iso'
```

No commercial game content is included in fixtures. They may be redistributed freely.

## Signing and distribution

The bundle uses ad-hoc signing (`-`). It is not Developer-ID-signed or notarized. For public distribution, use your own Apple Developer credentials and the official Tauri/macOS signing and notarization flow. No credentials or signing accounts are included. Do not weaken system-wide Gatekeeper settings.

## Quality and security review

Run `sh scripts/check-quality.sh` for format, TypeScript/build, Clippy, native tests, and full synthetic extraction/hash checks. It needs several GiB of temporary disk space. For current advisory checks, run `npm audit --package-lock-only` and `cargo audit --file src-tauri/Cargo.lock` (cargo-audit 0.22.2 was used for this review). See `docs/QUALITY-SECURITY-REVIEW.md` for results and warnings.

The app keeps the original `com.local.xbox-iso-extractor` bundle identifier and `xbox-iso-extractor` binary name for recovery compatibility. Its displayed name and packaged `.app` are Xiso Harbor.

## App icon

The master is `src-tauri/icons/icon.png` (1024 × 1024, RGBA). `sh scripts/build-icons.sh` creates the bundled `icon.icns` using standard macOS tools and removes the temporary iconset automatically. The README uses the same master, so no duplicate documentation icon is stored.

The green master was edited with the built-in image tool, preserving the disc design. Final edit prompt:

> Use case: precise-object-edit. Edit target: the supplied Xiso Harbor macOS app icon. Recolor it to match a minimal green-to-dark console UI. Preserve the same centered optical-disc symbol, its exact proportions, circular layers and highlight arc, and the rounded-square silhouette. Change only the palette: use a rich forest-green-to-near-black green gradient for the square (roughly #356b22 at upper left to #0b1b10 at lower right); use pale lime green / off-white green for the disc layers and subtle green-tinted detail, with a dark green center hole. Keep the icon crisp, flat and legible at small sizes. No words, no letters, no Xbox trademark, no added objects, no extra decoration, no outer shadow. Center the existing rounded square in a square 1024x1024 canvas, preserve its current small transparent outer margin and genuinely transparent corners. Output the icon asset alone.
