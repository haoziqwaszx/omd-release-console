# Real Publish Execution Design

## Goal

OMD Release Console should execute the publish portion of the release inside the Rust tool after explicit version confirmation. Publishing must remain deterministic and whitelist-only: GitHub and Gitee are fixed actions, not arbitrary shell commands.

## Scope

This pass enables:

- `publish-github`: create a GitHub Release for `v{version}` with `gh release create`, uploading `github/latest.json` plus files under `windows/` and `mac/`.
- `publish-gitee`: create a Gitee Release through the Gitee v5 API, upload `gitee/latest.json` plus files under `windows/` and `mac/`, then update the repository root `latest.json` used by the updater endpoint.
- UI progression through `publish-github -> publish-gitee -> verify -> report`.

Out of scope:

- Automatic retry or overwrite for an existing release tag.
- AI-driven command execution.
- Arbitrary command input from the UI/API.

## Safety

Both publish actions require `confirmVersion == version`. Asset lists are built from fixed release directory subfolders and passed as structured process arguments or API payloads. Gitee tokens are read from git credential storage and sent through stdin-backed curl payloads/config, so state files and logs do not store the token.

## Verification

Unit tests cover enabled publish steps, fixed asset path discovery, GitHub argument construction without shell globs, and Gitee release payload shape. The full repository check remains `npm run check`.
