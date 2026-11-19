#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
app="$PWD/dist/OC Setup Easy.app"
codesign --verify --strict "$app"
version=$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$app/Contents/Info.plist")
arch=$(lipo -archs "$app/Contents/MacOS/oc-setup-easy")
[[ "$arch" != *" "* ]] || arch=Universal
name="OC-Setup-Easy-$version-macOS-$arch"
cache="$PWD/target/packaging"
mkdir -p "$cache"
if [[ ! -x "$cache/dmg-env/bin/dmgbuild" ]]; then
  python3 -m venv "$cache/dmg-env"
  "$cache/dmg-env/bin/pip" install 'dmgbuild==1.6.5'
fi
sips -s format png assets/install.svg --out "$cache/install.png" >/dev/null
stage=$(mktemp -d "$PWD/dist/.package.XXXXXX")
trap 'rm -rf "$stage"' EXIT
ditto -c -k --sequesterRsrc --keepParent "$app" "$stage/$name.zip"
"$cache/dmg-env/bin/dmgbuild" -s scripts/dmg-settings.py -D "app=$app" -D "background=$cache/install.png" "OC Setup Easy" "$stage/$name.dmg"
hdiutil verify "$stage/$name.dmg" >/dev/null
(cd "$stage" && shasum -a 256 "$name.zip" "$name.dmg" >SHA256SUMS.txt)
mv "$stage/$name.zip" "$stage/$name.dmg" "$stage/SHA256SUMS.txt" dist/
