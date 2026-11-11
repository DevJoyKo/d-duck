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
  for t in codesign dwarfdump osascript open sleep npx spctl hdiutil xcrun ditto; do
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
  # codesign: FAKE_CODESIGN=fail-new fails the check of the copy staged in the install location.
  cat >>"$bin/codesign" <<'EOF'
for a in "$@"; do last="$a"; done
if [ "${FAKE_CODESIGN:-ok}" = fail-new ] && [ "$1" = --verify ] && [[ "$last" == *.D.Duck.app.new ]]; then exit 1; fi
exit 0
EOF
  # ditto: the real one, except FAKE_DITTO=fail-install leaves half a copy in the install location and fails.
  cat >>"$bin/ditto" <<'EOF'
for a in "$@"; do last="$a"; done
if [ "${FAKE_DITTO:-ok}" = fail-install ] && [[ "$last" == *.D.Duck.app.new ]]; then
  mkdir -p "$last/Contents" && echo half >"$last/Contents/partial"; echo "ditto: No space left on device" >&2; exit 1
fi
exec /usr/bin/ditto "$@"
EOF
  cat >>"$bin/hdiutil" <<'EOF'
if [ "$1" = create ]; then
  for a in "$@"; do case "$a" in *.dmg) echo "NEW-DMG" >"$a" ;; esac; done
fi
exit 0
EOF
  # spctl: FAKE_SPCTL=reject fails; devid accepts both as plain Developer ID (not notarized); devid-dmg only the DMG.
  cat >>"$bin/spctl" <<'EOF'
for a in "$@"; do last="$a"; done
if [ "${FAKE_SPCTL:-accept}" = reject ]; then echo "$last: rejected" >&2; exit 3; fi
echo "$last: accepted" >&2
case "${FAKE_SPCTL:-accept}:$last" in
  devid:* | devid-dmg:*.dmg) echo "source=Developer ID" >&2 ;;
  *) echo "source=Notarized Developer ID" >&2 ;;
esac
exit 0
EOF
  # notarytool: FAKE_NOTARY is a comma list, one outcome per submission: accepted | invalid | timeout | <other> (401,
  # no JSON). submit never waits; wait and info answer from the outcome stored under the submission id.
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
          accepted | invalid | timeout)
            echo "$outcome" >"$NOTARY_COUNT.sub-$n"
            echo "{\"id\":\"sub-$n\",\"path\":\"x\",\"message\":\"Successfully uploaded file\"}"; exit 0 ;;
          garbage) echo "Uploading... done"; exit 0 ;;
          *) echo "Error: HTTP status code: 401. Unable to authenticate." >&2; exit 1 ;;
        esac ;;
      wait)
        case "$(cat "$NOTARY_COUNT.$2")" in
          accepted) echo "{\"id\":\"$2\",\"status\":\"Accepted\"}" ;;
          invalid)  echo "{\"id\":\"$2\",\"status\":\"Invalid\"}" ;;
          *)        echo "Error: Timeout of 30m reached." >&2; exit 1 ;;
        esac ;;
      info)
        case "$(cat "$NOTARY_COUNT.$2")" in
          accepted) echo "{\"id\":\"$2\",\"status\":\"Accepted\"}" ;;
          invalid)  echo "{\"id\":\"$2\",\"status\":\"Invalid\"}" ;;
          *)        echo "{\"id\":\"$2\",\"status\":\"In Progress\"}" ;;
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
# No half-made copies left in the three locations.
no_leftovers() {
  [ ! -e "$SB/Applications/.D.Duck.app.new" ] && ! compgen -G "$SB/repo/app/target/release/bundle/dmg/*.partial" >/dev/null \
    && ! compgen -G "$SB/repo/publish/dmg/*.partial" >/dev/null
}
out_has() { grep -qF -- "$1" "$OUT"; }
out_lacks() { ! grep -qF -- "$1" "$OUT"; }
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
check "notice points to Open Anyway" out_has "System Settings > Privacy & Security > Open Anyway"
check "notice drops right-click > Open (no bypass on Sequoia)" out_lacks "right-click"
check "three locations replaced, old app in Trash" replaced
check "relaunched" calls_has "open $SB/Applications/D.Duck.app"
check "no .new or .partial left" no_leftovers
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

