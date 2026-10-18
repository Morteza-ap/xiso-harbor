#!/bin/sh
# Package the master PNG into macOS icon sizes without retaining intermediates.
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
icon_tmp=$(mktemp -d "${TMPDIR:-/tmp}/xiso-harbor-icons.XXXXXX")
trap 'rm -rf "$icon_tmp"' EXIT HUP INT TERM
iconset="$icon_tmp/AppIcon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  /usr/bin/sips -z "$size" "$size" "$project_dir/src-tauri/icons/icon.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  retina_size=$((size * 2))
  /usr/bin/sips -z "$retina_size" "$retina_size" "$project_dir/src-tauri/icons/icon.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
/usr/bin/iconutil -c icns "$iconset" -o "$project_dir/src-tauri/icons/icon.icns"
python3 "$project_dir/scripts/check-privacy.py" --strip-image-metadata "$project_dir/src-tauri/icons/icon.icns"
