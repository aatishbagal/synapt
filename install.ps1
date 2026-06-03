$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ParentDir = Split-Path -Parent $ScriptDir
$CoreDir   = Join-Path $ParentDir "synapt-core"

if (Test-Path $CoreDir) {
    Write-Host "synapt-core already present at $CoreDir"
} else {
    Write-Host "Cloning synapt-core..."
    git clone https://github.com/aatishbagal/synapt-core.git $CoreDir
    Write-Host "synapt-core cloned."
}

Write-Host "Install complete. Run: cargo tauri dev"
