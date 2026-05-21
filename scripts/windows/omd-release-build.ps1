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
  $script:LASTEXITCODE = 0
  & $Body
  if ($LASTEXITCODE -ne $null -and $LASTEXITCODE -ne 0) {
    Fail "$Name failed with exit code $LASTEXITCODE"
  }
  Write-Output "OMD_STEP_DONE=$Name"
}

function Invoke-Native {
  param(
    [Parameter(Mandatory=$true)]
    [string]$Command,
    [string[]]$Arguments = @(),
    [string]$DisplayName = '',
    [switch]$CaptureOutput
  )

  $label = $DisplayName
  if ([string]::IsNullOrWhiteSpace($label)) { $label = $Command }

  $script:LASTEXITCODE = 0
  try {
    if ($CaptureOutput) {
      $output = & $Command @Arguments 2>&1
      $exitCode = $LASTEXITCODE
      if ($null -ne $exitCode -and $exitCode -ne 0) {
        $details = ($output | Out-String).Trim()
        if (![string]::IsNullOrWhiteSpace($details)) {
          Fail "$label failed with exit code $exitCode`n$details"
        }
        Fail "$label failed with exit code $exitCode"
      }
      return ($output | Out-String).Trim()
    }

    & $Command @Arguments
    $exitCode = $LASTEXITCODE
    if ($null -ne $exitCode -and $exitCode -ne 0) {
      Fail "$label failed with exit code $exitCode"
    }
  } catch {
    Fail "$label failed to start: $($_.Exception.Message)"
  }
}

