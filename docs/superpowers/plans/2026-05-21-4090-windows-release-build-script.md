# 4090 Windows Release Build Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace ad-hoc Mac-generated PowerShell with a reusable 4090 Windows release build script that is faster, version-agnostic, signs the green zip reliably, exports artifacts as a zip, and can be driven by OMD Release Console.

**Architecture:** Store the Windows build logic as a versioned PowerShell script in this repo and upload it to the 4090 before each build. Rust keeps the release state machine, CLI, HTTP, and Tauri API, but delegates Windows-side build details to the script and collects a single exported `windows-artifacts.zip`. The script uses a fixed cached build directory, validates the requested version, creates all Windows artifacts, signs the green zip without `npx`, and writes `build-summary.json`.

**Tech Stack:** Rust 2021, Tauri v2, static JS frontend, PowerShell 5+, Windows OpenSSH, npm/Tauri CLI, Cargo/Rust, optional sccache.

---

## File Structure

- Create: `scripts/windows/omd-release-build.ps1` — long-lived Windows-side release builder, parameterized by version/branch/commit/build/export paths.
- Modify: `src/main.rs` — upload and invoke the script, collect `windows-artifacts.zip`, parse `build-summary.json`, adjust tests around Windows build/collect behavior.
- Modify: `release.config.json` — add optional script/build/export path settings for the Windows builder.
- Modify: `README.md` — document the 4090 script workflow, cached build mode, and clean build option.
- No frontend changes required unless existing UI labels become misleading after backend state changes.

---

### Task 1: Add the reusable 4090 PowerShell build script

**Files:**
- Create: `scripts/windows/omd-release-build.ps1`
- Test: run remotely through `ssh 4090@192.168.101.9 powershell ... -File ... -Version 0.0.7 -SkipBuild` after upload in later task

- [ ] **Step 1: Create the script directory**

Run:

```bash
mkdir -p scripts/windows
```

Expected: directory exists.

- [ ] **Step 2: Write the initial script**

Create `scripts/windows/omd-release-build.ps1` with this complete content:

