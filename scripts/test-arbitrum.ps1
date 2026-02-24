# MEV Protocol - Arbitrum Test Script
# This script tests the setup on Arbitrum Sepolia testnet

$ErrorActionPreference = "Stop"

Write-Host "================================" -ForegroundColor Cyan
Write-Host "  MEV Protocol - Arbitrum Test  " -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Cyan

# Check prerequisites
Write-Host "`n[1/5] Checking prerequisites..." -ForegroundColor Yellow

# Check Foundry
if (!(Get-Command forge -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: Foundry not installed. Run: curl -L https://foundry.paradigm.xyz | bash" -ForegroundColor Red
    exit 1
}
Write-Host "  ✓ Foundry installed" -ForegroundColor Green

# Check Rust
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: Rust not installed. Get it from https://rustup.rs" -ForegroundColor Red
    exit 1
}
Write-Host "  ✓ Rust installed" -ForegroundColor Green

# Load environment
Write-Host "`n[2/5] Loading environment..." -ForegroundColor Yellow

if (Test-Path "config/.env") {
    Get-Content "config/.env" | ForEach-Object {
        if ($_ -match "^([^#][^=]+)=(.*)$") {
            [Environment]::SetEnvironmentVariable($matches[1].Trim(), $matches[2].Trim())
        }
    }
    Write-Host "  ✓ Environment loaded" -ForegroundColor Green
} else {
    Write-Host "  ! No .env file found, using defaults" -ForegroundColor Yellow
}

# Build contracts
Write-Host "`n[3/5] Building contracts..." -ForegroundColor Yellow
Push-Location contracts
forge build
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Contract build failed" -ForegroundColor Red
    Pop-Location
    exit 1
}
Pop-Location
Write-Host "  ✓ Contracts built" -ForegroundColor Green

# Run contract tests
Write-Host "`n[4/5] Running contract tests..." -ForegroundColor Yellow
Push-Location contracts
forge test -vv
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Tests failed" -ForegroundColor Red
    Pop-Location
    exit 1
}
Pop-Location
Write-Host "  ✓ All tests passed" -ForegroundColor Green

# Build Rust core
Write-Host "`n[5/5] Building Rust core..." -ForegroundColor Yellow
Push-Location core
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Rust build failed" -ForegroundColor Red
    Pop-Location
    exit 1
}
Pop-Location
Write-Host "  ✓ Rust core built" -ForegroundColor Green

Write-Host "`n================================" -ForegroundColor Cyan
Write-Host "  All checks passed!  " -ForegroundColor Green
Write-Host "================================" -ForegroundColor Cyan

Write-Host "`nNext steps:"
Write-Host "  1. Get testnet ETH from https://faucet.arbitrum.io/"
Write-Host "  2. Set PRIVATE_KEY in config/.env"
Write-Host "  3. Deploy to testnet:"
Write-Host "     cd contracts"
Write-Host "     forge script script/DeployArbitrum.s.sol:DeployArbitrumSepolia --rpc-url `$ARBITRUM_SEPOLIA_RPC --broadcast"
Write-Host ""
