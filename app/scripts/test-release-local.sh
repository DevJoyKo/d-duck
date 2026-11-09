#!/bin/bash
# Shell test harness for release-local.sh (change notarized-release, task 1.3).
# Runs the script in a throwaway tree with fake codesign / xcrun (notarytool, stapler) / spctl / hdiutil / dwarfdump /
# osascript / open / sleep / npx first on PATH. Nothing is built, signed, submitted, installed or opened for real:
# /Applications is replaced by DDUCK_APPLICATIONS and the Trash by a fake HOME.
#   app/scripts/test-release-local.sh [work-dir]
set -uo pipefail
SCRIPT_SRC="$(cd "$(dirname "$0")" && pwd)/release-local.sh"
CONF_SRC="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/tauri.conf.json"
ENT_SRC="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/entitlements.plist"
BASE="${1:-$(mktemp -d)}"
mkdir -p "$BASE"
BASE="$(cd "$BASE" && pwd)"
NODE_DIR="$(dirname "$(command -v node)")"
VERSION="$(node -p "require(process.argv[1]).version" "$CONF_SRC")"
case "$(uname -m)" in arm64) ARCH=aarch64 ;; x86_64) ARCH=x64 ;; *) ARCH="$(uname -m)" ;; esac

PASS=0
FAIL=0
ok() { PASS=$((PASS + 1)); echo "  ok   $*"; }
bad() { FAIL=$((FAIL + 1)); echo "  FAIL $*"; }
check() { local d="$1"; shift; if "$@"; then ok "$d"; else bad "$d"; fi; }

write_fakes() {
  local bin="$1"
  mkdir -p "$bin"
  # Every fake logs "<tool> <args> | APPLE_ID=<value or unset>" to $CALLS.
  cat >"$bin/_log" <<'EOF'
#!/bin/bash
echo "$* | APPLE_ID=${APPLE_ID-unset}" >>"$CALLS"
EOF
  for t in codesign dwarfdump osascript open sleep npx spctl hdiutil xcrun; do
    # shellcheck disable=SC2016  # the fake's own code, expanded when the fake runs
    printf '#!/bin/bash\n"$(dirname "$0")/_log" %s "$@"\n' "$t" >"$bin/$t"
  done
  cat >>"$bin/dwarfdump" <<'EOF'
echo "UUID: 11111111-2222-3333-4444-555555555555 (arm64) ${2:-}"
EOF
  cat >>"$bin/npx" <<'EOF'
[ "${DDUCK_SKIP_BUILD:-}" = 1 ] && { echo "npx called although DDUCK_SKIP_BUILD=1" >&2; exit 99; }
exit 0
EOF
  cat >>"$bin/hdiutil" <<'EOF'
if [ "$1" = create ]; then
  for a in "$@"; do case "$a" in *.dmg) echo "NEW-DMG" >"$a" ;; esac; done
fi
exit 0
EOF
  cat >>"$bin/spctl" <<'EOF'
for a in "$@"; do last="$a"; done
if [ "${FAKE_SPCTL:-accept}" = reject ]; then echo "$last: rejected" >&2; exit 3; fi
echo "$last: accepted" >&2
echo "source=Notarized Developer ID" >&2
exit 0
EOF
  cat >>"$bin/xcrun" <<'EOF'
tool="$1"; shift
case "$tool" in
  notarytool)
    case "$1" in
      submit)
        n=$(( $(cat "$NOTARY_COUNT" 2>/dev/null || echo 0) + 1 )); echo "$n" >"$NOTARY_COUNT"
        outcome="$(echo "${FAKE_NOTARY:-accepted}" | cut -d, -f"$n")"; outcome="${outcome:-accepted}"
        echo "Conducting pre-submission checks..." >&2
        case "$outcome" in
          accepted) echo "{\"id\":\"sub-$n\",\"status\":\"Accepted\",\"message\":\"Processing complete\"}"; exit 0 ;;
          invalid)  echo "{\"id\":\"sub-$n\",\"status\":\"Invalid\",\"message\":\"Processing complete\"}"; exit 1 ;;
          timeout)  echo "{\"id\":\"sub-$n\",\"status\":\"In Progress\",\"message\":\"Timeout\"}"; exit 1 ;;
          *)        echo "Error: HTTP 401" >&2; exit 1 ;;
        esac ;;
      log) echo "FAKE NOTARY LOG for $2: The binary is not signed with a valid Developer ID certificate." ;;
    esac ;;
  stapler) [ "${FAKE_STAPLER:-ok}" = ok ] || exit 65 ;;
  *) echo "unexpected xcrun $tool" >&2; exit 2 ;;