```powershell
param(
  [Parameter(Mandatory=$true)]
  [ValidatePattern('^\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$')]
  [string]$Version,

  [string]$Repo = 'C:\Users\4090\Desktop\dix-extension-ui',
  [string]$BuildDir = 'C:\Users\4090\Desktop\dix-extension-ui-release-build',
  [string]$Branch = 'codex/tauri-rust-migration',
  [string]$Commit = '',
  [string]$ExportDir = '',
  [switch]$Clean,
  [switch]$SkipLint,
  [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

function Fail([string]$Message) {
  Write-Output "ERROR: $Message"
  exit 1
}

function Run-Step([string]$Name, [scriptblock]$Body) {
  Write-Output "OMD_STEP_START=$Name"
  & $Body
  if ($LASTEXITCODE -ne $null -and $LASTEXITCODE -ne 0) {
    Fail "$Name failed with exit code $LASTEXITCODE"
  }
  Write-Output "OMD_STEP_DONE=$Name"
}

function Ensure-Directory([string]$Path) {
  if (!(Test-Path -LiteralPath $Path)) {
    New-Item -ItemType Directory -Force -Path $Path | Out-Null
  }
}

function File-Sha256([string]$Path) {
  if (!(Test-Path -LiteralPath $Path)) { return '' }
  return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

function Copy-Required([string]$Source, [string]$Destination) {
  if (!(Test-Path -LiteralPath $Source)) {
    Fail "Missing required file: $Source"
  }
  Copy-Item -Force -LiteralPath $Source -Destination $Destination
}

function Run-Tauri-Signer([string]$BuildDir, [string]$InputFile, [string]$SignatureFile) {
  $tauri = Join-Path $BuildDir 'node_modules\.bin\tauri.cmd'
  $key = Join-Path $BuildDir 'src-tauri\updater.key'
  if (!(Test-Path -LiteralPath $tauri)) { Fail "Missing Tauri CLI: $tauri" }
  if (!(Test-Path -LiteralPath $key)) { Fail "Missing updater key: $key" }
  if (!(Test-Path -LiteralPath $InputFile)) { Fail "Missing file to sign: $InputFile" }

  $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $key
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''
  $job = Start-Job -ScriptBlock {
    param($TauriPath, $FilePath, $WorkingDirectory)
    Set-Location -LiteralPath $WorkingDirectory
    & $TauriPath signer sign $FilePath | Out-String
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } -ArgumentList $tauri, $InputFile, $BuildDir

  $completed = Wait-Job $job -Timeout 120
  if ($null -eq $completed) {
    Stop-Job $job -ErrorAction SilentlyContinue
    Remove-Job $job -Force -ErrorAction SilentlyContinue
    Fail 'green zip signer timed out after 120 seconds'
  }

  $signature = (Receive-Job $job | Out-String).Trim()
  $exitCode = $job.ChildJobs[0].JobStateInfo.State
  Remove-Job $job -Force -ErrorAction SilentlyContinue
  if ([string]::IsNullOrWhiteSpace($signature)) {
    Fail 'green zip signer returned an empty signature'
  }
  Set-Content -Encoding utf8 -NoNewline -LiteralPath $SignatureFile -Value $signature
}

function Resolve-ExportDir([string]$Version, [string]$ExportDir) {
  if (![string]::IsNullOrWhiteSpace($ExportDir)) { return $ExportDir }
  return "C:\Users\4090\AppData\Local\Temp\omd-release-export-$Version"
}

$ExportDir = Resolve-ExportDir $Version $ExportDir
$CacheDir = Join-Path $BuildDir '.omd-release-cache'
$WindowsTarget = 'x86_64-pc-windows-msvc'

Write-Output "OMD_BUILD_VERSION=$Version"
Write-Output "OMD_SOURCE_REPO=$Repo"
Write-Output "OMD_BUILD_DIR=$BuildDir"
Write-Output "OMD_EXPORT_DIR=$ExportDir"

if (!(Test-Path -LiteralPath $Repo)) {
  Fail "Source repo does not exist: $Repo"
}

if ($Clean -and (Test-Path -LiteralPath $BuildDir)) {
  Remove-Item -Recurse -Force -LiteralPath $BuildDir
}

if (!(Test-Path -LiteralPath $BuildDir)) {
  Run-Step 'clone-build-dir' {
    git clone $Repo $BuildDir
  }
}

Run-Step 'sync-build-dir' {
  git -C $BuildDir fetch --all --prune
  if (![string]::IsNullOrWhiteSpace($Commit)) {
    git -C $BuildDir reset --hard $Commit
  } else {
    git -C $BuildDir checkout $Branch
    git -C $BuildDir pull --ff-only
  }
  git -C $BuildDir clean -fdx -e node_modules -e src-tauri/target -e .omd-release-cache
}

Run-Step 'sync-release-metadata' {
  $releaseFiles = @('package.json', 'package-lock.json', 'src-tauri\Cargo.toml', 'src-tauri\Cargo.lock', 'src-tauri\tauri.conf.json')
  foreach ($releaseFile in $releaseFiles) {
    Copy-Required (Join-Path $Repo $releaseFile) (Join-Path $BuildDir $releaseFile)
  }
  $sourceKey = Join-Path $Repo 'src-tauri\updater.key'
  Copy-Required $sourceKey (Join-Path $BuildDir 'src-tauri\updater.key')
}

Set-Location -LiteralPath $BuildDir
$packageVersion = (Get-Content -Raw -LiteralPath 'package.json' | ConvertFrom-Json).version
$tauriVersion = (Get-Content -Raw -LiteralPath 'src-tauri\tauri.conf.json' | ConvertFrom-Json).version
if ($packageVersion -ne $Version) { Fail "package.json version $packageVersion does not match target $Version" }
if ($tauriVersion -ne $Version) { Fail "tauri.conf.json version $tauriVersion does not match target $Version" }

Ensure-Directory $CacheDir
$lockHashFile = Join-Path $CacheDir 'package-lock.sha256'
$currentLockHash = File-Sha256 (Join-Path $BuildDir 'package-lock.json')
$previousLockHash = ''
if (Test-Path -LiteralPath $lockHashFile) {
  $previousLockHash = (Get-Content -Raw -LiteralPath $lockHashFile).Trim()
}
if (!(Test-Path -LiteralPath (Join-Path $BuildDir 'node_modules')) -or $currentLockHash -ne $previousLockHash) {
  Run-Step 'npm-ci' {
    npm ci
  }
  Set-Content -Encoding utf8 -NoNewline -LiteralPath $lockHashFile -Value $currentLockHash
} else {
  Write-Output 'OMD_STEP_SKIPPED=npm-ci'
}

if (Get-Command sccache -ErrorAction SilentlyContinue) {
  $env:RUSTC_WRAPPER = 'sccache'
  Write-Output 'OMD_SCCACHE=enabled'
} else {
  Write-Output 'OMD_SCCACHE=missing'
}

if (!$SkipLint) {
  Run-Step 'npm-lint' {
    npm run lint
  }
}

if (!$SkipBuild) {
  Run-Step 'tauri-bundles' {
    npm run tauri:build -- --bundles nsis,msi --target $WindowsTarget
  }

  Run-Step 'tauri-no-bundle' {
    npm run tauri:build -- --no-bundle --target $WindowsTarget
  }
}

Run-Step 'green-artifacts' {
  $greenDir = Join-Path $BuildDir "src-tauri\target\$WindowsTarget\release\bundle\green"
  Ensure-Directory $greenDir
  $greenExe = Join-Path $greenDir "OMD_${Version}_x64_green.exe"
  $greenZip = Join-Path $greenDir "OMD_${Version}_x64_green.zip"
  $greenSig = "$greenZip.sig"
  Copy-Required (Join-Path $BuildDir "src-tauri\target\$WindowsTarget\release\omd.exe") $greenExe
  if (Test-Path -LiteralPath $greenZip) { Remove-Item -Force -LiteralPath $greenZip }
  Compress-Archive -LiteralPath $greenExe -DestinationPath $greenZip -Force
  Run-Tauri-Signer $BuildDir $greenZip $greenSig
}

Run-Step 'export-artifacts' {
  if (Test-Path -LiteralPath $ExportDir) { Remove-Item -Recurse -Force -LiteralPath $ExportDir }
  Ensure-Directory $ExportDir
  $artifacts = @(
    "src-tauri\target\$WindowsTarget\release\bundle\nsis\OMD_${Version}_x64-setup.exe",
    "src-tauri\target\$WindowsTarget\release\bundle\nsis\OMD_${Version}_x64-setup.exe.sig",
    "src-tauri\target\$WindowsTarget\release\bundle\msi\OMD_${Version}_x64_en-US.msi",
    "src-tauri\target\$WindowsTarget\release\bundle\msi\OMD_${Version}_x64_en-US.msi.sig",
    "src-tauri\target\$WindowsTarget\release\bundle\green\OMD_${Version}_x64_green.exe",
    "src-tauri\target\$WindowsTarget\release\bundle\green\OMD_${Version}_x64_green.zip",
    "src-tauri\target\$WindowsTarget\release\bundle\green\OMD_${Version}_x64_green.zip.sig"
  )
  $summaryArtifacts = @()
  foreach ($artifact in $artifacts) {
    $source = Join-Path $BuildDir $artifact
    Copy-Required $source (Join-Path $ExportDir (Split-Path -Leaf $artifact))
    $item = Get-Item -LiteralPath $source
    $summaryArtifacts += [ordered]@{
      name = $item.Name
      path = $source
      size = $item.Length
      sha256 = File-Sha256 $source
    }
  }
  $summary = [ordered]@{
    ok = $true
    version = $Version
    branch = $Branch
    commit = (git -C $BuildDir rev-parse HEAD).Trim()
    buildDir = $BuildDir
    exportDir = $ExportDir
    artifacts = $summaryArtifacts
    completedAt = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
  }
  $summary | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 -LiteralPath (Join-Path $ExportDir 'build-summary.json')
  $zip = Join-Path $ExportDir 'windows-artifacts.zip'
  if (Test-Path -LiteralPath $zip) { Remove-Item -Force -LiteralPath $zip }
  Compress-Archive -LiteralPath (Join-Path $ExportDir '*') -DestinationPath $zip -Force
  Write-Output "OMD_EXPORT_ZIP=$zip"
}

if (Get-Command sccache -ErrorAction SilentlyContinue) {
  sccache --show-stats
}

Write-Output 'OMD_BUILD_OK=true'
```

