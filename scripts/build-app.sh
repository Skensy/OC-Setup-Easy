#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=11.0
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
repo="$PWD"
app="$repo/dist/OC Setup Easy.app"
cache="$repo/target/packaging"
mkdir -p "$cache" "$app/Contents/MacOS" "$app/Contents/Resources"
archive="$cache/OpenCore-1.0.8-RELEASE.zip"
if [[ ! -f "$archive" ]]; then
  curl -fL --retry 3 https://github.com/acidanthera/OpenCorePkg/releases/download/1.0.8/OpenCore-1.0.8-RELEASE.zip -o "$archive.part"
  mv "$archive.part" "$archive"
fi
echo "2011e8b7216ecb2645d97ea710df965947edd406846e92050b9bc4190be6d27b  $archive" | shasum -a 256 -c -
cargo build --locked --release
cp target/release/oc-setup-easy "$app/Contents/MacOS/"
cp native/Info.plist "$app/Contents/"
ditto ui "$app/Contents/Resources/ui"
cp licenses/OpenCorePkg.txt "$app/Contents/Resources/"
unzip -p "$archive" Utilities/ocvalidate/ocvalidate > "$app/Contents/Resources/ocvalidate"
chmod 755 "$app/Contents/Resources/ocvalidate"
icons="$cache/AppIcon.iconset"
mkdir -p "$icons"
sips -s format png assets/icon.svg --out "$cache/icon.png" >/dev/null
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$cache/icon.png" --out "$icons/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$cache/icon.png" --out "$icons/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$icons" -o "$app/Contents/Resources/AppIcon.icns"
codesign --force --sign - "$app"
codesign --verify --strict "$app"
echo "$app"
