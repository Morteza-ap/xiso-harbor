# Privacy and release checks

Xiso Harbor processes selected images locally. The app does not collect or send images, extracted files, filenames, analytics, or telemetry to the maintainer. It has no account, cloud extraction service, advertising, remote assets, or automatic update client. Production frontend assets and the native extractor are bundled in the app; no OpenAI account, API key, or runtime AI service is used.

## Local storage and reports

- Your chosen destination receives the extracted files. The source image is opened read-only.
- Interrupted extraction leaves a marked `.partial` folder. A small recovery record in the app's local Application Support directory contains its absolute path, device ID, and inode so the app can offer recovery and check cleanup ownership. A process lock also lives there. These records are local and are not uploaded.
- Session state and diagnostic logs are held in memory. The app only copies diagnostics to the clipboard when you ask it to. Selected image/destination paths are redacted from copied reports, but names inside an image or unusual operating-system errors may still reveal information. Review a report before sharing it.
- GitHub handles downloads and anything you voluntarily submit through issues or vulnerability reports under its own policies. macOS may perform its own security checks. These services are separate from the app's offline extraction workflow.

## Release hygiene

Before sharing source or a macOS build, run `npm run check:privacy`. For a build, use `npm run package`; the packaging script remaps local build paths and scans the resulting app. These targeted checks help catch common personal-data leaks, but they do not prove that a project or account is anonymous.

Screenshots and icons should be checked for metadata. Test fixtures should be synthetic. Keep dependency and copyright notices with the source.

GitHub displays the owner of a public repository and may show activity associated with that account. Cleaning files and commit history does not anonymize the account that publishes them.

For vulnerability reports and the limits of the app's protections, see [SECURITY.md](../SECURITY.md).