- [ ] **Step 3: Validate the script parses locally**

Run:

```bash
pwsh -NoProfile -Command '$null = [System.Management.Automation.Language.Parser]::ParseFile("scripts/windows/omd-release-build.ps1", [ref]$null, [ref]$errors); if ($errors.Count -gt 0) { $errors | Format-List; exit 1 }'
```

If `pwsh` is not installed on the Mac, run:

```bash
ssh 4090@192.168.101.9 "powershell -NoProfile -Command \"$null = [System.Management.Automation.Language.Parser]::ParseFile('C:\\Users\\4090\\Desktop\\omd-release-build.ps1', [ref]$null, [ref]$errors); if ($errors.Count -gt 0) { $errors | Format-List; exit 1 }\""
```

after Task 3 uploads the script.

Expected: exit 0.

- [ ] **Step 4: Commit**

Do not commit unless the user explicitly asks. If asked:

```bash
git add scripts/windows/omd-release-build.ps1
git commit -m "Add reusable Windows release build script"
```

---

### Task 2: Add Windows script configuration to release config

**Files:**
- Modify: `release.config.json`
- Modify: `src/main.rs`
- Test: Rust unit tests in `src/main.rs`

- [ ] **Step 1: Add config fields to `release.config.json`**

Modify `release.config.json` by adding these keys near the existing Windows settings:

