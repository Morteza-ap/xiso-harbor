<div align="center">
  <img src="src-tauri/icons/icon.png" width="88" height="88" alt="Xiso Harbor app icon">
  <h1>Xiso Harbor</h1>
  <p><strong>Turn Xbox disc images into game folders, right on your Mac.</strong></p>
  <p>Private preview &nbsp;·&nbsp; Apple Silicon &nbsp;·&nbsp; Offline &nbsp;·&nbsp; Native</p>
  <p>
    <a href="#get-started">Get started</a>
    &nbsp;·&nbsp; <a href="#how-it-works">How it works</a>
    &nbsp;·&nbsp; <a href="docs/QUALITY-SECURITY-REVIEW.md">Quality and security</a>
  </p>
</div>

---

Xiso Harbor reads compatible Xbox and Xbox 360 ISO/XISO images and extracts their files into a folder you choose. Pick the image, choose where the game should live, and follow the extraction progress in the app.

Everything runs locally on your Mac. Images stay on your computer; the app has no account, telemetry, or online extraction service.

<p align="center">
  <img src="docs/assets/app-window.png" width="780" alt="Xiso Harbor's green console-inspired interface with its wave background, disc selector, and destination controls">
</p>

## Get started

The current source builds a native Apple Silicon macOS app. No downloadable app releases are currently published; build the current source to use the latest interface.

### Requirements

- An Apple Silicon Mac running macOS 12 or later. Testing covers one Apple Silicon environment; the full supported macOS range has not yet been verified.
- Xcode Command Line Tools, Node.js 22.12 or later, and Rust 1.90.0 for building from source.
- Python 3.9 or later for the package privacy check.

### Build the app

From the project folder:

```sh
npm ci
sh scripts/build-icons.sh
cargo fetch --locked --manifest-path src-tauri/Cargo.toml --target aarch64-apple-darwin
npm run package
```

The app is created at `src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Xiso Harbor.app`. Move it to **Applications** to keep it handy, or open it directly. See [DEVELOPMENT.md](DEVELOPMENT.md) for testing, fixture generation, and build details.

> **First launch:** Builds use ad-hoc signing and are not notarized by Apple. macOS may ask you to confirm that you want to open the app. Only continue when you trust the source. Keep Gatekeeper enabled.

## How it works

1. Choose one `.iso` or `.xiso` image with **Browse**, or drop it into the app.
2. Choose a destination folder and review the suggested game-folder name.
3. Select **Extract**. Progress reflects bytes written, and extraction can be cancelled.
4. When verification finishes, use **Show in Finder** to open the output folder.

The app recognizes raw XISO and the XGD1, XGD2, and XGD3 layouts supported by its extraction library. Extract each disc of a multi-disc game into its own folder.

## Built for careful extraction

- Images are read locally; file data is copied with a fixed-size buffer.
- Existing destinations are preserved. Xiso Harbor will not silently replace them.
- Cancelled or interrupted work is kept in a clearly marked partial folder so you can review or remove it.
- Output file types and sizes are checked before the app reports completion.
- Error details omit selected host paths. Review a report before sharing it, since an error from a malformed image can include names stored inside that image.

Xiso Harbor does not repair images, merge discs, set up a console, or execute extracted files. Output checks cannot prove that an image is authentic, complete, or harmless. Use images you trust and are authorized to access.

## Quality and security

The review covered synthetic raw and Xbox 360 image layouts, more than 10,000 files, a file larger than 2 GiB, cancellation, recovery, and malformed metadata. The quality report includes the test results, dependency findings, and remaining limitations. No commercial game images were used.

- [Quality and security review](docs/QUALITY-SECURITY-REVIEW.md)
- [Security model and reporting](SECURITY.md)
- [Extraction test evidence](tests/workflow-test-results.json)
- [Recent wave-motion checks](docs/review/ambient-motion.json)
- [Repository and build privacy](docs/PRIVACY.md)

The project and its private preview are not security-certified, and no claim is made that every possible image can be safely extracted. The app is not enabled for macOS App Sandbox. See [SECURITY.md](SECURITY.md) for the threat model and known limits.

## Project status

This repository is private while the project is reviewed. The code for Xiso Harbor has no project-specific license yet; do not redistribute the app or source. Third-party notices and dependency licenses are included in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and [DEPENDENCY_LICENSES.md](DEPENDENCY_LICENSES.md).

The native extractor uses [antangelo/xdvdfs](https://github.com/antangelo/xdvdfs), pinned to version 0.8.3. The interface is built with React and TypeScript and runs inside Tauri; Rust handles the image and output files.

---

<p align="center"><sub>Made for a focused, local extraction workflow on Mac.</sub></p>
