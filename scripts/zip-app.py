#!/usr/bin/env python3
"""Archive an app without host ownership, extended attributes, or local timestamps."""
import argparse
import os
from pathlib import Path
import stat
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('app', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    app = args.app.resolve(strict=True)
    if app.suffix != '.app' or not (app / 'Contents/Info.plist').is_file():
        parser.error('Expected a built macOS .app bundle')
    output = args.output.resolve()
    if output.is_relative_to(app):
        parser.error('The archive must be outside the app bundle')
    output.parent.mkdir(parents=True, exist_ok=True)
    # Exclusive creation prevents accidentally overwriting a released archive.
    with zipfile.ZipFile(output, 'x', compression=zipfile.ZIP_DEFLATED,
                         compresslevel=9) as archive:
        for path in [app, *sorted(app.rglob('*'))]:
            mode = path.lstat().st_mode
            name = path.relative_to(app.parent).as_posix()
            if stat.S_ISDIR(mode):
                name += '/'
                payload = b''
            elif stat.S_ISLNK(mode):
                target = os.readlink(path)
                if Path(target).is_absolute() or not path.resolve().is_relative_to(app):
                    raise ValueError('App contains a link outside its bundle')
                payload = os.fsencode(target)
            elif stat.S_ISREG(mode):
                payload = path.read_bytes()
            else:
                raise ValueError('App contains a non-regular filesystem object')
            entry = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            entry.create_system = 3
            entry.external_attr = (stat.S_IFMT(mode) | stat.S_IMODE(mode)) << 16
            if stat.S_ISDIR(mode):
                entry.external_attr |= 0x10
            entry.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(entry, payload, compresslevel=9)


if __name__ == '__main__':
    main()