```json
"windowsBuildScript": "C:\\Users\\4090\\Desktop\\omd-release-build.ps1",
"windowsBuildDir": "C:\\Users\\4090\\Desktop\\dix-extension-ui-release-build",
"windowsExportDirPattern": "C:\\Users\\4090\\AppData\\Local\\Temp\\omd-release-export-{version}",
"windowsBranch": "codex/tauri-rust-migration",
```

Expected: JSON remains valid.

- [ ] **Step 2: Extend `ReleaseConfig`**

In `src/main.rs`, update `ReleaseConfig` to include optional fields:

```rust
    windows_build_script: Option<String>,
    windows_build_dir: Option<String>,
    windows_export_dir_pattern: Option<String>,
    windows_branch: Option<String>,
```

Place them after `windows_target: String,`.

- [ ] **Step 3: Validate config fields when present**

In `validate_config`, add these optional checks after required Windows field checks:

```rust
    if let Some(value) = config.windows_build_script.as_deref() {
        ensure(!value.trim().is_empty(), "windowsBuildScript 不能为空")?;
    }
    if let Some(value) = config.windows_build_dir.as_deref() {
        ensure(!value.trim().is_empty(), "windowsBuildDir 不能为空")?;
    }
    if let Some(value) = config.windows_export_dir_pattern.as_deref() {
        ensure(
            value.contains("{version}"),
            "windowsExportDirPattern 必须包含 {version}",
        )?;
    }
    if let Some(value) = config.windows_branch.as_deref() {
        ensure(!value.trim().is_empty(), "windowsBranch 不能为空")?;
    }
```

- [ ] **Step 4: Add helpers for default values**

Near `join_windows_path`, add:

```rust
fn windows_build_script(config: &ReleaseConfig) -> String {
    config
        .windows_build_script
        .clone()
        .unwrap_or_else(|| r"C:\Users\4090\Desktop\omd-release-build.ps1".to_string())
}

fn windows_build_dir(config: &ReleaseConfig) -> String {
    config
        .windows_build_dir
        .clone()
        .unwrap_or_else(|| r"C:\Users\4090\Desktop\dix-extension-ui-release-build".to_string())
}

fn windows_export_dir(config: &ReleaseConfig, version: &str) -> String {
    config
        .windows_export_dir_pattern
        .as_deref()
        .unwrap_or(r"C:\Users\4090\AppData\Local\Temp\omd-release-export-{version}")
        .replace("{version}", version)
}

fn windows_branch(config: &ReleaseConfig) -> String {
    config
        .windows_branch
        .clone()
        .unwrap_or_else(|| "codex/tauri-rust-migration".to_string())
}
```

- [ ] **Step 5: Add unit test for export dir rendering**

In `mod tests`, add:

```rust
#[test]
fn windows_export_dir_uses_target_version() {
    let mut config = test_config();
    config.windows_export_dir_pattern = Some(r"C:\Temp\omd-{version}".to_string());

    assert_eq!(windows_export_dir(&config, "0.0.8"), r"C:\Temp\omd-0.0.8");
}
```

Also update `test_config()` to initialize the new fields:

