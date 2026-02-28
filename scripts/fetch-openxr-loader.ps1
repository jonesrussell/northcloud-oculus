# Downloads the Khronos OpenXR loader (openxr_loader.dll) and copies it next to the release exe.
# Run once after clone or after `cargo clean` so `cargo run --release` can find the loader.
# Requires: PowerShell 5+ (Expand-Archive, Invoke-WebRequest)

$ErrorActionPreference = "Stop"
$Version = "1.1.57"
$Repo = "KhronosGroup/OpenXR-SDK-Source"
$NupkgName = "OpenXR.Loader.$Version.nupkg"
$Url = "https://github.com/$Repo/releases/download/release-$Version/$NupkgName"

$Root = Split-Path $PSScriptRoot -Parent
$TargetDir = Join-Path $Root "target\release"
$ExtractDir = Join-Path $Root "target\openxr_extract"
$ZipPath = Join-Path $Root "target\openxr_loader.zip"

if (-not (Test-Path $TargetDir)) {
    Write-Host "Building release first so target\release exists..."
    Set-Location $Root
    cargo build --release
}

New-Item -ItemType Directory -Force -Path (Join-Path $Root "target") | Out-Null
Write-Host "Downloading $NupkgName..."
Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing

Write-Host "Extracting..."
if (Test-Path $ExtractDir) { Remove-Item $ExtractDir -Recurse -Force }
Expand-Archive -Path $ZipPath -DestinationPath $ExtractDir -Force

$Dll = Join-Path $ExtractDir "native\x64\release\bin\openxr_loader.dll"
if (-not (Test-Path $Dll)) {
    Write-Error "Expected DLL not found: $Dll"
}
Copy-Item $Dll -Destination (Join-Path $TargetDir "openxr_loader.dll") -Force
Write-Host "Copied openxr_loader.dll to $TargetDir"
# Optional cleanup
Remove-Item $ExtractDir -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $ZipPath -Force -ErrorAction SilentlyContinue
