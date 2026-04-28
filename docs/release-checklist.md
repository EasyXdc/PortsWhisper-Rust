# Release Checklist

This checklist is for publishing the stable `port-whisperer-rust` release assets and the matching `ports-rs` npm package.

## 1. Preflight

- Run `cargo fmt --check`
- Run `cargo clippy -- -D warnings`
- Run `cargo test`
- Run `npm test`
- Run `npm pack`
- Run `PORTS_RS_SKIP_DOWNLOAD=1 node scripts/postinstall.js`
- Confirm `README.md` and `README.zh-CN.md` are current
- Confirm `scripts/package-release.js` still produces the expected local asset and cleans up its staging directory

## 2. Tag & Release

- Create a release tag such as `v0.2.4` or `v0.2.5`
- Push the tag to GitHub
- Wait for `.github/workflows/release.yml` to finish `verify-release`, `publish-release`, `verify-install`, and `publish-npm`
- Confirm the GitHub Release contains exactly these assets:
  - `ports-rs-darwin-arm64.tar.gz`
  - `ports-rs-darwin-x64.tar.gz`
  - `ports-rs-linux-x64.tar.gz`
  - `ports-rs-windows-x64.zip`
  - `ports-rs.rb`
  - `SHA256SUMS`

## 3. Smoke Checks

- Download at least one Unix asset and verify it extracts to:
  - `ports`
  - `whoisonport`
- Download the Windows asset and verify it extracts to:
  - `ports.exe`
  - `whoisonport.exe`
- Run at least one extracted binary locally and confirm it starts
- Confirm `ports-rs.rb` contains the current version and both macOS archive URLs
- Confirm `SHA256SUMS` contains all four binary archives; npm `postinstall` uses this file to verify downloaded archive checksums

## 4. Homebrew Tap Follow-up

- Download `ports-rs.rb` from the GitHub Release
- Copy it to the tap repository as `Formula/ports-rs.rb`
- Commit and push the tap repository
- Verify:
  - `brew update`
  - `brew reinstall EasyXdc/tap/ports-rs`
  - `brew test EasyXdc/tap/ports-rs`

## 5. npm Publish Verification

- Open the Actions run for the release tag
- Review the resolved npm dist-tag (`latest` for stable, `next` only for prerelease builds)
- Approve the `npm-publish` environment when the release assets and install verification look correct
- Wait for `publish-npm` to finish
- Verify npm metadata:
  - `npm view ports-rs version dist-tags`

## 6. Final Sanity Check

- Confirm the README install commands still match the published release assets
- Confirm npm package version and GitHub tag version align
- Confirm no local-only files, generated archives, `vendor/`, `dist/`, `target/`, logs, or `docs/superpowers/` files are staged before the next commit