```rust
windows_build_script: None,
windows_build_dir: None,
windows_export_dir_pattern: None,
windows_branch: None,
```

- [ ] **Step 6: Run focused test**

Run:

```bash
cargo test windows_export_dir_uses_target_version
```

Expected: PASS.

- [ ] **Step 7: Commit**

Do not commit unless the user explicitly asks. If asked:

```bash
git add release.config.json src/main.rs
git commit -m "Configure reusable Windows build script paths"
```

---

### Task 3: Upload and invoke the reusable script from `build-windows`

**Files:**
- Modify: `src/main.rs`
- Test: Rust unit tests in `src/main.rs`; runtime `cargo run -- build-windows ...`

- [ ] **Step 1: Add a local script path helper**

Near other path helpers in `src/main.rs`, add:

```rust
fn local_windows_build_script(root: &Path) -> PathBuf {
    root.join("scripts/windows/omd-release-build.ps1")
}
```

- [ ] **Step 2: Add remote upload helper**

Near `run_remote_powershell`, add:

```rust
fn upload_windows_build_script(ctx: &CliContext) -> Result<(), String> {
    let local_script = local_windows_build_script(&ctx.app.root);
    if !local_script.exists() {
        return Err(format!("缺少 Windows 构建脚本：{}", local_script.display()));
    }
    let remote_script = windows_build_script(&ctx.app.config);
    let remote_arg = format!("{}:{}", ctx.app.config.windows_ssh, windows_scp_path(&remote_script));
    let local_arg = local_script.to_string_lossy().to_string();
    let output = run_command_output(
        "scp",
        &["-q", local_arg.as_str(), remote_arg.as_str()],
        Some(Path::new(&ctx.app.config.source_repo)),
    )?;
    if output.status != 0 {
        return Err(command_output_summary(&output));
    }
    Ok(())
}
```

This requires `windows_scp_path` from Task 4. If Task 4 is not implemented yet, temporarily upload to the existing raw path and finish Task 4 before runtime testing.

- [ ] **Step 3: Replace generated remote build script in `build_windows`**

In `build_windows`, replace:

```rust
let worktree = remote_windows_worktree(&ctx.version, run_id);
let remote_script = remote_windows_build_script(&ctx.app.config, &worktree, &ctx.version);
let output = run_remote_powershell(ctx, &remote_script)?;
```

with:

```rust
upload_windows_build_script(ctx)?;
let export_dir = windows_export_dir(&ctx.app.config, &ctx.version);
let script = windows_build_script(&ctx.app.config);
let build_dir = windows_build_dir(&ctx.app.config);
let branch = windows_branch(&ctx.app.config);
let remote_script = format!(
    "& '{}' -Version '{}' -Repo '{}' -BuildDir '{}' -Branch '{}' -ExportDir '{}'",
    powershell_escape(&script),
    powershell_escape(&ctx.version),
    powershell_escape(&ctx.app.config.windows_repo),
    powershell_escape(&build_dir),
    powershell_escape(&branch),
    powershell_escape(&export_dir),
);
let output = run_remote_powershell(ctx, &remote_script)?;
```

Keep the existing retry block for `STATUS_ACCESS_VIOLATION` only if the script does not already handle it. After Task 1, the first implementation does not retry access violation, so keep the Rust retry branch initially.

- [ ] **Step 4: Record export dir in state summary**

In the success block of `build_windows`, include `exportDir` in the summary:

```rust
json!({
    "remoteCommit": remote_commit,
    "exportDir": export_dir,
    "message": command_output_summary(&final_output)
})
```

- [ ] **Step 5: Add PowerShell escape helper**

Near string utility helpers, add:

```rust
fn powershell_escape(value: &str) -> String {
    value.replace('`', "``").replace('\'', "''")
}
```

- [ ] **Step 6: Add unit test for command construction ingredients**

Add:

```rust
#[test]
fn powershell_escape_doubles_single_quotes() {
    assert_eq!(powershell_escape("C:\\A'B"), "C:\\A''B");
}
```

Run:

```bash
cargo test powershell_escape_doubles_single_quotes
```

Expected: PASS.

- [ ] **Step 7: Commit**

Do not commit unless the user explicitly asks. If asked:

```bash
git add src/main.rs
git commit -m "Invoke reusable Windows build script"
```

---

### Task 4: Fix Windows artifact collection by pulling a remote export zip

**Files:**
- Modify: `src/main.rs`
- Test: Rust unit tests; runtime `collect-windows-artifacts`

- [ ] **Step 1: Add Windows scp path helper**

Near `join_windows_path`, add:

```rust
fn windows_scp_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        format!("/{}", normalized)
    } else {
        normalized
    }
}
```

- [ ] **Step 2: Add tests for Windows scp path helper**

Add:

```rust
#[test]
fn windows_scp_path_converts_drive_paths() {
    assert_eq!(
        windows_scp_path(r"C:\Users\4090\AppData\Local\Temp\x.zip"),
        "/C:/Users/4090/AppData/Local/Temp/x.zip"
    );
}

