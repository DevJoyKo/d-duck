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
# Any failure before the last step leaves those three untouched; the temp dir and any half-made copy in the three
# locations (*.partial, .D.Duck.app.new) are removed by the trap.
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
NEW_APP="$INSTALL_DIR/.D.Duck.app.new"   # staged copy, swapped in only after it is complete and verified

TMP_ROOT="${TMPDIR:-/tmp}"
WORK="$(mktemp -d "${TMP_ROOT%/}/dduck-release.XXXXXX")"
cleanup() { rm -rf "$WORK" "$NEW_APP"; rm -f "$FIXED_DMG.partial" "$SHARE_DMG.partial"; }
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
    if [[ "$(file -b "$f")" == *Mach-O* ]]; then  # no `| grep -q`: under pipefail its early exit can SIGPIPE file
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
# notarize <file>: submit without waiting so the submission id is printed at once (an interrupted wait no longer loses
# it), then `notarytool wait` up to $NOTARY_TIMEOUT, then `notarytool info` for the status. Succeeds only on "Accepted".
# plutil prints its parse error on stdout, so only its output on success counts.
json_field() { local v; v="$(plutil -extract "$1" raw -o - "$2" 2>/dev/null)" || v=""; printf '%s' "$v"; }
# Keep the last one-line JSON object of a notarytool run; otherwise try the whole output as one document.
last_json() { grep '^{.*}$' "$1" | tail -n 1 >"$2" || true; [ -s "$2" ] || cp "$1" "$2"; }
notary_hint() {
  echo "Submission $1. Check it with:" >&2
  echo "  xcrun notarytool info $1 --keychain-profile \"$NOTARY_PROFILE\"" >&2
  echo "  xcrun notarytool log $1 --keychain-profile \"$NOTARY_PROFILE\"" >&2
  echo "  xcrun notarytool history --keychain-profile \"$NOTARY_PROFILE\"" >&2
}
notarize() {
  local file="$1" name rc=0 id status
  name="$(basename "$1")"
  echo "notarize: submitting $name" >&2
  xcrun notarytool submit "$file" --keychain-profile "$NOTARY_PROFILE" --output-format json \
    >"$WORK/submit-$name.raw" 2>"$WORK/submit-$name.err" || rc=$?
  last_json "$WORK/submit-$name.raw" "$WORK/submit-$name.json"
  id="$(json_field id "$WORK/submit-$name.json")"
  [[ "$id" =~ ^[0-9A-Za-z-]+$ ]] || id=""   # notarytool ids are UUIDs; anything else is not an id
  if [ -z "$id" ]; then
    cat "$WORK/submit-$name.raw" "$WORK/submit-$name.err" >&2
    echo "If the upload went through anyway, it is listed by: xcrun notarytool history --keychain-profile \"$NOTARY_PROFILE\"" >&2
    die "notarytool submit failed (exit $rc) for $name: no submission id was returned."
  fi
  echo "notarize: submission $id for $name (waiting up to $NOTARY_TIMEOUT)" >&2
  if [ "$rc" -ne 0 ]; then
    cat "$WORK/submit-$name.err" >&2
    notary_hint "$id"
    die "notarytool submit exited $rc for $name (submission $id)"
  fi
  rc=0
  xcrun notarytool wait "$id" --timeout "$NOTARY_TIMEOUT" --keychain-profile "$NOTARY_PROFILE" --output-format json \
    >"$WORK/wait-$name.raw" 2>&1 || rc=$?
  xcrun notarytool info "$id" --keychain-profile "$NOTARY_PROFILE" --output-format json \
    >"$WORK/info-$name.raw" 2>"$WORK/info-$name.err" || true
  last_json "$WORK/info-$name.raw" "$WORK/info-$name.json"
  status="$(json_field status "$WORK/info-$name.json")"
  echo "notarize: submission $id status ${status:-unknown}" >&2
  case "$status" in
    Accepted) ;;
    Invalid | Rejected)
      echo "--- notarytool log $id ---" >&2
      xcrun notarytool log "$id" --keychain-profile "$NOTARY_PROFILE" >&2 || true
      notary_hint "$id"
      die "notarization $status for $name (submission $id)"
      ;;
    *)
      cat "$WORK/wait-$name.raw" "$WORK/info-$name.err" >&2
      echo "Notarization did not finish (wait exit $rc, limit $NOTARY_TIMEOUT)." >&2
      notary_hint "$id"
      die "notarization not finished for $name (submission $id, status ${status:-unknown})"
      ;;
  esac
}