function Normalize-FullPath([string]$Path) {
  if ([string]::IsNullOrWhiteSpace($Path)) { Fail 'Path must not be empty' }
  $full = [System.IO.Path]::GetFullPath($Path)
  $root = [System.IO.Path]::GetPathRoot($full)
  $trimChars = [char[]]@([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
  $trimmed = $full.TrimEnd($trimChars)
  if ([string]::IsNullOrEmpty($trimmed)) { return $root }
  if (![string]::IsNullOrEmpty($root)) {
    $rootTrimmed = $root.TrimEnd($trimChars)
    if ($trimmed -eq $rootTrimmed) { return $root }
  }
  return $trimmed
}

function Test-SamePath([string]$Left, [string]$Right) {
  $leftPath = Normalize-FullPath $Left
  $rightPath = Normalize-FullPath $Right
  return [System.String]::Equals($leftPath, $rightPath, [System.StringComparison]::OrdinalIgnoreCase)
}

function Test-DriveRoot([string]$Path) {
  $full = Normalize-FullPath $Path
  $root = [System.IO.Path]::GetPathRoot($full)
  if ([string]::IsNullOrEmpty($root)) { return $false }
  $root = Normalize-FullPath $root
  return (Test-SamePath $full $root)
}

function Test-AncestorOrSamePath([string]$PotentialAncestor, [string]$Path) {
  $ancestor = Normalize-FullPath $PotentialAncestor
  $child = Normalize-FullPath $Path
  if (Test-SamePath $ancestor $child) { return $true }

  $separator = [System.IO.Path]::DirectorySeparatorChar
  $prefix = $ancestor
  if (!$prefix.EndsWith($separator)) { $prefix = "$prefix$separator" }
  return $child.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)
}

function Test-PathsOverlap([string]$Left, [string]$Right) {
  return ((Test-AncestorOrSamePath $Left $Right) -or (Test-AncestorOrSamePath $Right $Left))
}

function Assert-Safe-RemovalPath([string]$Label, [string]$Path, [string]$RepoPath, [string]$BuildPath) {
  $full = Normalize-FullPath $Path
  if (Test-DriveRoot $full) {
    Fail "Refusing to remove $Label because it resolves to a drive root: $full"
  }
  if (Test-PathsOverlap $full $RepoPath) {
    Fail "Refusing to remove $Label because it overlaps the source repo: $full"
  }
  if (![string]::IsNullOrWhiteSpace($BuildPath) -and $Label -eq 'ExportDir' -and (Test-PathsOverlap $full $BuildPath)) {
    Fail "Refusing to remove ExportDir because it overlaps the build directory: $full"
  }
}

function Assert-ReleasePathsSafe([string]$RepoPath, [string]$BuildPath, [string]$ExportPath) {
  if (Test-DriveRoot $BuildPath) { Fail "BuildDir must not be a drive root: $BuildPath" }
  if (Test-DriveRoot $ExportPath) { Fail "ExportDir must not be a drive root: $ExportPath" }
  if (Test-PathsOverlap $BuildPath $RepoPath) { Fail "BuildDir must not overlap source repo: $BuildPath" }
  if (Test-PathsOverlap $ExportPath $RepoPath) { Fail "ExportDir must not overlap source repo: $ExportPath" }
  if (Test-PathsOverlap $ExportPath $BuildPath) { Fail "ExportDir must not overlap BuildDir: $ExportPath" }
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

  $job = Start-Job -ScriptBlock {
    param($TauriPath, $FilePath, $WorkingDirectory, $KeyPath)
    $ErrorActionPreference = 'Stop'
    Set-Location -LiteralPath $WorkingDirectory
    $arguments = @('signer', 'sign', '--private-key-path', $KeyPath, '--password=', $FilePath)
    $output = & $TauriPath @arguments 2>&1
    $exitCode = $LASTEXITCODE
    [pscustomobject]@{
      ExitCode = $exitCode
      Output = ($output | Out-String).Trim()
    }
    if ($null -ne $exitCode -and $exitCode -ne 0) { exit $exitCode }
  } -ArgumentList $tauri, $InputFile, $BuildDir, $key

  $completed = Wait-Job $job -Timeout 120
  if ($null -eq $completed) {
    Stop-Job $job -ErrorAction SilentlyContinue
    Remove-Job $job -Force -ErrorAction SilentlyContinue
    Fail 'green zip signer timed out after 120 seconds'
  }

  $result = Receive-Job $job
  $jobState = $job.ChildJobs[0].JobStateInfo.State
  $jobReason = $job.ChildJobs[0].JobStateInfo.Reason
  Remove-Job $job -Force -ErrorAction SilentlyContinue

  $signerOutput = ''
  if ($null -ne $result) {
    $signerOutput = ($result | ForEach-Object {
      if ($_.PSObject.Properties.Name -contains 'Output') { $_.Output } else { $_ | Out-String }
    } | Out-String).Trim()
  }

  if ($jobState -ne 'Completed') {
    $message = "green zip signer failed with job state $jobState"
    if ($null -ne $jobReason) { $message = "$message`: $($jobReason.Message)" }
    if (![string]::IsNullOrWhiteSpace($signerOutput)) { $message = "$message`n$signerOutput" }
    Fail $message
  }
  if ([string]::IsNullOrWhiteSpace($signerOutput)) {
    Fail 'green zip signer returned an empty signature'
  }
  $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
  [System.IO.File]::WriteAllText($SignatureFile, $signerOutput, $utf8NoBom)
}

function Resolve-ExportDir([string]$Version, [string]$ExportDir) {
  if (![string]::IsNullOrWhiteSpace($ExportDir)) { return $ExportDir }
  return "C:\Users\4090\AppData\Local\Temp\omd-release-export-$Version"
}

$Repo = Normalize-FullPath $Repo
$BuildDir = Normalize-FullPath $BuildDir
$ExportDir = Normalize-FullPath (Resolve-ExportDir $Version $ExportDir)
$CacheDir = Join-Path $BuildDir '.omd-release-cache'
$WindowsTarget = 'x86_64-pc-windows-msvc'

Write-Output "OMD_BUILD_VERSION=$Version"
Write-Output "OMD_SOURCE_REPO=$Repo"
Write-Output "OMD_BUILD_DIR=$BuildDir"
Write-Output "OMD_EXPORT_DIR=$ExportDir"

if (!(Test-Path -LiteralPath $Repo)) {
  Fail "Source repo does not exist: $Repo"
}
Assert-ReleasePathsSafe $Repo $BuildDir $ExportDir

if ($Clean -and (Test-Path -LiteralPath $BuildDir)) {
  Assert-Safe-RemovalPath 'BuildDir' $BuildDir $Repo $BuildDir
  Remove-Item -Recurse -Force -LiteralPath $BuildDir
}

if (!(Test-Path -LiteralPath $BuildDir)) {
  Run-Step 'clone-build-dir' {
    Invoke-Native -Command 'git' -Arguments @('clone', $Repo, $BuildDir)
  }
}

Run-Step 'sync-build-dir' {
  Invoke-Native -Command 'git' -Arguments @('-C', $BuildDir, 'fetch', '--all', '--prune')
  if (![string]::IsNullOrWhiteSpace($Commit)) {
    Invoke-Native -Command 'git' -Arguments @('-C', $BuildDir, 'reset', '--hard', $Commit)
  } else {
    Invoke-Native -Command 'git' -Arguments @('-C', $BuildDir, 'checkout', $Branch)
    Invoke-Native -Command 'git' -Arguments @('-C', $BuildDir, 'pull', '--ff-only')
  }
  Invoke-Native -Command 'git' -Arguments @('-C', $BuildDir, 'clean', '-fdx', '-e', 'node_modules', '-e', 'src-tauri/target', '-e', '.omd-release-cache')
}

$remoteCommit = Invoke-Native -Command 'git' -Arguments @('-C', $BuildDir, 'rev-parse', 'HEAD') -CaptureOutput
Write-Output "OMD_REMOTE_COMMIT=$remoteCommit"

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
$packageJsonHash = File-Sha256 (Join-Path $BuildDir 'package.json')
$packageLockHash = File-Sha256 (Join-Path $BuildDir 'package-lock.json')
$nodeVersion = Invoke-Native -Command 'node' -Arguments @('--version') -CaptureOutput
$npmVersion = Invoke-Native -Command 'npm' -Arguments @('--version') -CaptureOutput
$cacheKey = "package=$packageJsonHash`nlock=$packageLockHash`nnode=$nodeVersion`nnpm=$npmVersion"
$cacheKeyHash = [System.BitConverter]::ToString(
  [System.Security.Cryptography.SHA256]::Create().ComputeHash([System.Text.Encoding]::UTF8.GetBytes($cacheKey))
).Replace('-', '').ToLowerInvariant()
$npmCacheKeyFile = Join-Path $CacheDir 'npm-cache-key.sha256'
$previousNpmCacheKey = ''
if (Test-Path -LiteralPath $npmCacheKeyFile) {
  $previousNpmCacheKey = (Get-Content -Raw -LiteralPath $npmCacheKeyFile).Trim()
}
if (!(Test-Path -LiteralPath (Join-Path $BuildDir 'node_modules')) -or $cacheKeyHash -ne $previousNpmCacheKey) {
  Run-Step 'npm-ci' {
    Invoke-Native -Command 'npm' -Arguments @('ci')
  }
  Set-Content -Encoding utf8 -NoNewline -LiteralPath $npmCacheKeyFile -Value $cacheKeyHash
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
    Invoke-Native -Command 'npm' -Arguments @('run', 'lint')
  }
}

