param(
    [string]$TestLibrary = "A:\music_test",
    [string]$TestStaging = "A:\music_test_staging",
    [string]$TestQuarantine = "A:\music_test_quarantine",
    [string]$TestDb = "cassette_test.db"
)

$ErrorActionPreference = "Stop"

$targets = @($TestLibrary, $TestStaging, $TestQuarantine)
foreach ($target in $targets) {
    if (Test-Path $target) {
        Remove-Item -Recurse -Force $target
        Write-Host "Removed $target"
    }
}

if (Test-Path $TestDb) {
    Remove-Item -Force $TestDb
    Write-Host "Removed $TestDb"
}

Write-Host "Validation sandbox reset complete."