#[test]
fn windows_scp_path_keeps_relative_paths_normalized() {
    assert_eq!(windows_scp_path(r"tmp\x.zip"), "tmp/x.zip");
}
```

Run:

```bash
cargo test windows_scp_path_
```

Expected: both PASS.

- [ ] **Step 3: Add unzip helper**

Near command helpers, add:

```rust
fn unzip_to_dir(zip_file: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let zip_arg = zip_file.to_string_lossy().to_string();
    let dest_arg = destination.to_string_lossy().to_string();
    let output = run_command_output(
        "python3",
        &[
            "-c",
            "import sys, zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])",
            zip_arg.as_str(),
            dest_arg.as_str(),
        ],
        None,
    )?;
    if output.status != 0 {
        return Err(command_output_summary(&output));
    }
    Ok(())
}
```

- [ ] **Step 4: Replace `collect_windows_artifacts` scp loop**

In `collect_windows_artifacts`, replace the per-artifact `scp` loop with:

```rust
let export_dir = windows_export_dir(&ctx.app.config, &ctx.version);
let remote_zip = join_windows_path(&export_dir, "windows-artifacts.zip");
let remote_arg = format!(
    "{}:{}",
    ctx.app.config.windows_ssh,
    windows_scp_path(&remote_zip)
);
let local_zip = ctx.release_dir.join("windows-artifacts.zip");
let local_arg = local_zip.to_string_lossy().to_string();
let output = run_command_output(
    "scp",
    &["-q", remote_arg.as_str(), local_arg.as_str()],
    Some(Path::new(&ctx.app.config.source_repo)),
)?;
if output.status != 0 {
    ctx.state.record_suggestion(
        "Windows export zip 拉取失败，请确认 4090 构建脚本已成功导出 windows-artifacts.zip",
        "error",
        "collect-windows-artifacts",
    )?;
    ctx.state.fail_step("collectWindowsArtifacts", "收集 Windows 产物失败", &command_output_summary(&output))?;
    return Err(command_output_summary(&output));
}
unzip_to_dir(&local_zip, &ctx.release_dir.join("windows"))?;
let _ = fs::remove_file(&local_zip);
ctx.state.complete_step(
    "collectWindowsArtifacts",
    "Windows 产物已收集",
    json!({ "source": remote_zip, "destination": ctx.release_dir.join("windows").to_string_lossy() }),
)
```

- [ ] **Step 5: Runtime verification after Task 3 build succeeds**

Run:

```bash
cargo run -- collect-windows-artifacts 0.0.7 --release-dir /Users/glame/Desktop/omd-0.0.7-release-script-verify
find /Users/glame/Desktop/omd-0.0.7-release-script-verify/windows -maxdepth 1 -type f | sort
```

Expected files:

```text
OMD_0.0.7_x64-setup.exe
OMD_0.0.7_x64-setup.exe.sig
OMD_0.0.7_x64_en-US.msi
OMD_0.0.7_x64_en-US.msi.sig
OMD_0.0.7_x64_green.exe
OMD_0.0.7_x64_green.zip
OMD_0.0.7_x64_green.zip.sig
build-summary.json
```

- [ ] **Step 6: Commit**

Do not commit unless the user explicitly asks. If asked:

```bash
git add src/main.rs
git commit -m "Collect Windows artifacts from export zip"
```

---

### Task 5: Verify end-to-end Windows build through the reusable script

**Files:**
- Modify only if verification exposes a bug.
- Runtime surfaces: CLI and HTTP summary.

- [ ] **Step 1: Run preflight in a fresh release directory**

Run:

```bash
cargo run -- preflight 0.0.7 --release-dir /Users/glame/Desktop/omd-0.0.7-release-script-verify
```

Expected:

```text
进度：开始预检
进度：预检通过
```

- [ ] **Step 2: Run Windows build**

Run:

```bash
cargo run -- build-windows 0.0.7 --release-dir /Users/glame/Desktop/omd-0.0.7-release-script-verify
```

Expected output includes:

```text
OMD_BUILD_VERSION=0.0.7
OMD_BUILD_DIR=C:\Users\4090\Desktop\dix-extension-ui-release-build
OMD_EXPORT_ZIP=C:\Users\4090\AppData\Local\Temp\omd-release-export-0.0.7\windows-artifacts.zip
OMD_BUILD_OK=true
```

- [ ] **Step 3: Run Windows artifact collection**

Run:

```bash
cargo run -- collect-windows-artifacts 0.0.7 --release-dir /Users/glame/Desktop/omd-0.0.7-release-script-verify
```

Expected:

```text
进度：Windows 产物已收集
```

- [ ] **Step 4: Inspect collected files**

Run:

```bash
find /Users/glame/Desktop/omd-0.0.7-release-script-verify/windows -maxdepth 1 -type f | sort
```

Expected: all seven Windows artifacts plus `build-summary.json` are present.

- [ ] **Step 5: Run artifact check**

Run:

```bash
cargo run -- check-artifacts 0.0.7 --release-dir /Users/glame/Desktop/omd-0.0.7-release-script-verify
```

Expected:

```text
进度：产物完整
```

- [ ] **Step 6: Run manifest generation**

Run:

```bash
cargo run -- manifest 0.0.7 --release-dir /Users/glame/Desktop/omd-0.0.7-release-script-verify
```

Expected: `github/latest.json`, `gitee/latest.json`, and `checksums.sha256` exist.

- [ ] **Step 7: Probe HTTP summary**

Run:

```bash
PORT=4188 cargo run -- serve
curl -fsS 'http://127.0.0.1:4188/api/summary?releaseDir=/Users/glame/Desktop/omd-0.0.7-release-script-verify' | python3 -m json.tool >/tmp/omd-summary.json
```

Expected: summary JSON parses and shows Windows artifacts as present. Stop the server after the request.

- [ ] **Step 8: Record findings**

If green signing still fails, capture the exact `OMD_STEP_START`, `ERROR`, and signer output. Do not continue to publish steps until all required artifacts and signatures exist.

---

### Task 6: Document the reusable Windows build workflow

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add documentation section**

Add this section after “发布原则”:

```markdown
## 4090 Windows 构建脚本