if (!$SkipBuild) {
  Run-Step 'tauri-bundles' {
    Invoke-Native -Command 'npm' -Arguments @('run', 'tauri:build', '--', '--bundles', 'nsis,msi', '--target', $WindowsTarget)
  }

  Run-Step 'tauri-no-bundle' {
    Invoke-Native -Command 'npm' -Arguments @('run', 'tauri:build', '--', '--no-bundle', '--target', $WindowsTarget)
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
  if (Test-Path -LiteralPath $ExportDir) {
    Assert-Safe-RemovalPath 'ExportDir' $ExportDir $Repo $BuildDir
    Remove-Item -Recurse -Force -LiteralPath $ExportDir
  }
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
    commit = Invoke-Native -Command 'git' -Arguments @('-C', $BuildDir, 'rev-parse', 'HEAD') -CaptureOutput
    buildDir = $BuildDir
    exportDir = $ExportDir
    artifacts = $summaryArtifacts
    completedAt = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
  }
  $summaryPath = Join-Path $ExportDir 'build-summary.json'
  $summaryJson = $summary | ConvertTo-Json -Depth 6
  $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
  [System.IO.File]::WriteAllText($summaryPath, $summaryJson, $utf8NoBom)
  $zip = Join-Path $ExportDir 'windows-artifacts.zip'
  $zipEntries = @($summaryPath)
  foreach ($artifact in $summaryArtifacts) {
    $exportedArtifact = Join-Path $ExportDir $artifact.name
    if (Test-SamePath $exportedArtifact $zip) { continue }
    $zipEntries += $exportedArtifact
  }
  if (Test-Path -LiteralPath $zip) { Remove-Item -Force -LiteralPath $zip }
  Compress-Archive -LiteralPath $zipEntries -DestinationPath $zip -Force
  Write-Output "OMD_EXPORT_ZIP=$zip"
}

if (Get-Command sccache -ErrorAction SilentlyContinue) {
  Invoke-Native -Command 'sccache' -Arguments @('--show-stats')
}

Write-Output 'OMD_BUILD_OK=true'
