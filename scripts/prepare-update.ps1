# Zenith Launcher Update Preparation Script
# This script helps prepare files for releasing updates

param(
    [Parameter(Mandatory=$true)]
    [string]$Version,
    
    [Parameter(Mandatory=$false)]
    [string]$Notes = "Bug fixes and performance improvements"
)

Write-Host "🚀 Preparing Zenith Launcher Update v$Version" -ForegroundColor Cyan

# Check if version is valid
if (-not ($Version -match '^\d+\.\d+\.\d+$')) {
    Write-Error "Version must be in format x.y.z (e.g., 0.1.1)"
    exit 1
}

# Update version in package.json
Write-Host "📝 Updating package.json version..." -ForegroundColor Yellow
$packageJson = Get-Content "package.json" | ConvertFrom-Json
$packageJson.version = $Version
$packageJson | ConvertTo-Json -Depth 10 | Set-Content "package.json"

# Update version in tauri.conf.json
Write-Host "📝 Updating tauri.conf.json version..." -ForegroundColor Yellow
$tauriConf = Get-Content "src-tauri/tauri.conf.json" | ConvertFrom-Json
$tauriConf.version = $Version
$tauriConf | ConvertTo-Json -Depth 10 | Set-Content "src-tauri/tauri.conf.json"

# Update version in Cargo.toml
Write-Host "📝 Updating Cargo.toml version..." -ForegroundColor Yellow
$cargoContent = Get-Content "src-tauri/Cargo.toml"
$cargoContent = $cargoContent -replace 'version = "\d+\.\d+\.\d+"', "version = `"$Version`""
$cargoContent | Set-Content "src-tauri/Cargo.toml"

Write-Host "✅ Version updated to $Version in all files" -ForegroundColor Green

# Check if signing environment is set
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    Write-Host "⚠️  Setting up signing environment..." -ForegroundColor Yellow
    if (Test-Path "zenith-private.key") {
        $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "zenith-private.key" -Raw
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
        Write-Host "✅ Signing environment configured" -ForegroundColor Green
    } else {
        Write-Warning "Private key not found. Build will proceed without signing."
    }
}

# Build the application
Write-Host "🔨 Building application with auto-signing..." -ForegroundColor Yellow
npm run tauri build

if ($LASTEXITCODE -ne 0) {
    Write-Error "Build failed!"
    exit 1
}

Write-Host "✅ Build completed successfully" -ForegroundColor Green

# Check for signature file and create latest.json template
$sigFile = "src-tauri/target/release/bundle/nsis/Zenith Launcher_${Version}_x64-setup.exe.sig"
if (Test-Path $sigFile) {
    $signature = Get-Content $sigFile -Raw
    $currentDate = Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ"
    
    $latestJson = @{
        version = $Version
        notes = $Notes
        pub_date = $currentDate
        platforms = @{
            "windows-x86_64" = @{
                signature = $signature.Trim()
                url = "https://github.com/itsBintang/Zenith/releases/download/v$Version/Zenith.Launcher_${Version}_x64-setup.exe"
            }
        }
    }
    
    # Create latest.json in root for GitHub releases
    $latestJson | ConvertTo-Json -Depth 10 | Set-Content "latest.json"
    
    # Also create release-output folder for convenience
    New-Item -ItemType Directory -Force -Path "release-output" | Out-Null
    $latestJson | ConvertTo-Json -Depth 10 | Set-Content "release-output/latest.json"
    Copy-Item "src-tauri/target/release/bundle/nsis/Zenith Launcher_${Version}_x64-setup.exe" "release-output/Zenith.Launcher_${Version}_x64-setup.exe"
    
    Write-Host "✅ latest.json created in root directory" -ForegroundColor Green
    Write-Host "✅ Release files also created in release-output/" -ForegroundColor Green
} else {
    Write-Warning "Signature file not found. Signing may have failed."
}

Write-Host "Update preparation completed!" -ForegroundColor Green
Write-Host "Next steps: Upload files from release-output/ to GitHub" -ForegroundColor Cyan
