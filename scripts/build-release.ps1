param(
    [ValidateSet("Shim", "HardLink")]
    [string]$AliasMode = "Shim"
)

$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$releaseDir = Join-Path $root "target\release"
$exePath = Join-Path $releaseDir "rosettrism.exe"

Push-Location $root
try {
    cargo build --release --bin rosettrism
}
finally {
    Pop-Location
}

if (!(Test-Path -LiteralPath $exePath)) {
    throw "Expected release binary was not found: $exePath"
}

foreach ($alias in @("rstm", "rosm")) {
    $aliasPath = Join-Path $releaseDir $alias
    $cmdPath = Join-Path $releaseDir "$alias.cmd"
    $aliasExePath = Join-Path $releaseDir "$alias.exe"

    Remove-Item -LiteralPath $aliasPath -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $cmdPath -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $aliasExePath -ErrorAction SilentlyContinue

    if ($AliasMode -eq "HardLink") {
        New-Item -ItemType HardLink -Path $aliasExePath -Target $exePath | Out-Null
    }
    else {
        Set-Content -LiteralPath $cmdPath -Encoding ASCII -Value @(
            "@echo off",
            "`"%~dp0rosettrism.exe`" %*",
            "exit /b %ERRORLEVEL%"
        )
    }
}

Write-Host "Built $exePath"
if ($AliasMode -eq "HardLink") {
    Write-Host "Created hard-link aliases: rstm.exe, rosm.exe"
}
else {
    Write-Host "Created shim aliases: rstm.cmd, rosm.cmd"
}
