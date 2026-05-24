param(
    [ValidateSet("debug", "release")]
    [string] $Configuration = "release",
    [string] $OutputDir = "build\portable\PatternGifStudio"
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$portableRoot = Join-Path $root "build\portable"
$output = Join-Path $root $OutputDir
$targetProfile = if ($Configuration -eq "release") { "release" } else { "debug" }
$exe = Join-Path $root "target\$targetProfile\pattern-gif-studio.exe"
$previousRustflags = $env:RUSTFLAGS
$pathRemapFlags = @("--remap-path-prefix=$root=.")
if ($env:USERPROFILE) {
    $pathRemapFlags += "--remap-path-prefix=$env:USERPROFILE=%USERPROFILE%"
}
$env:RUSTFLAGS = (@($previousRustflags) + $pathRemapFlags | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }) -join " "

Push-Location $root
try {
    if ($Configuration -eq "release") {
        cargo build --release
    } else {
        cargo build
    }

    if (-not $output.StartsWith($root)) {
        throw "Output directory must be inside the project root."
    }

    if (-not $portableRoot.StartsWith($root)) {
        throw "Portable package root must be inside the project root."
    }

    if (Test-Path $portableRoot) {
        Remove-Item -LiteralPath $portableRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $output | Out-Null

    Copy-Item -LiteralPath $exe -Destination (Join-Path $output "PatternGifStudio.exe")

    foreach ($dir in @("assets", "presets")) {
        $source = Join-Path $root $dir
        if (Test-Path $source) {
            Copy-Item -LiteralPath $source -Destination (Join-Path $output $dir) -Recurse
        }
    }

    foreach ($dir in @("app_data", "exports")) {
        New-Item -ItemType Directory -Force -Path (Join-Path $output $dir) | Out-Null
    }

    Write-Host "Portable package created: $output"
}
finally {
    $env:RUSTFLAGS = $previousRustflags
    Pop-Location
}
