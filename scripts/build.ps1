# MEV Protocol - Windows Build Script
# Run: .\scripts\build.ps1

$ErrorActionPreference = "Stop"

Write-Host "==================================" -ForegroundColor Cyan
Write-Host "MEV Protocol - Build Script" -ForegroundColor Cyan
Write-Host "==================================" -ForegroundColor Cyan

# Build C Hot Path
Write-Host "`n[1/4] Building C Hot Path..." -ForegroundColor Green
Push-Location fast
if (Test-Path "Makefile") {
    # Use mingw32-make or make
    if (Get-Command mingw32-make -ErrorAction SilentlyContinue) {
        mingw32-make clean
        mingw32-make
    } elseif (Get-Command make -ErrorAction SilentlyContinue) {
        make clean
        make
    } else {
        Write-Host "Warning: make not found. Install MinGW or MSYS2" -ForegroundColor Yellow
    }
}
Pop-Location

# Build Rust Core
Write-Host "`n[2/4] Building Rust Core..." -ForegroundColor Green
Push-Location core
cargo build --release
Pop-Location

# Build Go Network
Write-Host "`n[3/4] Building Go Network..." -ForegroundColor Green
Push-Location network
$env:CGO_ENABLED = "0"
go build -o ..\bin\mev-node.exe .\cmd\mev-node
Pop-Location

# Build Solidity Contracts
Write-Host "`n[4/4] Building Solidity Contracts..." -ForegroundColor Green
Push-Location contracts
forge build
Pop-Location

Write-Host "`n==================================" -ForegroundColor Cyan
Write-Host "Build Complete!" -ForegroundColor Cyan
Write-Host "==================================" -ForegroundColor Cyan

Write-Host "`nBinaries:"
Write-Host "  - core\target\release\mev-engine.exe"
Write-Host "  - bin\mev-node.exe"
Write-Host "  - contracts\out\"
Write-Host "  - fast\lib\libmev_fast.a"
