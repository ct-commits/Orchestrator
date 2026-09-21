# Build the Orchestrator desktop app (production — no dev server, frontend
# embedded) and install a Desktop shortcut to it. Re-run after pulling
# changes to refresh the installed app.
#
#   powershell -ExecutionPolicy Bypass -File scripts\install-shortcut.ps1
#
# Uses `tauri build` (NOT `cargo build --release`, which leaves Tauri's
# `dev` flag on so the app would try to load the dev server URL).

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot   # scripts/.. = repo root
Set-Location $repo

if (-not (Test-Path "$repo\node_modules")) {
    Write-Host "Installing frontend dependencies..."
    npm install
    if ($LASTEXITCODE -ne 0) { throw "npm install failed" }
}

Write-Host "Building production app (tauri build)..."
npm run tauri build -- --no-bundle
if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }

$exe = "$repo\target\release\orchestrator-app.exe"
if (-not (Test-Path $exe)) { throw "built exe not found at $exe" }

$dest = "$env:LOCALAPPDATA\Orchestrator"
New-Item -ItemType Directory -Force -Path $dest | Out-Null

# Stop a running instance so the exe isn't locked while we copy over it.
Get-Process -Name "Orchestrator", "orchestrator-app" -ErrorAction SilentlyContinue |
    Stop-Process -Force
Start-Sleep -Milliseconds 500

Copy-Item $exe "$dest\Orchestrator.exe" -Force
Copy-Item "$repo\src-tauri\icons\icon.ico" "$dest\icon.ico" -Force

$lnk = Join-Path ([Environment]::GetFolderPath("Desktop")) "Orchestrator.lnk"
$ws = New-Object -ComObject WScript.Shell
$s = $ws.CreateShortcut($lnk)
$s.TargetPath = "$dest\Orchestrator.exe"
$s.WorkingDirectory = $dest
$s.IconLocation = "$dest\icon.ico"
$s.Description = "Orchestrator - local-first portfolio dashboard"
$s.Save()

Write-Host ""
Write-Host "Installed: $dest\Orchestrator.exe"
Write-Host "Shortcut : $lnk"
Write-Host "Done - double-click the Desktop shortcut to open."