echo "(d) notarization wait timeout -> fail, id printed with info/log/history hint"
setup d; f=$FAIL; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_NOTARY=timeout
check "exit non-zero" [ "$RC" -ne 0 ]
check "submit does not wait" calls_lacks "--wait"
check "submit asks for json" calls_has "D.Duck.zip --keychain-profile dduck-notary --output-format json"
check "submission id printed before waiting" in_order "notarytool submit" "notarytool wait sub-1"
check "id printed" out_has "notarize: submission sub-1"
check "wait has the 30m timeout" calls_has "xcrun notarytool wait sub-1 --timeout 30m --keychain-profile dduck-notary"
check "status read with info" calls_has "xcrun notarytool info sub-1 --keychain-profile dduck-notary"
check "notarytool info hint" out_has "xcrun notarytool info sub-1 --keychain-profile"
check "notarytool log hint" out_has "xcrun notarytool log sub-1 --keychain-profile"
check "notarytool history hint" out_has "xcrun notarytool history --keychain-profile"
check "nothing stapled" calls_lacks "stapler staple"
check "three locations unchanged" unchanged
check "temp dir cleaned" temp_clean
setup d1; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_NOTARY=accepted,timeout
check "DMG wait timeout: exit non-zero" [ "$RC" -ne 0 ]
check "DMG id printed with hint" out_has "xcrun notarytool log sub-2 --keychain-profile"
check "three locations unchanged" unchanged
show_on_fail "$f"

echo "(d1) notarytool submit 401 / no JSON -> clear error, no id"
setup d1b; f=$FAIL; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_NOTARY=auth401
check "exit non-zero" [ "$RC" -ne 0 ]
check "says no submission id" out_has "no submission id"
check "notarytool's own error shown" out_has "HTTP status code: 401"
check "no made-up id" out_lacks "submission sub-"
check "no wait without an id" calls_lacks "notarytool wait"
check "three locations unchanged" unchanged
check "temp dir cleaned" temp_clean
setup d1c; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_NOTARY=garbage
check "non-JSON submit, exit 0: exit non-zero" [ "$RC" -ne 0 ]
check "non-JSON submit: no id (plutil error not read as id)" out_has "no submission id"
check "non-JSON submit: no wait" calls_lacks "notarytool wait"
check "non-JSON submit: three locations unchanged" unchanged
show_on_fail "$f"

echo "(d3) stapling fails -> three locations unchanged"
setup d3; f=$FAIL; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_STAPLER=fail
check "exit non-zero" [ "$RC" -ne 0 ]
check "says stapling failed" out_has "stapling the ticket to the app failed"
check "no DMG made after the app staple failed" calls_lacks "hdiutil create"
check "three locations unchanged" unchanged
check "temp dir cleaned" temp_clean
show_on_fail "$f"

echo "(d4) Gatekeeper says plain Developer ID (not notarized) -> refused"
setup d4; f=$FAIL; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_SPCTL=devid
check "app: exit non-zero" [ "$RC" -ne 0 ]
check "app: says not notarized" out_has "app is not reported as Notarized Developer ID"
check "app: three locations unchanged" unchanged
setup d4b; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_SPCTL=devid-dmg
check "DMG: exit non-zero" [ "$RC" -ne 0 ]
check "DMG: says not notarized" out_has "DMG is not reported as Notarized Developer ID"
check "DMG: three locations unchanged" unchanged
check "temp dir cleaned" temp_clean
show_on_fail "$f"

echo "(d2) Gatekeeper rejects -> three locations unchanged"
setup d2; f=$FAIL; run DDUCK_SIGN_IDENTITY="$ID" DDUCK_NOTARY_PROFILE=dduck-notary DDUCK_SKIP_BUILD=1 FAKE_SPCTL=reject
check "exit non-zero" [ "$RC" -ne 0 ]
check "three locations unchanged" unchanged
check "temp dir cleaned" temp_clean
show_on_fail "$f"

echo "(d5) install copy fails -> old app kept, nothing replaced, no leftovers"
setup d5; f=$FAIL; run FAKE_DITTO=fail-install
check "copy failure: exit non-zero" [ "$RC" -ne 0 ]
check "copy failure: says the installed app was kept" out_has "were left as they were"
check "copy failure: three locations unchanged" unchanged
check "copy failure: no .new or .partial left" no_leftovers
check "copy failure: app not quit" calls_lacks "osascript"
setup d5b; run FAKE_CODESIGN=fail-new
check "copy check failure: exit non-zero" [ "$RC" -ne 0 ]
check "copy check failure: three locations unchanged" unchanged
check "copy check failure: no .new or .partial left" no_leftovers
check "temp dir cleaned" temp_clean
setup d5c; chmod 555 "$SB/home/.Trash"; run
chmod 755 "$SB/home/.Trash"
check "old app cannot go to Trash: exit non-zero" [ "$RC" -ne 0 ]
check "old app cannot go to Trash: installed app kept" [ "$(cat "$SB/Applications/D.Duck.app/marker")" = OLD-APP ]
check "old app cannot go to Trash: says DMGs already updated" out_has "the DMGs are already updated"
check "old app cannot go to Trash: no .new or .partial left" no_leftovers
check "old app cannot go to Trash: not relaunched" calls_lacks "open $SB/Applications/D.Duck.app"
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
check "no .new or .partial left" no_leftovers
check "both submissions waited on" in_order "notarytool wait sub-1" "notarytool wait sub-2"
check "temp dir cleaned" temp_clean
show_on_fail "$f"

echo
echo "passed $PASS, failed $FAIL (work dir $BASE)"
[ "$FAIL" -eq 0 ]