Windows 打包由 4090 上的长期脚本执行：

```powershell
C:\Users\4090\Desktop\omd-release-build.ps1 -Version 0.0.7
```

Release Console 会在执行 `build-windows` 前上传仓库内的 `scripts/windows/omd-release-build.ps1`，然后通过 SSH 调用它。脚本使用固定构建目录 `C:\Users\4090\Desktop\dix-extension-ui-release-build`，保留 `node_modules` 和 `src-tauri\target` 以加速后续构建。构建成功后会在 4090 的 export 目录生成 `windows-artifacts.zip` 和 `build-summary.json`，`collect-windows-artifacts` 只拉取这个 zip。

脚本是版本无关的；每次发布通过 `-Version` 传入目标版本，并要求 `package.json` 和 `src-tauri\tauri.conf.json` 已经等于该版本。
```

- [ ] **Step 2: Commit**

Do not commit unless the user explicitly asks. If asked:

```bash
git add README.md
git commit -m "Document reusable Windows build workflow"
```

---

## Self-Review

- Spec coverage: Covers reusable 4090 script, version-agnostic parameters, cached build speedup, green zip signing through local Tauri CLI, export zip collection, Console integration, and runtime verification.
- Placeholder scan: No TBD/TODO placeholders remain. Runtime verification has explicit commands and expected evidence.
- Type consistency: New config names use serde camelCase fields: `windowsBuildScript`, `windowsBuildDir`, `windowsExportDirPattern`, `windowsBranch`; Rust fields use snake_case equivalents.
- Known risk: If Tauri CLI still times out signing green zip, implementation must capture that exact failure. A Rust signing helper is intentionally deferred until this direct-script path proves insufficient.
