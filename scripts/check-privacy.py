#!/usr/bin/env python3
"""Check tracked source, optional Git history, and distribution artifacts.

Reports locations and categories only, never matched private values. This is a
release hygiene check, not a guarantee of anonymity or a complete secret audit.
"""
import argparse
import json
import re
import socket
import subprocess
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
METADATA = {b'eXIf', b'iTXt', b'tEXt', b'zTXt', b'tIME'}


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)


def image_metadata(data):
    if data.startswith(b'\x89PNG\r\n\x1a\n'):
        offset = 8
        while offset + 12 <= len(data):
            size = int.from_bytes(data[offset:offset + 4], 'big')
            if data[offset + 4:offset + 8] in METADATA:
                return True
            offset += size + 12
    elif data.startswith(b'icns'):
        offset = 8
        while offset + 8 <= len(data):
            size = int.from_bytes(data[offset + 4:offset + 8], 'big')
            if size < 8:
                return True
            if image_metadata(data[offset + 8:offset + size]):
                return True
            offset += size
    return False


def strip_image_metadata(data):
    if data.startswith(b'\x89PNG\r\n\x1a\n'):
        result = bytearray(data[:8])
        offset = 8
        while offset + 12 <= len(data):
            size = int.from_bytes(data[offset:offset + 4], 'big') + 12
            if data[offset + 4:offset + 8] not in METADATA:
                result.extend(data[offset:offset + size])
            offset += size
        return bytes(result)
    if data.startswith(b'icns'):
        result = bytearray()
        offset = 8
        while offset + 8 <= len(data):
            size = int.from_bytes(data[offset + 4:offset + 8], 'big')
            if size < 8:
                raise ValueError('Invalid icon chunk')
            payload = strip_image_metadata(data[offset + 8:offset + size])
            result.extend(data[offset:offset + 4] + (len(payload) + 8).to_bytes(4, 'big') + payload)
            offset += size
        return b'icns' + (len(result) + 8).to_bytes(4, 'big') + result
    # ICNS also contains non-PNG records; preserve those records byte for byte.
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--history', action='store_true', help='Also inspect every reachable commit/blob')
    parser.add_argument('--artifact', type=Path, action='append', default=[], help='Also inspect an app, file, or ZIP')
    parser.add_argument('--deny-literal', action='append', default=[], help='Additional personal value to flag (never echoed)')
    parser.add_argument('--strip-image-metadata', type=Path, action='append', default=[], help='Remove only nonessential PNG/ICNS metadata, then exit')
    args = parser.parse_args()
    if args.strip_image_metadata:
        for path in args.strip_image_metadata:
            if path.suffix.lower() not in ('.png', '.icns'):
                raise ValueError('Only PNG and ICNS files are supported')
            path.write_bytes(strip_image_metadata(path.read_bytes()))
        print(json.dumps({'metadataStripped': len(args.strip_image_metadata)}))
        return False
    account = Path.home().name
    generic_accounts = {'root', 'runner', 'user', 'builder', 'build', 'admin', 'developer', 'home'}
    literals = [socket.gethostname(), *args.deny_literal]
    if account.lower() not in generic_accounts:
        literals.append(account)
    rules = {
        # /build/home is the package wrapper's neutral prefix, not a host path.
        'developer-home-path': re.compile(b'(?<!/build)/' + b'(?:Users|home)/[^/\s\x00]+'),
        'host-temporary-path': re.compile(b'/(?:private/)?var/' + b'folders/'),
        'local-host-email': re.compile(rb'[A-Za-z0-9_.+-]+@[A-Za-z0-9_.-]+\.local\b'),
        'commit-local-timezone': re.compile(rb'^(?:author|committer) .* \d+ (?!\+0000)[+-]\d{4}$', re.MULTILINE),
        'local-workspace-layout': re.compile(b'Documents/' + b'Codex/|outputs/' + b'Xbox-ISO-Extractor'),
        'recorded-device-preference': re.compile(b'"system' + b'(?:ReduceMotion|LowPowerMode)"\s*:'),
        'credential-marker': re.compile(rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,}|AKIA[A-Z0-9]{16}|sk-proj-[A-Za-z0-9_-]{40,}'),
    }
    findings = []
    inspected = 0

    def inspect(location, data):
        nonlocal inspected
        inspected += 1
        categories = [name for name, pattern in rules.items() if pattern.search(data)]
        if any(value.encode().lower() in data.lower() for value in literals if len(value) >= 4):
            categories.append('local-or-denied-identity')
        if image_metadata(data):
            categories.append('embedded-image-metadata')
        if categories:
            findings.append({'location': location, 'categories': categories})

    # Untracked local files and .git/config are deliberately not published source.
    for name in git('ls-files', '-z').decode().split('\0')[:-1]:
        path = ROOT / name
        if path.is_symlink():
            inspect(name, str(path.readlink()).encode())
        elif path.is_file():
            inspect(name, path.read_bytes())

    if args.history:
        process = subprocess.Popen(['git', 'cat-file', '--batch'], cwd=ROOT,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        try:
            for entry in git('rev-list', '--objects', '--all').decode().splitlines():
                oid, _, name = entry.partition(' ')
                process.stdin.write((oid + '\n').encode())
                process.stdin.flush()
                header = process.stdout.readline().decode().split()
                data = process.stdout.read(int(header[2]))
                process.stdout.read(1)
                if header[1] in ('blob', 'commit', 'tag'):
                    inspect('history:' + oid[:12] + ':' + name, data)
        finally:
            process.stdin.close()
            process.stdout.close()
            process.wait()

    for artifact in args.artifact:
        if artifact.is_dir():
            for path in sorted(artifact.rglob('*')):
                if path.is_symlink():
                    inspect('artifact:' + path.relative_to(artifact).as_posix(), str(path.readlink()).encode())
                elif path.is_file():
                    inspect('artifact:' + path.relative_to(artifact).as_posix(), path.read_bytes())
        elif zipfile.is_zipfile(artifact):
            with zipfile.ZipFile(artifact) as archive:
                for entry in archive.infolist():
                    if entry.is_dir():
                        continue
                    location = 'archive:' + artifact.name + ':' + entry.filename
                    inspect(location, archive.read(entry))
                    if entry.filename.startswith('__MACOSX/') or '/._' in entry.filename:
                        findings.append({'location': location, 'categories': ['appledouble-metadata']})
        else:
            inspect('artifact:' + artifact.name, artifact.read_bytes())

    print(json.dumps({'passed': not findings, 'objectsInspected': inspected, 'findings': findings}, indent=2))
    return bool(findings)


if __name__ == '__main__':
    raise SystemExit(main())
