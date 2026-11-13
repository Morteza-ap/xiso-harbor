# Experimental release verification — 1.1.0

**Date:** 2026-11-13 · **Target:** Apple Silicon macOS · **Distribution:** ZIP containing `Xiso Harbor.app`

This record covers the release build and automated checks for 1.1.0. It is not an independent security audit, a console compatibility guarantee, or Apple notarization. Games on Demand (GOD) output remains experimental and should be tested with one game before adding it to a console drive.

## Results

| Check | Result |
|---|---|
| Prettier, TypeScript and production frontend build | Passed |
| Rust formatting and first-party Clippy with warnings denied | Passed |
| Native regression suite | 33 tests passed; the developer example repeated 19 shared engine tests |
| Synthetic extraction workflow | All 8 checks passed: raw XISO, XGD1/2/3, 10,025 files, unsupported input, output larger than 2 GiB, and streaming cancellation |
| Production package build | Passed for `aarch64-apple-darwin`; the standard app configuration was used, not the preview build |
| Bundle identity and version | `com.local.xbox-iso-extractor`, version 1.1.0 |
| Signature after ZIP extraction | `codesign --verify --deep --strict` passed; ad-hoc signature |
| Archive | 6,648,447 bytes; 827 entries; no AppleDouble files or local timestamps |
| Archive SHA-256 | `6c77de7840254d567bb190384abc346ae78d14f989c3c9f58a928129d57a1162` |

The build emitted the existing lifetime-style warning from the vendored `xdvdfs` dependency. First-party Clippy passed. The package is not notarized by Apple because this project has no Developer ID or notarization credentials; macOS may require a first-launch exception.

## Privacy scope

The packaging step remaps local build paths, strips linker debug paths from the app executable, and restores and verifies the ad-hoc signature. The release hygiene scanner checks source, reachable Git history, and the final app/ZIP for developer-home paths, local identity strings, credential markers, and embedded image metadata. This does not hide the public GitHub repository owner or guarantee anonymity outside the checked source and package.

The app processes images locally and adds no runtime network client. GOD output uses the pinned vendored format code, not an external conversion service. See [Privacy](../PRIVACY.md), [GOD architecture and limits](../GOD-OUTPUT.md), and [Security](../../SECURITY.md).

## Limits

- The package is for Apple Silicon. macOS 12 is the configured minimum; the full supported OS range has not been tested.
- The GOD path has synthetic format and integrity tests, but has not been tested against an Xbox 360. Dashboard visibility and game launch depend on the console, its active patches, and the specific image.
- Multi-disc games and detected installation-content images are not supported by GOD mode. Keep existing extracted folders intact while testing.
- No commercial game images were used. Fixtures contain synthetic data, not executable game content.
- The app is not App Sandbox enabled, and the archive is ad-hoc signed rather than notarized.

Reproduce automated checks with `sh scripts/check-quality.sh`, build with `npm run package`, and create the metadata-neutral ZIP with `python3 scripts/zip-app.py <app> <output.zip>`. The public release includes `SHA256SUMS.txt` for download integrity.
