# Privacy and release checks

Xiso Harbor processes selected images locally. The app does not upload images, use an account, or collect telemetry. Review error reports before sharing them because malformed images can contain names that appear in diagnostics.

Before sharing source or a macOS build, run `npm run check:privacy`. For a build, use `npm run package`; the packaging script remaps local build paths and scans the resulting app. These targeted checks help catch common personal-data leaks, but they do not prove that a project or account is anonymous.

Screenshots and icons should be checked for metadata. Test fixtures should be synthetic. Keep dependency and copyright notices with the source.

GitHub displays the owner of a public repository and may show activity associated with that account. Cleaning files and commit history does not anonymize the account that publishes them.

For vulnerability reports and the limits of the app's protections, see [SECURITY.md](../SECURITY.md).
