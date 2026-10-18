# Third-party notices

This application embeds antangelo/xdvdfs 0.8.3, commit
`1cc850bf1b3487fad7ec7c9eed01d83e8fc75ba4`.
Source: https://github.com/antangelo/xdvdfs/tree/v0.8.3

The parser is vendored in `vendor/xdvdfs`. Its manifest has been made
standalone; its Rust implementation is unchanged. Our streaming extraction
orchestration follows the upstream CLI/native unpacker approach, with added
validation, cancellation, verification, and transactional destinations.

MIT License

Copyright 2023 antangelo.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.


Tauri, React, Vite, rfd, Rust crates, and their dependencies are used under
their respective open-source licenses. Exact versions are recorded in
`package-lock.json` and `src-tauri/Cargo.lock`. See `DEPENDENCY_LICENSES.md`
and `licenses/` for the dependency notices shipped with this source release.
System frameworks (AppKit, WebKit) are provided by macOS.
