# First Beta Release Runbook

This runbook is the step-by-step operating guide for publishing the first beta release of `port-whisperer-rust` and the matching `ports-rs` npm package.

Use this document when releasing versions like:

- `v0.1.0-beta.1`
- `v0.1.0-beta.2`

The target outcome is:

- GitHub Release assets are published for all supported platforms
- npm package `ports-rs` is published to the `next` dist-tag
- installation and command smoke checks succeed afterward

---

## 1. Pre-release Sanity

Before tagging anything, confirm you are releasing from a clean `main` branch.

### 1.1 Confirm branch and working tree

```bash
git branch --show-current
git status --short
```

Expected:

- current branch is `main`
- working tree is clean

### 1.2 Pull latest main

```bash
git pull origin main
```

### 1.3 Run local verification

```bash
cargo test
npm pack
PORTS_RS_SKIP_DOWNLOAD=1 node scripts/postinstall.js
```

Expected:

- Rust tests pass
- npm tarball is generated successfully
- `postinstall` skip path succeeds

### 1.4 Confirm docs are current

Check these files before release:

- `README.md`
- `README.zh-CN.md`
- `docs/release-checklist.md`

Make sure:

- install commands still match reality
- release asset names still match the workflow and package installer
- the current npm package name is still `ports-rs`

---

## 2. Choose the Version

Decide the exact beta version before tagging.

Examples:

- first beta: `v0.1.0-beta.1`
- follow-up beta: `v0.1.0-beta.2`

Version alignment rules:

- Git tag: `v0.1.0-beta.1`
- npm package version: `0.1.0-beta.1`

If you are changing the beta number, update `package.json` first.

Example:

```json
{
  "version": "0.1.0-beta.1"
}
```

If you modify `package.json`, commit that change before creating the tag.

---

## 3. Create and Push the Tag

### 3.1 Create the tag locally

```bash
git tag v0.1.0-beta.1
```

### 3.2 Push the tag

```bash
git push origin v0.1.0-beta.1
```

This triggers:

- `.github/workflows/release.yml`

---

## 4. Verify the GitHub Actions Release Workflow

Open the repository Actions tab and watch the release workflow.

The workflow should:

- build release binaries
- package assets for four targets
- upload assets to the GitHub Release for the pushed tag

### 4.1 Expected assets

The Release page should contain exactly these files:

- `ports-rs-darwin-arm64.tar.gz`
- `ports-rs-darwin-x64.tar.gz`
- `ports-rs-linux-x64.tar.gz`
- `ports-rs-windows-x64.zip`

### 4.2 Quick manual verification

Download at least one Unix package and check contents:

```bash
tar -tzf ports-rs-darwin-arm64.tar.gz
```

Expected:

- `ports`
- `whoisonport`

For Windows, unzip and check:

- `ports.exe`
- `whoisonport.exe`

If any asset is missing, incorrectly named, or contains the wrong files, stop and fix the workflow before touching npm.

---

## 5. Approve npm Publish in GitHub Actions

The workflow does not publish to npm until a maintainer approves the `npm-publish` environment.

### 5.1 Open the release workflow run

- open the Actions run triggered by the tag
- confirm `verify-release` passed
- confirm `publish-release` passed

### 5.2 Approve the environment

- open the pending `publish-npm` job
- review the target dist-tag
- approve the `npm-publish` environment

### 5.3 Verify publish results

```bash
npm view ports-rs version dist-tags
```

Expected:

- version matches the intended beta
- `next` points to that beta release

---

## 6. Post-release Smoke Checks

After publish, validate the user-facing install path.

### 6.1 Install from npm

```bash
npm i -g ports-rs@next
```

### 6.2 Verify command availability

```bash
ports --help
whoisonport --help
```

### 6.3 Verify runtime behavior

At minimum, run:

```bash
ports
ports ps
ports --all
```

If you have a local dev server running, also test:

```bash
ports 3000
whoisonport 3000
```

### 6.4 Optional stronger smoke test

Start a temporary local server and test the full install path:

```bash
node -e "require('http').createServer((_,res)=>res.end('ok')).listen(3000,'127.0.0.1'); setInterval(()=>{},1000)"
```

Then in another terminal:

```bash
ports 3000
whoisonport 3000
ports kill 3000
```

---

## 7. If Something Goes Wrong

### GitHub Release assets failed

- inspect the failed Actions job
- fix the workflow or packaging script
- delete the bad tag if needed

### npm publish failed after approval

- check the `publish-npm` job logs
- verify the environment secret `NPM_TOKEN` is still valid
- run `npm view ports-rs version dist-tags` to confirm whether npm published anything
- if the publish did not succeed, fix the cause and release a new version instead of trying to overwrite the same package version
- re-tag and re-push

### npm publish failed

- confirm `package.json` version is correct
- confirm you are logged in to the correct npm account
- confirm the package name `ports-rs` is still available to your account

### npm install succeeds but binary is missing

- inspect `scripts/postinstall.js`
- confirm the GitHub Release asset names still match the installer mapping
- confirm the Release actually contains the expected files

---

## 8. Done Criteria for First Beta

The first beta release is complete when all of the following are true:

- GitHub Release exists for the chosen tag
- all four expected release assets are attached
- `ports-rs@next` is published to npm
- `ports` and `whoisonport` install successfully through npm
- basic smoke checks succeed after install