esac
exit 0
EOF
  chmod +x "$bin"/*
}

# Fresh tree per scenario: previous DMGs at both fixed places, a previous installed app, a built app to release.
setup() {
  SB="$BASE/$1"
  rm -rf "$SB"
  mkdir -p "$SB/tmp" "$SB/home/.Trash" "$SB/Applications/D.Duck.app" "$SB/repo/publish/dmg" \
    "$SB/repo/app/scripts" "$SB/repo/app/src-tauri"
  cp "$SCRIPT_SRC" "$SB/repo/app/scripts/release-local.sh"
  cp "$CONF_SRC" "$ENT_SRC" "$SB/repo/app/src-tauri/"
  local b="$SB/repo/app/target/release/bundle"
  mkdir -p "$b/dmg" "$b/macos/D.Duck.app/Contents/MacOS" "$b/macos/D.Duck.app/Contents/Resources" \
    "$SB/repo/app/target/release/dduck.dSYM/Contents"
  cp /usr/bin/true "$b/macos/D.Duck.app/Contents/MacOS/dduck"
  echo "<plist/>" >"$b/macos/D.Duck.app/Contents/Info.plist"
  echo "icon" >"$b/macos/D.Duck.app/Contents/Resources/icon.icns"
  echo "OLD-BUNDLE" >"$b/dmg/D.Duck_${VERSION}_${ARCH}.dmg"
  echo "OLD-SHARE" >"$SB/repo/publish/dmg/D.Duck-latest_${ARCH}.dmg"
  echo "OLD-APP" >"$SB/Applications/D.Duck.app/marker"
  FIXED="$b/dmg/D.Duck_${VERSION}_${ARCH}.dmg"
  SHARE="$SB/repo/publish/dmg/D.Duck-latest_${ARCH}.dmg"
  CALLS="$SB/calls.log"
  : >"$CALLS"
  write_fakes "$SB/bin"
}

# run [VAR=value ...]: runs the copied script with fakes; sets RC and OUT.
run() {
  OUT="$SB/out.log"
  env -i HOME="$SB/home" TMPDIR="$SB/tmp/" PATH="$SB/bin:$NODE_DIR:/usr/bin:/bin:/usr/sbin:/sbin" \
    CALLS="$CALLS" NOTARY_COUNT="$SB/notary.count" DDUCK_APPLICATIONS="$SB/Applications" \
    APPLE_ID=leak@example.invalid APPLE_PASSWORD=leak "$@" \
    /bin/bash "$SB/repo/app/scripts/release-local.sh" >"$OUT" 2>&1
  RC=$?
}

unchanged() {
  [ "$(cat "$FIXED")" = OLD-BUNDLE ] && [ "$(cat "$SHARE")" = OLD-SHARE ] \
    && [ "$(cat "$SB/Applications/D.Duck.app/marker" 2>/dev/null)" = OLD-APP ] \
    && [ -z "$(ls -A "$SB/home/.Trash")" ]
}
replaced() {
  [ "$(cat "$FIXED")" = NEW-DMG ] && [ "$(cat "$SHARE")" = NEW-DMG ] \
    && [ ! -e "$SB/Applications/D.Duck.app/marker" ] && [ -x "$SB/Applications/D.Duck.app/Contents/MacOS/dduck" ] \
    && compgen -G "$SB/home/.Trash/D.Duck.app-*" >/dev/null
}
temp_clean() { [ -z "$(ls -A "$SB/tmp")" ] && grep -qF "$SB/tmp/dduck-release." "$CALLS"; }  # temp dir was used, then removed
out_has() { grep -qF -- "$1" "$OUT"; }
calls_has() { grep -qF -- "$1" "$CALLS"; }
calls_lacks() { ! grep -qF -- "$1" "$CALLS"; }
# in_order pattern...: each fixed string appears on a later calls.log line than the previous one.
in_order() {
  local from=0 p n
  for p in "$@"; do
    n="$(awk -v from="$from" -v p="$p" 'NR > from && index($0, p) { print NR; exit }' "$CALLS")"
    [ -n "$n" ] || { echo "    missing after line $from: $p"; return 1; }
    from="$n"
  done
}
show_on_fail() { [ "$FAIL" -eq "$1" ] || { echo "    --- output ---"; sed 's/^/    /' "$OUT"; echo "    --- calls ---"; sed 's/^/    /' "$CALLS"; }; }

ID="Developer ID Application: Test (TEAMID0000)"

echo "(a) neither variable -> ad-hoc, as before"
setup a; f=$FAIL; run
check "exit 0" [ "$RC" -eq 0 ]
check "tauri build ran with APPLE_* unset" calls_has "npx tauri build --bundles app | APPLE_ID=unset"
check "ad-hoc --deep signature" calls_has "codesign --force --deep --sign - "
check "no notarytool/stapler/spctl" calls_lacks "xcrun" ; check "no spctl" calls_lacks "spctl"
check "not-notarized notice" out_has "NOT notarized"
check "three locations replaced, old app in Trash" replaced
check "relaunched" calls_has "open $SB/Applications/D.Duck.app"
check "dSYM kept by UUID" [ -d "$SB/repo/publish/dsym/11111111-2222-3333-4444-555555555555.dSYM" ]
check "temp dir cleaned" temp_clean
show_on_fail "$f"

echo "(b) only one variable -> refuse"
setup b1; f=$FAIL; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_SKIP_BUILD=1
check "identity only: exit non-zero" [ "$RC" -ne 0 ]
check "names DDUCK_NOTARY_PROFILE as missing" out_has "DDUCK_NOTARY_PROFILE is missing"
check "nothing signed or built" [ ! -s "$CALLS" ]
check "three locations unchanged" unchanged
setup b2; run DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1
check "profile only: exit non-zero" [ "$RC" -ne 0 ]
check "names DDUCK_SIGN_IDENTITY as missing" out_has "DDUCK_SIGN_IDENTITY is missing"
check "three locations unchanged" unchanged
show_on_fail "$f"

echo "(c) notarization Invalid -> fail, log shown, three locations unchanged"
setup c1; f=$FAIL; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_NOTARY=invalid
check "app Invalid: exit non-zero" [ "$RC" -ne 0 ]
check "notarytool log shown" out_has "FAKE NOTARY LOG for sub-1"
check "log fetched with the profile" calls_has "xcrun notarytool log sub-1 --keychain-profile dduck-notary"
check "nothing stapled" calls_lacks "stapler staple"
check "three locations unchanged" unchanged
check "temp dir cleaned" temp_clean
setup c2; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_NOTARY=accepted,invalid
check "DMG Invalid: exit non-zero" [ "$RC" -ne 0 ]
check "DMG log shown" out_has "FAKE NOTARY LOG for sub-2"
check "three locations unchanged" unchanged
check "temp dir cleaned" temp_clean
show_on_fail "$f"

echo "(d) notarization timeout -> fail with submission id"
setup d; f=$FAIL; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_NOTARY=timeout
check "exit non-zero" [ "$RC" -ne 0 ]
check "submission id printed" out_has "submission sub-1"
check "notarytool info hint" out_has "xcrun notarytool info sub-1 --keychain-profile"
check "30m timeout and json requested" calls_has "--wait --timeout 30m --output-format json"
check "three locations unchanged" unchanged
check "temp dir cleaned" temp_clean
show_on_fail "$f"

echo "(d2) Gatekeeper rejects -> three locations unchanged"
setup d2; f=$FAIL; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_SPCTL=reject
check "exit non-zero" [ "$RC" -ne 0 ]
check "three locations unchanged" unchanged
check "temp dir cleaned" temp_clean
show_on_fail "$f"

echo "(e) both variables -> notarized, in order"
setup e; f=$FAIL; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1
check "exit 0" [ "$RC" -eq 0 ]
check "build skipped by hook" calls_lacks "npx"
check "APPLE_* unset for every tool" calls_lacks "APPLE_ID=leak"
check "calls in design order" in_order \
  "codesign --force --options runtime --timestamp --sign $ID $SB/repo/app/target/release/bundle/macos/D.Duck.app/Contents/MacOS/dduck" \
  "codesign --force --options runtime --timestamp --entitlements $SB/repo/app/src-tauri/entitlements.plist --sign $ID $SB/repo/app/target/release/bundle/macos/D.Duck.app |" \
  "codesign --verify --deep --strict" \
  "xcrun notarytool submit $SB/tmp/" \
  "xcrun stapler staple $SB/repo/app/target/release/bundle/macos/D.Duck.app" \
  "hdiutil create" "hdiutil verify" \
  "codesign --force --timestamp --sign $ID $SB/tmp/" \
  "xcrun notarytool submit $SB/tmp/" \
  "xcrun stapler staple $SB/tmp/" \
  "spctl -a -vv -t exec" "spctl -a -vv -t open --context context:primary-signature" \
  "xcrun stapler validate $SB/repo" "xcrun stapler validate $SB/tmp/" \
  "osascript" "open $SB/Applications/D.Duck.app"
check "app zip submitted first, then DMG" in_order "D.Duck.zip --keychain-profile dduck-notary" "D.Duck.dmg --keychain-profile dduck-notary"
check "no --deep when signing" calls_lacks "codesign --force --deep"
check "entitlements only on the app" [ "$(grep -c -- '--entitlements' "$CALLS")" -eq 1 ]
check "three locations replaced" replaced
check "temp dir cleaned" temp_clean
show_on_fail "$f"

echo
echo "passed $PASS, failed $FAIL (work dir $BASE)"
[ "$FAIL" -eq 0 ]
