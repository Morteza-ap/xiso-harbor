#!/usr/bin/env python3
"""Exercise the production Rust engine with independent synthetic XISO images."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import time

root = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('fixture', root/'scripts/make-fixture.py')
fixture = importlib.util.module_from_spec(spec); spec.loader.exec_module(fixture)

def digest_file(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024**2), b''): h.update(block)
    return h.hexdigest()

def expected_digest(payload):
    h = hashlib.sha256()
    if isinstance(payload, int):
        block = bytes(1024**2); remaining = payload
        while remaining: size = min(remaining, len(block)); h.update(block[:size]); remaining -= size
    else: h.update(payload)
    return h.hexdigest()

def main():
    parser = argparse.ArgumentParser(); parser.add_argument('--engine', type=Path, default=root/'src-tauri/target/debug/examples/engine-check'); parser.add_argument('--large', action='store_true'); parser.add_argument('--report', type=Path)
    args = parser.parse_args(); engine = args.engine.resolve(); results = []
    def run(*arguments, success=True):
        process = subprocess.run([str(engine), *map(str, arguments)], capture_output=True, text=True)
        assert (process.returncode == 0) == success, (process.stdout, process.stderr)
        return json.loads(process.stdout)
    with tempfile.TemporaryDirectory(prefix='Xbox QA بازی ') as directory:
        base = Path(directory)
        for label, offset in [('raw', 0), ('XGD1',405798912), ('XGD2',265879552), ('XGD3',34078720), ('many-files',0)]:
            started = time.monotonic(); iso = base/f"{label} user's disc.iso"; payloads = fixture.make_fixture(iso, many=label=='many-files', offset=offset)
            result = run('extract', iso, base, label)
            out = Path(result['path']); actual = sorted(p.relative_to(out).as_posix() for p in out.rglob('*') if p.is_file())
            assert actual == sorted(payloads), 'File list mismatch'
            for path, payload in payloads.items(): assert digest_file(out/path) == expected_digest(payload), path
            assert result['hasXex'] and result['files'] == len(payloads)
            collision = run('extract', iso, base, label, success=False); assert collision['category'] == 'collision'
            results.append({'test':label, 'passed':True, 'files':result['files'], 'bytes':result['bytes'], 'seconds':round(time.monotonic()-started,2), 'verification':'SHA-256 of every output file matched independent fixture payload'})
        invalid = base/'invalid.iso'; invalid.write_bytes(b'not an ISO')
        assert run('scan', invalid, success=False)['category'] == 'image'
        results.append({'test':'unsupported-image', 'passed':True})
        if args.large:
            started=time.monotonic(); iso=base/'large.iso'; payloads=fixture.make_fixture(iso, large=True)
            result=run('extract',iso,base,'large')
            for path,payload in payloads.items(): assert digest_file(Path(result['path'])/path)==expected_digest(payload)
            results.append({'test':'multi-gigabyte-streaming', 'passed':True, 'files':result['files'], 'bytes':result['bytes'], 'seconds':round(time.monotonic()-started,2), 'verification':'Every output SHA-256 matched, including the >2 GiB file'})
            cancelled=run('extract',iso,base,'cancelled','cancel',success=False)
            assert cancelled['category']=='cancelled'; assert not (base/'cancelled').exists()
            assert list(base.glob('*.partial')), 'Cancellation should preserve a partial folder'
            results.append({'test':'cancellation-during-streaming','passed':True})
    report={'engine':'xdvdfs 0.8.3 + production Rust streaming backend','fixtures':'Synthetic and redistributable. No commercial game data.', 'results':results}
    text=json.dumps(report,indent=2); print(text)
    if args.report: args.report.write_text(text+'\n')

if __name__=='__main__': main()
