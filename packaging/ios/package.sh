#!/usr/bin/env bash
# Builds Horizon for iPhone and iPad: Horizon.app with the game, its prepared resources and the
# icon, signed for a device or for the simulator.
#
#   packaging/ios/package.sh device|simulator RESOURCES OUTDIR
#
# RESOURCES is horizon-prepare's folder (.local/horizon-client/26.3): the game finds it as
# `horizon-resources` beside its binary (Options::read in game.rs). It is made from the player's
# own copy of Minecraft, so the app is for the player's own devices: keep OUTDIR under .local/.
#
# device: Horizon.app and Horizon.ipa, signed with IOS_SIGN_IDENTITY ("Apple Development: …") and
# the provisioning profile IOS_PROFILE, which must cover IOS_BUNDLE_ID (default dev.horizon.ios) and
# the device. Without IOS_PROFILE they are signed ad hoc, for a sideloading tool (Sideloadly,
# AltStore) to sign again with the player's Apple ID as it installs the .ipa.
# simulator: signed ad hoc.
# Needs Xcode and `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
kind=${1:-}
bundle_id=${IOS_BUNDLE_ID:-dev.horizon.ios}
minimum_os=16.0
case "$kind" in
  device) target=aarch64-apple-ios platform=iPhoneOS sdk=iphoneos ;;
  simulator) target=aarch64-apple-ios-sim platform=iPhoneSimulator sdk=iphonesimulator ;;
  *) echo "usage: $0 device|simulator RESOURCES OUTDIR" >&2; exit 2 ;;
esac
resources=$(cd "$2" && pwd)
out=$(mkdir -p "$3" && cd "$3" && pwd)
[[ -f "$resources/horizon-resources.json" ]] || { echo "$resources holds no prepared resources" >&2; exit 1; }

# gilrs and hidapi have no iOS backend: no `controllers`. Code mods run on wasmtime's Pulley
# interpreter there (horizon-mods' runtime.rs).
(cd "$root" && IPHONEOS_DEPLOYMENT_TARGET=$minimum_os cargo build --profile fast --target "$target" \
  -p horizon-client --bin horizon-client --no-default-features --features audio,mods)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/crates/horizon-client/Cargo.toml" | head -1)

app=$out/$kind/Horizon.app
rm -rf "$app"
mkdir -p "$app"
target_dir=${CARGO_TARGET_DIR:-target}
[[ "$target_dir" == /* ]] || target_dir=$root/$target_dir
cp "$target_dir/$target/fast/horizon-client" "$app/"
sed -e "s/@BUNDLE_ID@/$bundle_id/" -e "s/@VERSION@/$version/g" -e "s/@PLATFORM@/$platform/" \
  -e "s/@MINIMUM_OS@/$minimum_os/" "$here/Info.plist" > "$app/Info.plist"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
# The home screen icon: the macOS icon's 1024-pixel image, compiled into Assets.car.
iconutil -c iconset -o "$work/horizon.iconset" "$root/packaging/icons/horizon.icns"
icons=$work/Assets.xcassets/AppIcon.appiconset
mkdir -p "$icons"
cp "$work/horizon.iconset/icon_512x512@2x.png" "$icons/icon.png"
echo '{"images":[{"filename":"icon.png","idiom":"universal","platform":"ios","size":"1024x1024"}],"info":{"author":"xcode","version":1}}' \
  > "$icons/Contents.json"
xcrun actool --compile "$app" --platform "$sdk" --minimum-deployment-target "$minimum_os" \
  --app-icon AppIcon --output-partial-info-plist "$work/icon.plist" "$work/Assets.xcassets" >/dev/null
/usr/libexec/PlistBuddy -c "Merge $work/icon.plist" "$app/Info.plist"
plutil -lint "$app/Info.plist"
# Cloned, not copied, on APFS: the resources take no space twice.
cp -Rc "$resources" "$app/horizon-resources"

if [[ "$kind" == device && -n "${IOS_PROFILE:-}" ]]; then
  cp "$IOS_PROFILE" "$app/embedded.mobileprovision"
  security cms -D -i "$IOS_PROFILE" > "$work/profile.plist"
  /usr/libexec/PlistBuddy -x -c "Print :Entitlements" "$work/profile.plist" > "$work/entitlements.plist"
  # A wildcard profile's application-identifier is TEAM.*: name this app.
  team=$(/usr/libexec/PlistBuddy -c "Print :TeamIdentifier:0" "$work/profile.plist")
  /usr/libexec/PlistBuddy -c "Set :application-identifier $team.$bundle_id" "$work/entitlements.plist"
  codesign --force --sign "$IOS_SIGN_IDENTITY" --entitlements "$work/entitlements.plist" "$app"
else
  codesign --force --sign - "$app"
fi
codesign --verify --strict --verbose=2 "$app"
echo "Packaged $app"
if [[ "$kind" == device ]]; then
  # An .ipa is the app in a Payload folder, zipped.
  ipa=$out/device/Horizon.ipa
  rm -f "$ipa"
  mkdir "$work/Payload"
  cp -Rc "$app" "$work/Payload/"
  (cd "$work" && zip -qry "$ipa" Payload)
  echo "Packaged $ipa"
fi
