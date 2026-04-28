# Homebrew Support

The public tap repository is expected to live at:

```sh
https://github.com/EasyXdc/homebrew-tap
```

Users install it as:

```sh
brew install EasyXdc/tap/ports-rs
```

This project publishes macOS release archives consumed by that tap:

- `ports-rs-darwin-arm64.tar.gz`
- `ports-rs-darwin-x64.tar.gz`

Release automation generates two helper assets:

- `ports-rs.rb` — a Homebrew formula for the current release
- `SHA256SUMS` — checksums for all release archives

## User Install Command

```sh
brew install EasyXdc/tap/ports-rs
```

This installs:

```sh
ports
whoisonport
```

## Tap Maintenance Flow

For each tagged release:

1. Wait for the GitHub Release workflow to complete.
2. Download the generated `ports-rs.rb` asset from the release.
3. Copy it into the tap repository as `Formula/ports-rs.rb`.
4. Commit and push the tap repository.
5. Verify locally:

```sh
brew tap EasyXdc/tap
brew update
brew reinstall ports-rs
brew test ports-rs
```

The formula intentionally targets macOS only. Linux users should use npm, Cargo, or the prebuilt GitHub Release archive.

## Formula Generation

The formula can also be generated locally after packaging release assets:

```sh
cargo build --release
node scripts/package-release.js darwin-arm64
node scripts/package-release.js darwin-x64
node scripts/homebrew-formula.js
```

The default output path is:

```sh
dist/ports-rs.rb
```

Use `DIST_DIR` and `HOMEBREW_FORMULA_OUT` to override input and output paths.
