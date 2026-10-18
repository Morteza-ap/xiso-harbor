# Security and trust

Xiso Harbor is a small, offline extraction utility. It reads ISO/XISO files and writes their contents into a new folder. It does not execute extracted game files, upload images, collect telemetry, or request administrator access.

## Supported build

The latest private review build, **1.0.1**, targets Apple Silicon macOS. Support is best effort. We do not promise a response time or security certification.

## Report a problem

While this repository is private, collaborators can report a suspected security issue in a private repository issue. Include the version, macOS version, what happened, and a minimal synthetic reproduction when possible. Do not post commercial ISOs, credentials, personal files, or unsanitized logs.

Before a public release, a private vulnerability-reporting channel must be configured and this section updated. Ordinary extraction and interface bugs can use GitHub Issues.

## What we protect

- Source images are opened read-only; non-regular inputs are rejected without waiting on named pipes.
- Image filenames, directory records, traversal edges, entry counts, nesting, and region bounds are checked.
- Extraction uses a fixed 1 MiB copy buffer, with cancellation between reads.
- Existing destinations and existing files in a partial extraction are preserved.
- Output-directory symlinks are rejected when encountered. Folder identity is checked during extraction and before cleanup.
- Partial-file deletion requires an absolute app-managed UUID folder name and matching device/inode identity. Recovery writes are atomic and synced, and only one running instance may own recovery state.
- Finder actions reveal a validated directory with `open -R`; they do not launch an extracted bundle.
- Production content is embedded locally. The WebView has a restrictive CSP, event-listen permissions, and no generic shell or filesystem plugin.

## Limits

These safeguards are defense in depth, not a claim that every hostile image is safe. The application is not macOS App Sandbox enabled. A malicious process already running as the same user can race filesystem operations or tamper with that user's data; the current implementation does not claim full resistance to that threat. Do not modify app-managed partial directories during extraction.

An image can be readable and structurally valid while containing damaged or unwanted data. Output size/type verification cannot authenticate a commercial disc or prove that its contents are harmless. Use images you trust and are authorized to access.

The build is ad-hoc signed and not Apple-notarized. Code signing verification checks bundle integrity; it does not establish an Apple-verified publisher identity.

See [the quality and security review](docs/QUALITY-SECURITY-REVIEW.md) for concrete findings, dependency warnings, tests, and remaining gaps. Dependency advisories can change after a scan.
