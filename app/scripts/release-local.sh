#!/bin/bash
# Local release: build, sign, (notarize), put the DMG at the two fixed places, install to /Applications, relaunch
# (D-124, D-160, D-179). Notarized builds need DDUCK_SIGN_IDENTITY and DDUCK_NOTARY_PROFILE.
#   app/target/release/bundle/dmg/D.Duck_<version>_<arch>.dmg   (tauri's own output path)
#   publish/dmg/D.Duck-latest_<arch>.dmg                         (fixed name for sharing; replaced every time)
# Older builds are moved to the Trash, never kept side by side.
#
# Modes:
#   DDUCK_SIGN_IDENTITY and DDUCK_NOTARY_PROFILE both set -> Developer ID + hardened runtime + Apple notarization
#   neither set                                           -> ad-hoc signature (not notarized), as before
#   only one set                                          -> refuse
# DDUCK_SIGN_IDENTITY is the certificate name ("Developer ID Application: ..."); DDUCK_NOTARY_PROFILE is the name of
# a keychain profile made once with `xcrun notarytool store-credentials`. No Apple ID, password or key is read here.
#
# Order (design decision 0): build -> dSYM -> sign (inside out) -> notarize+staple app -> DMG in a temp dir ->
# sign+notarize+staple DMG -> verify -> only then replace bundle/dmg, publish/dmg and /Applications.
# Any failure before the last step leaves those three untouched; the temp dir is removed by the trap.
#
# Test hooks (app/scripts/test-release-local.sh): DDUCK_SKIP_BUILD=1 uses the app already in bundle/macos;
# DDUCK_APPLICATIONS replaces /Applications as the install location. Defaults are the normal behavior.
set -euo pipefail
APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$APP_DIR/.." && pwd)"
BUNDLE="$APP_DIR/target/release/bundle"
PUBLISH="$ROOT/publish/dmg"
INSTALL_DIR="${DDUCK_APPLICATIONS:-/Applications}"
ENTITLEMENTS="$APP_DIR/src-tauri/entitlements.plist"
NOTARY_TIMEOUT="30m"

die() { echo "error: $*" >&2; exit 1; }

# --- Mode -------------------------------------------------------------------------------------------------------
SIGN_ID="${DDUCK_SIGN_IDENTITY:-}"
NOTARY_PROFILE="${DDUCK_NOTARY_PROFILE:-}"
if [ -n "$SIGN_ID" ] && [ -n "$NOTARY_PROFILE" ]; then
  MODE=notarized
elif [ -z "$SIGN_ID" ] && [ -z "$NOTARY_PROFILE" ]; then
  MODE=adhoc
elif [ -z "$SIGN_ID" ]; then
  die "DDUCK_NOTARY_PROFILE is set but DDUCK_SIGN_IDENTITY is missing. Set both to notarize, or neither for an ad-hoc build."
else
  die "DDUCK_SIGN_IDENTITY is set but DDUCK_NOTARY_PROFILE is missing. Set both to notarize, or neither for an ad-hoc build."
fi
if [ "$MODE" = notarized ]; then
  [ -f "$ENTITLEMENTS" ] || die "missing $ENTITLEMENTS"
fi

# Keep tauri from signing or notarizing on its own with whatever Apple credentials the shell happens to hold.
unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH \
  APPLE_SIGNING_IDENTITY APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD

# --- Names ------------------------------------------------------------------------------------------------------
VERSION="$(node -p "require(process.argv[1]).version" "$APP_DIR/src-tauri/tauri.conf.json")"
[[ "$VERSION" =~ ^[0-9A-Za-z.+-]+$ ]] || die "unexpected version in tauri.conf.json: $VERSION"
case "$(uname -m)" in
  arm64) ARCH=aarch64 ;;
  x86_64) ARCH=x64 ;;
  *) ARCH="$(uname -m)" ;;
esac
FIXED_DMG="$BUNDLE/dmg/D.Duck_${VERSION}_${ARCH}.dmg"
SHARE_DMG="$PUBLISH/D.Duck-latest_${ARCH}.dmg"

TMP_ROOT="${TMPDIR:-/tmp}"
WORK="$(mktemp -d "${TMP_ROOT%/}/dduck-release.XXXXXX")"
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

# --- 1. Build ---------------------------------------------------------------------------------------------------
cd "$APP_DIR"
if [ "${DDUCK_SKIP_BUILD:-}" = 1 ]; then
  echo "DDUCK_SKIP_BUILD=1: using the existing app bundle" >&2
else
  npx tauri build --bundles app
fi
APP="$BUNDLE/macos/D.Duck.app"
[ -d "$APP" ] || die "build produced no app"

