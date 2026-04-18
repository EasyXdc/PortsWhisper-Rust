# Release Checklist

This checklist is for publishing `port-whisperer-rust` release assets and then following up with the `ports-rs` npm package.

For the full first beta release procedure, see:

- `docs/first-beta-release-runbook.md`

## 1. Preflight

- Run `cargo test`
- Run `npm pack`
- Run `PORTS_RS_SKIP_DOWNLOAD=1 node scripts/postinstall.js`
- Confirm `README.md` and `README.zh-CN.md` are current
- Confirm `scripts/package-release.js` still produces the expected local asset

## 2. Tag & Release

- Create a release tag such as `v0.1.0-beta.3` or `v0.1.0`
- Push the tag to GitHub
- Wait for `.github/workflows/release.yml` to finish `verify-release` and `publish-release`
- Confirm the GitHub Release contains exactly these assets:
  - `ports-rs-darwin-arm64.tar.gz`
  - `ports-rs-darwin-x64.tar.gz`
  - `ports-rs-linux-x64.tar.gz`
  - `ports-rs-windows-x64.zip`

## 3. Smoke Checks

- Download at least one Unix asset and verify it extracts to:
  - `ports`
  - `whoisonport`
- Download the Windows asset and verify it extracts to:
  - `ports.exe`
  - `whoisonport.exe`
- Run at least one extracted binary locally and confirm it starts

## 4. Approval & npm Publish

- Open the Actions run for the release tag
- Review the resolved npm dist-tag (`next` for beta, `latest` for stable)
- Approve the `npm-publish` environment when the release assets look correct
- Wait for `publish-npm` to finish
- Verify npm metadata:
  - `npm view ports-rs version dist-tags`

## 5. Final Sanity Check

- Confirm the README install commands still match the published release assets
- Confirm npm package version and GitHub tag version align
- Confirm no local-only files or generated artifacts are staged before the next commit
