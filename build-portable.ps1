$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$nodeBin = Join-Path $env:USERPROFILE ".cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin"
if (Test-Path $nodeBin) {
  $env:PATH = "$nodeBin;$env:PATH"
}
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

$pnpm = "pnpm"
$codexPnpmCandidates = @(
  (Join-Path $env:USERPROFILE ".cache\codex-runtimes\codex-primary-runtime\dependencies\bin\pnpm.cmd"),
  (Join-Path $env:USERPROFILE ".cache\codex-runtimes\codex-primary-runtime\dependencies\bin\fallback\pnpm.cmd")
)
$codexPnpm = $codexPnpmCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($codexPnpm) {
  $pnpm = $codexPnpm
  # Tauri's beforeBuildCommand runs `pnpm build` in a child process.
  # Put the pnpm directory on PATH so that child process can resolve pnpm.
  $pnpmDir = Split-Path $codexPnpm
  $env:PATH = "$pnpmDir;$env:PATH"
}

Push-Location $root
try {
  & $pnpm install
  & $pnpm build:app

  $portableRoot = Join-Path $root "dist"
  $portableDir = Join-Path $portableRoot "VideoSizeComposer"
  if (Test-Path $portableDir) {
    Remove-Item -LiteralPath $portableDir -Recurse -Force
  }
  New-Item -ItemType Directory -Force -Path $portableDir | Out-Null

  Copy-Item -LiteralPath "src-tauri\target\release\videosize-composer.exe" -Destination (Join-Path $portableDir "VideoSizeComposer.exe") -Force

  foreach ($tool in @("ffmpeg.exe", "ffprobe.exe")) {
    $found = Get-Command $tool -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) {
      Copy-Item -LiteralPath $found.Source -Destination (Join-Path $portableDir $tool) -Force
    } else {
      throw "$tool was not found on PATH. Refusing to create a portable build that cannot inspect or encode video."
    }
  }

  @"
VideoSize Composer Portable

Run VideoSizeComposer.exe directly.

This folder is portable. ffmpeg.exe and ffprobe.exe are bundled and required.
Windows requires Microsoft Edge WebView2 Runtime, which is normally already installed on Windows 10/11.
"@ | Set-Content -LiteralPath (Join-Path $portableDir "README-PORTABLE.txt") -Encoding UTF8

  Write-Host ""
  Write-Host "Portable build:"
  Write-Host $portableDir
} finally {
  Pop-Location
}
