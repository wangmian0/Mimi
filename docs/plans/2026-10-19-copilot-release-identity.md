# Mimi Copilot release identity

## Goal

Ship this fork as a downloadable macOS DMG that installs beside official Mimi
(owner: "有没有dmg", then "可以" to a separate app named Mimi Copilot).

## Shortest acceptance demo

- Mount the DMG.
- Check that `Mimi Copilot.app` reports bundle ID `io.github.wangmian0.mimi-copilot`, and that its signature verifies with the local `mimi Local Development` identity.
- Open the app. Confirm that the settings window appears without any Keychain prompt and that `/Applications/mimi.app` is untouched.
- Open the GitHub release page and confirm the DMG is offered for download.

## Scope and non-goals

- `productName` becomes `Mimi Copilot` and the release identifier becomes
  `io.github.wangmian0.mimi-copilot`. Without this, the package shares
  `app.yuxino.mimi`, so installing it replaces official Mimi and shares its
  settings.
- The fork identity uses its own credential service and never imports or
  retires upstream native credentials (`credential_scope` in
  `src-tauri/src/settings_store.rs`). A fresh data directory already has no
  pending imports. The scope makes that a rule instead of a coincidence.
- The packaging and verification scripts expect the fork identifier.
  `dev-app.sh` also treats a running Mimi Copilot as a conflicting instance.
- The app bundle ships ONNX Runtime's component notices, because a binary
  release statically links them through sherpa-onnx.
- Not done:
  - Apple notarization (needs a paid Developer ID).
  - Intel or Windows/Linux packages.
  - An updater feed.
  - Renaming the development build or its data directory.
  - Changing in-app copy that says "Mimi".
  - Rebasing onto upstream 1.6.4.

## Minimal path

1. Change the identity in `tauri.conf.json`. Set the credential scope and add its tests.
2. Update the script identifiers.
3. Run `./scripts/package-app.sh`. It signs the build with the local identity and produces the `.app` and `.dmg`.
4. Publish a GitHub release on `wangmian0/Mimi` with bilingual notes, using a tag that does not start with `v` so upstream CI does not run.

## Unknowns and verification

- **Gatekeeper on a downloaded copy.**
  - Observe: quarantine a copy of the DMG and run `spctl -a -t open --context context:primary-signature -vv`.
  - Expected: it is rejected as unnotarized, and the documented "仍要打开" path is required.
  - Overturned if: it is accepted, in which case the install steps are simplified.
