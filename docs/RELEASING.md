# Releasing D.Duck

D.Duck ships as a DMG signed with a Developer ID certificate and notarized by Apple, so Gatekeeper does not block it
as coming from an "unidentified developer". It is not on the Mac App Store: the App Store sandbox would stop the app
from reading `~/.claude`, running your CLI, using git and opening Terminal.

Without the setup below, `app/scripts/release-local.sh` still works and makes an ad-hoc signed, **not notarized**
build. That is fine for your own Mac; on another Mac the first open needs right-click > Open.

## One-time setup

1. **Join the Apple Developer Program** (USD 99 per year): <https://developer.apple.com/programs/enroll/>
2. **Create a Developer ID Application certificate.** In Xcode: Settings > Accounts > select your team >
   Manage Certificates > `+` > **Developer ID Application**. The certificate and its private key go into your login
   keychain. Check it is there:

   ```sh
   security find-identity -v -p codesigning
   # 1) ABCDEF... "Developer ID Application: Your Name (TEAMID)"
   ```

3. **Store notarization credentials in the keychain.** Create an app-specific password at
   <https://account.apple.com> (Sign-In and Security > App-Specific Passwords), then run:

   ```sh
   xcrun notarytool store-credentials dduck-notary --apple-id <apple-id> --team-id <team-id>
   ```

   Leave out `--password`: notarytool asks for the app-specific password interactively, so it never lands in your
   shell history. The release script only refers to the profile name `dduck-notary`.

Never put your Apple ID, app-specific password, team ID or certificate in this repository, in scripts or in CI logs.

## Make a notarized release

```sh
DDUCK_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
DDUCK_NOTARY_PROFILE=dduck-notary \
app/scripts/release-local.sh
```

The script builds the app, signs it inside out with the hardened runtime and `app/src-tauri/entitlements.plist`
(Apple Events only), notarizes and staples the app, builds the DMG, signs, notarizes and staples the DMG, and
verifies both. Each notarization waits up to 30 minutes. Only after every check passes does it replace
`app/target/release/bundle/dmg/`, `publish/dmg/D.Duck-latest_<arch>.dmg` and `/Applications/D.Duck.app`. On any
failure those stay as they were.

- Both variables set: notarized build. Neither set: ad-hoc build. Only one set: the script stops and says which is
  missing.
- Rejected: the script prints `notarytool log <id>`. Fix the reported issue and run again.
- Timed out: the script prints the submission id. Check later with
  `xcrun notarytool info <id> --keychain-profile dduck-notary`.

## Verify

```sh
APP=/Applications/D.Duck.app
DMG=publish/dmg/D.Duck-latest_aarch64.dmg
codesign --verify --deep --strict --verbose=2 "$APP"
spctl -a -vv -t exec "$APP"                                     # source=Notarized Developer ID
spctl -a -vv -t open --context context:primary-signature "$DMG" # source=Notarized Developer ID
xcrun stapler validate "$APP"
xcrun stapler validate "$DMG"
```

### First open as a downloader would

Test on a second macOS user account (System Settings > Users & Groups), so nothing from your own account is
trusted already:

1. Copy the DMG to that account (for example via `/Users/Shared`) and mark it as downloaded:
   `xattr -w com.apple.quarantine "0081;$(printf %x "$(date +%s)");Safari;" D.Duck-latest_aarch64.dmg`
2. Turn off Wi-Fi and unplug the network, so the stapled tickets are what Gatekeeper checks.
3. Open the DMG, drag D.Duck to Applications, open it.

Expected: only the "downloaded from the Internet" confirmation, no "unidentified developer" block. The first time a
task is opened in Terminal, macOS asks for Automation permission; that is normal. Also check that the CLI runs and
that notifications and the tray icon work.

## Publish

1. Note the SHA-256 the script prints.
2. Create a GitHub Release for the version tag and upload `publish/dmg/D.Duck-latest_aarch64.dmg`, renamed to
   `D.Duck_<version>_aarch64.dmg`. Put the SHA-256 in the release notes.

   ```sh
   cp publish/dmg/D.Duck-latest_aarch64.dmg "D.Duck_<version>_aarch64.dmg"
   gh release create v<version> "D.Duck_<version>_aarch64.dmg" --title "D.Duck <version>" --notes "SHA-256: <hash>"
   ```
