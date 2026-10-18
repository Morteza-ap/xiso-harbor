# Changelog

## 1.0.2 — Experimental public download

- Published a self-contained Apple Silicon `.app` in a ZIP; no developer tools are needed to run it.
- Added the MIT project license and bundled it alongside third-party notices.
- Documented installation, macOS first-launch approval, offline behavior, local recovery data, and experiment limitations.
- Enabled private vulnerability reporting for the public repository.
- Bound native disc and destination dialogs to the main window explicitly, avoiding reliance on macOS choosing a parent after an earlier sheet closes.

- Simplified the UI to a compact green-to-dark console style.
- Added an 840-point window width limit and a consistent dark appearance.
- Recolored the app icon green and consolidated icon assets around a single PNG master.
- Added a slow isometric wave mesh with green contour lines and reflections, with reduced-motion and hidden-window pauses.
- Made the wave motion clearly visible with roughly 15- and 23-second cycles, and preserved the wave phase during window resizing.
- Kept extraction logic and permissions unchanged.

## 1.0.1 — Xiso Harbor private review build

- Adopted the Xiso Harbor name and refreshed the README for first-time users.
- Updated Vite to 7.3.7 to address development-server advisories.
- Fixed validation hanging on a named pipe with an ISO extension.
- Added complete directory-record bounds, repeated-child/cycle checks, and early metadata entry limits around upstream decoding.
- Rejected symlink ancestors encountered in the partial output tree and refused deletion of unrelated `.partial` directories.
- Added a process lock and bounded, atomic recovery records.
- Changed Finder opening to directory reveal, so bundle-shaped output cannot be launched by the app.
- Limited WebView event permissions to listening/unlistening.
- Fixed long Unicode destination suggestions to respect filesystem byte limits.
- Added regression tests and a deterministic malformed-metadata smoke corpus.

The bundle identifier and native binary name remain stable so the renamed app can find recovery state from the original build.

## 1.0.0 — Initial private test build

Native Xbox ISO/XISO extraction, file/folder pickers, drag-and-drop handling, truthful progress, cancellation, output verification, and partial-file recovery. Released privately under the original name, Xbox ISO Extractor.