# --- 2. dSYM ----------------------------------------------------------------------------------------------------
# Keep the symbols of this exact binary (D-161), named by its UUID so a sample of a hung app can be symbolicated:
#   atos -o publish/dsym/<uuid>.dSYM -l <load address> <addresses>
# Signing does not change the UUID, so this can run before signing.
DSYM_SRC="$APP_DIR/target/release/dduck.dSYM"
DSYM_DIR="$ROOT/publish/dsym"
UUID="$(dwarfdump --uuid "$APP/Contents/MacOS/dduck" | awk '{print $2; exit}')"
if [ -d "$DSYM_SRC" ] && [ -n "$UUID" ]; then
  mkdir -p "$DSYM_DIR"
  rm -rf "${DSYM_DIR:?}/$UUID.dSYM"
  ditto "$DSYM_SRC" "$DSYM_DIR/$UUID.dSYM"
  dwarfdump --uuid "$DSYM_DIR/$UUID.dSYM" | grep -q "$UUID" || die "dSYM does not match the app binary"
  # Keep the last 10 builds' symbols; older ones go to the Trash.
  # shellcheck disable=SC2012  # names are UUID.dSYM, no odd characters
  ls -1t "$DSYM_DIR" | tail -n +11 | while read -r old; do mv "$DSYM_DIR/$old" ~/.Trash/ 2>/dev/null || true; done
else
  echo "warning: no dSYM for this build (symbols not kept)" >&2
fi

# --- 3. Sign ----------------------------------------------------------------------------------------------------
# Path depth, for deepest-first ordering.
by_depth_desc() { awk -F/ '{print NF "\t" $0}' | sort -rn -k1,1 | cut -f2-; }

sign_developer_id() {
  local app="$1" f
  # Loose Mach-O files anywhere in the bundle (today only Contents/MacOS/dduck), deepest first.
  find "$app/Contents" -type f | by_depth_desc | while IFS= read -r f; do
    if file -b "$f" | grep -q 'Mach-O'; then
      echo "sign: ${f#"$app"/}" >&2
      codesign --force --options runtime --timestamp --sign "$SIGN_ID" "$f"
    fi
  done
  # Nested bundles (frameworks, helpers, plug-ins), deepest first, so each seals already-signed contents.
  find "$app/Contents" -mindepth 1 -type d \
    \( -name '*.framework' -o -name '*.app' -o -name '*.xpc' -o -name '*.appex' -o -name '*.bundle' \) \
    | by_depth_desc | while IFS= read -r f; do
      echo "sign: ${f#"$app"/}" >&2
      codesign --force --options runtime --timestamp --sign "$SIGN_ID" "$f"
    done
  # The app itself last, the only place the entitlements go. No --deep (Apple's advice; keeps entitlements off
  # the inner binaries).
  echo "sign: D.Duck.app (entitlements)" >&2
  codesign --force --options runtime --timestamp --entitlements "$ENTITLEMENTS" --sign "$SIGN_ID" "$app"
}

if [ "$MODE" = notarized ]; then
  sign_developer_id "$APP"
else
  # Sign the whole bundle ad-hoc (D-160): the linker signs only the binary, and a downloaded copy of a bundle whose
  # signature lacks its resources is reported by Gatekeeper as "damaged".
  codesign --force --deep --sign - "$APP"
fi
codesign --verify --deep --strict "$APP"

# --- Notarization helper ----------------------------------------------------------------------------------------
# notarize <file>: submit, wait up to $NOTARY_TIMEOUT, succeed only on "Accepted".
json_field() { plutil -extract "$1" raw -o - "$2" 2>/dev/null || true; }
notarize() {
  local file="$1" name out rc=0 id status
  name="$(basename "$1")"
  out="$WORK/notary-$name.json"
  echo "notarize: $(basename "$file") (waiting up to $NOTARY_TIMEOUT)" >&2
  xcrun notarytool submit "$file" --keychain-profile "$NOTARY_PROFILE" \
    --wait --timeout "$NOTARY_TIMEOUT" --output-format json >"$WORK/notary.raw" || rc=$?
  # Keep the last one-line JSON object (the final status); otherwise try the whole output as one document.
  grep '^{.*}$' "$WORK/notary.raw" | tail -n 1 >"$out" || true
  [ -s "$out" ] || cp "$WORK/notary.raw" "$out"
  id="$(json_field id "$out")"
  status="$(json_field status "$out")"
  if [ -z "$id" ]; then
    cat "$WORK/notary.raw" >&2
    die "notarytool submit failed (exit $rc) before a submission id was returned"
  fi
  echo "notarize: submission $id status ${status:-unknown}" >&2
  case "$status" in
    Accepted)
      [ "$rc" -eq 0 ] || die "notarytool exited $rc although status is Accepted (submission $id)"
      ;;
    Invalid | Rejected)
      echo "--- notarytool log $id ---" >&2
      xcrun notarytool log "$id" --keychain-profile "$NOTARY_PROFILE" >&2 || true
      die "notarization $status for $(basename "$file") (submission $id)"
      ;;
    *)
      echo "Notarization did not finish within $NOTARY_TIMEOUT. Check it later with:" >&2
      echo "  xcrun notarytool info $id --keychain-profile \"$NOTARY_PROFILE\"" >&2
      echo "  xcrun notarytool log $id --keychain-profile \"$NOTARY_PROFILE\"" >&2
      die "notarization not finished for $(basename "$file") (submission $id, status ${status:-unknown})"
      ;;
  esac
}

