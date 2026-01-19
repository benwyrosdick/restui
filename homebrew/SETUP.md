# Homebrew Tap Setup Guide

This guide explains how to set up the Homebrew tap for restui.

## 1. Create the Homebrew Tap Repository

Create a new GitHub repository named `homebrew-tap` at:
https://github.com/benwyrosdick/homebrew-tap

## 2. Set Up the Repository Structure

```
homebrew-tap/
├── Formula/
│   └── restui.rb          # Copy from this directory
└── .github/
    └── workflows/
        └── update-formula.yml  # Copy from this directory
```

## 3. Create a Personal Access Token

For automatic formula updates on release:

1. Go to https://github.com/settings/tokens
2. Click "Generate new token (classic)"
3. Give it a name like "homebrew-tap-update"
4. Select scopes: `repo` (full control of private repositories)
5. Generate and copy the token

## 4. Add the Token as a Secret

In the **restui** repository (not the tap):

1. Go to Settings > Secrets and variables > Actions
2. Click "New repository secret"
3. Name: `HOMEBREW_TAP_TOKEN`
4. Value: paste the token from step 3

## 5. Initial Release

Create and push a version tag to trigger the first release:

```bash
git tag v0.1.2
git push origin v0.1.2
```

This will:
1. Build binaries for macOS (Intel + Apple Silicon) and Linux
2. Create a GitHub release with the binaries
3. Trigger the homebrew-tap to update the formula

## 6. Manual Formula Update (if needed)

If automatic updates fail, manually update the formula:

```bash
# Download the release and get SHA256
curl -L https://github.com/benwyrosdick/restui/releases/download/v0.1.2/restui-aarch64-apple-darwin.tar.gz -o arm.tar.gz
shasum -a 256 arm.tar.gz

curl -L https://github.com/benwyrosdick/restui/releases/download/v0.1.2/restui-x86_64-apple-darwin.tar.gz -o x86.tar.gz
shasum -a 256 x86.tar.gz

curl -L https://github.com/benwyrosdick/restui/releases/download/v0.1.2/restui-x86_64-unknown-linux-gnu.tar.gz -o linux.tar.gz
shasum -a 256 linux.tar.gz
```

Then update the SHA256 values in `Formula/restui.rb`.

## 7. Users Can Install With

```bash
brew tap benwyrosdick/tap
brew install restui
```

Or in one command:
```bash
brew install benwyrosdick/tap/restui
```

## Troubleshooting

### Build fails on Apple Silicon
Make sure the workflow uses `macos-latest` which now runs on ARM.

### Formula test fails
Ensure restui supports `--version` flag and outputs the version number.

### Automatic update doesn't trigger
Check that:
- `HOMEBREW_TAP_TOKEN` secret is set correctly
- Token has `repo` scope
- The tap repository exists at the correct path