# --- 4. Notarize and staple the app -----------------------------------------------------------------------------
# The app carries its own ticket so a copy dragged out of the DMG also passes Gatekeeper offline.
if [ "$MODE" = notarized ]; then
  ditto -c -k --keepParent "$APP" "$WORK/D.Duck.zip"
  notarize "$WORK/D.Duck.zip"
  xcrun stapler staple "$APP" || die "stapling the ticket to the app failed"
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
  xcrun stapler staple "$TMP_DMG" || die "stapling the ticket to the DMG failed"
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
  grep -q 'source=Notarized Developer ID' "$WORK/spctl-dmg.txt" || die "DMG is not reported as Notarized Developer ID"
  xcrun stapler validate "$APP" || die "no stapled ticket on the app"
  xcrun stapler validate "$TMP_DMG" || die "no stapled ticket on the DMG"
else
  echo "note: ad-hoc signed, NOT notarized. On another Mac the first open is blocked; allow it in" \
    "System Settings > Privacy & Security > Open Anyway. Set DDUCK_SIGN_IDENTITY and DDUCK_NOTARY_PROFILE to notarize." >&2
fi

# --- 8. Promote: bundle/dmg, publish/dmg, /Applications ---------------------------------------------------------
# Stage every copy next to its target first (same volume, so the final mv is a rename). A failed or unverified copy
# stops here with all three locations as they were; the trap removes the staged copies.
install_kept() { die "$* The installed app and both DMGs were left as they were."; }
rm -rf "$NEW_APP"
ditto "$APP" "$NEW_APP" || install_kept "copying the app to $INSTALL_DIR failed."
codesign --verify --deep --strict "$NEW_APP" || install_kept "the copy in $INSTALL_DIR does not verify."
mkdir -p "$BUNDLE/dmg" "$PUBLISH" || install_kept "cannot create the DMG folders."
cp -f "$TMP_DMG" "$FIXED_DMG.partial" || install_kept "copying the DMG to $BUNDLE/dmg failed."
cp -f "$TMP_DMG" "$SHARE_DMG.partial" || install_kept "copying the DMG to $PUBLISH failed."
mv -f "$FIXED_DMG.partial" "$FIXED_DMG"
mv -f "$SHARE_DMG.partial" "$SHARE_DMG"

# Anything else in bundle/dmg or publish/dmg is an older build: to the Trash.
for f in "$BUNDLE/dmg"/*.dmg "$PUBLISH"/*.dmg; do
  [ -e "$f" ] || continue
  [ "$f" = "$FIXED_DMG" ] || [ "$f" = "$SHARE_DMG" ] || mv "$f" ~/.Trash/ 2>/dev/null || true
done

# Install: quit, swap, relaunch. The replaced app goes to the Trash; if the swap fails it is put back.
osascript -e 'quit app "D.Duck"' >/dev/null 2>&1 || true
sleep 2
OLD_IN_TRASH=""
if [ -d "$INSTALL_DIR/D.Duck.app" ]; then
  OLD_IN_TRASH="$HOME/.Trash/D.Duck.app-$(date +%Y%m%d-%H%M%S)"
  mv "$INSTALL_DIR/D.Duck.app" "$OLD_IN_TRASH" \
    || die "could not move the installed app to the Trash; it was left as it was (the DMGs are already updated)."
fi
if ! mv "$NEW_APP" "$INSTALL_DIR/D.Duck.app"; then
  if [ -n "$OLD_IN_TRASH" ] && mv "$OLD_IN_TRASH" "$INSTALL_DIR/D.Duck.app"; then
    die "could not move the new app into $INSTALL_DIR; the previous app was put back."
  fi
  die "could not move the new app into $INSTALL_DIR; the previous app is in the Trash: ${OLD_IN_TRASH:-none}"
fi
open "$INSTALL_DIR/D.Duck.app"

shasum -a 256 "$SHARE_DMG"
echo "installed $INSTALL_DIR/D.Duck.app ($MODE) · $FIXED_DMG · $SHARE_DMG"
