# Register (or update) a daily Scheduled Task that refreshes Orchestrator's
# cache by running `orchestrator ingest`, so the desktop app isn't stale
# when you open it. The app reads the cache this writes.
#
#   powershell -ExecutionPolicy Bypass -File scripts\schedule-ingest.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\schedule-ingest.ps1 -At 7am
#
# Remove it with:
#   Unregister-ScheduledTask -TaskName "Orchestrator Ingest" -Confirm:$false

param([string]$At = "8am")

$ErrorActionPreference = "Stop"

$exe = "$env:USERPROFILE\.cargo\bin\orchestrator.exe"
if (-not (Test-Path $exe)) {
    throw "orchestrator CLI not found at $exe. Install it first:  cargo install --path ."
}

$me = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name

$action    = New-ScheduledTaskAction -Execute $exe -Argument "ingest"
$trigger   = New-ScheduledTaskTrigger -Daily -At $At
$settings  = New-ScheduledTaskSettingsSet -StartWhenAvailable -RunOnlyIfNetworkAvailable `
             -ExecutionTimeLimit (New-TimeSpan -Minutes 10)
$principal = New-ScheduledTaskPrincipal -UserId $me -LogonType Interactive -RunLevel Limited

Register-ScheduledTask -TaskName "Orchestrator Ingest" -Action $action -Trigger $trigger `
    -Settings $settings -Principal $principal `
    -Description "Daily refresh of Orchestrator delivery data (orchestrator ingest)" -Force | Out-Null

Write-Host "Scheduled 'Orchestrator Ingest' daily at $At (runs when available; needs network)."
Write-Host "Remove with:  Unregister-ScheduledTask -TaskName 'Orchestrator Ingest' -Confirm:`$false"