# --- 4. Notarize and staple the app -----------------------------------------------------------------------------
# The app carries its own ticket so a copy dragged out of the DMG also passes Gatekeeper offline.
if [ "$MODE" = notarized ]; then
  ditto -c -k --keepParent "$APP" "$WORK/D.Duck.zip"
  notarize "$WORK/D.Duck.zip"
  xcrun stapler staple "$APP"
fi

# --- 5. DMG in the temp dir -------------------------------------------------------------------------------------
# hdiutil, not tauri's bundle_dmg.sh (it fails when Finder holds the image).
STAGE="$WORK/stage"
mkdir "$STAGE"
ditto "$APP" "$STAGE/D.Duck.app"
ln -s /Applications "$STAGE/Applications"
TMP_DMG="$WORK/D.Duck.dmg"
hdiutil create -volname "D.Duck" -srcfolder "$STAGE" -ov -format UDZO "$TMP_DMG" -quiet
hdiutil verify "$TMP_DMG" -quiet

# --- 6. Sign, notarize and staple the DMG -----------------------------------------------------------------------
if [ "$MODE" = notarized ]; then
  codesign --force --timestamp --sign "$SIGN_ID" "$TMP_DMG"
  notarize "$TMP_DMG"
  xcrun stapler staple "$TMP_DMG"
fi

# --- 7. Verify --------------------------------------------------------------------------------------------------
codesign --verify --deep --strict "$APP"
if [ "$MODE" = notarized ]; then
  codesign --verify --strict "$TMP_DMG"
  spctl -a -vv -t exec "$APP" 2>"$WORK/spctl-app.txt" || { cat "$WORK/spctl-app.txt" >&2; die "Gatekeeper rejected the app"; }
  cat "$WORK/spctl-app.txt" >&2
  grep -q 'source=Notarized Developer ID' "$WORK/spctl-app.txt" || die "app is not reported as Notarized Developer ID"
  spctl -a -vv -t open --context context:primary-signature "$TMP_DMG" 2>"$WORK/spctl-dmg.txt" \
    || { cat "$WORK/spctl-dmg.txt" >&2; die "Gatekeeper rejected the DMG"; }
  cat "$WORK/spctl-dmg.txt" >&2
  xcrun stapler validate "$APP" || die "no stapled ticket on the app"
  xcrun stapler validate "$TMP_DMG" || die "no stapled ticket on the DMG"
else
  echo "note: ad-hoc signed, NOT notarized. On another Mac the first open needs right-click > Open" \
    "(or System Settings > Privacy & Security > Open Anyway). Set DDUCK_SIGN_IDENTITY and DDUCK_NOTARY_PROFILE to notarize." >&2
fi

# --- 8. Promote: bundle/dmg, publish/dmg, /Applications ---------------------------------------------------------
mkdir -p "$BUNDLE/dmg" "$PUBLISH"
cp -f "$TMP_DMG" "$FIXED_DMG.partial" && mv -f "$FIXED_DMG.partial" "$FIXED_DMG"
cp -f "$TMP_DMG" "$SHARE_DMG.partial" && mv -f "$SHARE_DMG.partial" "$SHARE_DMG"

# Anything else in bundle/dmg or publish/dmg is an older build: to the Trash.
for f in "$BUNDLE/dmg"/*.dmg "$PUBLISH"/*.dmg; do
  [ -e "$f" ] || continue
  [ "$f" = "$FIXED_DMG" ] || [ "$f" = "$SHARE_DMG" ] || mv "$f" ~/.Trash/ 2>/dev/null || true
done

# Install: quit, swap, relaunch. The replaced app goes to the Trash.
osascript -e 'quit app "D.Duck"' >/dev/null 2>&1 || true
sleep 2
if [ -d "$INSTALL_DIR/D.Duck.app" ]; then mv "$INSTALL_DIR/D.Duck.app" ~/.Trash/"D.Duck.app-$(date +%Y%m%d-%H%M%S)"; fi
ditto "$APP" "$INSTALL_DIR/D.Duck.app"
open "$INSTALL_DIR/D.Duck.app"

shasum -a 256 "$SHARE_DMG"
echo "installed $INSTALL_DIR/D.Duck.app ($MODE) · $FIXED_DMG · $SHARE_DMG"
