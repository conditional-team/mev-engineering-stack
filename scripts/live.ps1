# ═══════════════════════════════════════════════════════════════════════════════
# MEV PROTOCOL — LIVE LAUNCHER
# Single command: .\scripts\live.ps1
# Starts: Rust gRPC core + Go node + Dashboard
# ═══════════════════════════════════════════════════════════════════════════════

param(
    [ValidateSet("mainnet","testnet")]
    [string]$Network = "mainnet",
    [ValidateSet("simulate","live")]
    [string]$ExecutionMode = "simulate",
    [switch]$NoDashboard,
    [switch]$BuildFirst
)

$ErrorActionPreference = "Stop"
$ROOT = Split-Path -Parent (Split-Path -Parent $PSCommandPath)

# ── Load .env ────────────────────────────────────────────────────────────────
$envFile = Join-Path $ROOT ".env"
if (Test-Path $envFile) {
    Get-Content $envFile | ForEach-Object {
        if ($_ -match '^\s*([^#][^=]+?)\s*=\s*(.+?)\s*$') {
            [System.Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], "Process")
        }
    }
}

# ── Banner ───────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "  ╔═══════════════════════════════════════════════════════════╗" -ForegroundColor Blue
Write-Host "  ║                                                           ║" -ForegroundColor Blue
Write-Host "  ║     ███╗   ███╗███████╗██╗   ██╗                          ║" -ForegroundColor Cyan
Write-Host "  ║     ████╗ ████║██╔════╝██║   ██║                          ║" -ForegroundColor Cyan
Write-Host "  ║     ██╔████╔██║█████╗  ██║   ██║                          ║" -ForegroundColor Cyan
Write-Host "  ║     ██║╚██╔╝██║██╔══╝  ╚██╗ ██╔╝                          ║" -ForegroundColor Cyan
Write-Host "  ║     ██║ ╚═╝ ██║███████╗ ╚████╔╝                           ║" -ForegroundColor Cyan
Write-Host "  ║     ╚═╝     ╚═╝╚══════╝  ╚═══╝                            ║" -ForegroundColor Cyan
Write-Host "  ║                                                           ║" -ForegroundColor Blue
Write-Host "  ║     P R O T O C O L  —  L I V E   E N G I N E            ║" -ForegroundColor White
Write-Host "  ║                                                           ║" -ForegroundColor Blue
Write-Host "  ╚═══════════════════════════════════════════════════════════╝" -ForegroundColor Blue
Write-Host ""

# ── Network config ───────────────────────────────────────────────────────────
if ($Network -eq "mainnet") {
    $wsUrl  = $env:ARBITRUM_WS_URL
    $chain  = "Arbitrum One (42161)"
    if (-not $wsUrl) {
        Write-Host "  [!] ARBITRUM_WS_URL not set in .env" -ForegroundColor Red
        exit 1
    }
    # Build multi-endpoint string (comma-separated)
    $rpcEndpoints = $env:MEV_RPC_ENDPOINTS
    if (-not $rpcEndpoints) {
        $rpcEndpoints = $wsUrl
        if ($env:ARBITRUM_WS_URL_2) { $rpcEndpoints += "," + $env:ARBITRUM_WS_URL_2 }
        if ($env:ARBITRUM_WS_URL_3) { $rpcEndpoints += "," + $env:ARBITRUM_WS_URL_3 }
    }
} else {
    $wsUrl  = $env:ARBITRUM_WS_URL -replace "arb-mainnet", "arb-sepolia"
    $chain  = "Arbitrum Sepolia (421614)"
    $rpcEndpoints = $wsUrl
}

Write-Host "  Network:    $chain" -ForegroundColor Yellow
$epCount = ($rpcEndpoints -split ',').Count
Write-Host "  RPC Pool:   $epCount endpoints" -ForegroundColor Yellow
if ($ExecutionMode -eq "live") {
    Write-Host "  Mode:       LIVE (signed bundle submission enabled)" -ForegroundColor Red

    # Fail fast on missing credentials required for relay submission.
    if (-not $env:PRIVATE_KEY) {
        Write-Host "  [!] PRIVATE_KEY is required when -ExecutionMode live" -ForegroundColor Red
        exit 1
    }
    if (-not $env:FLASHBOTS_SIGNING_KEY) {
        Write-Host "  [!] FLASHBOTS_SIGNING_KEY is required when -ExecutionMode live" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "  Mode:       SIMULATION (read-only, no execution)" -ForegroundColor Green
}
Write-Host "  Dashboard:  http://localhost — metrics on :9091" -ForegroundColor Gray
Write-Host ""

# ── Optional build ───────────────────────────────────────────────────────────
if ($BuildFirst) {
    Write-Host "  [1/4] Building Rust core..." -ForegroundColor Green
    Push-Location (Join-Path $ROOT "core")
    cargo build --release 2>&1 | Select-Object -Last 3
    Pop-Location

    Write-Host "  [2/4] Building Go node..." -ForegroundColor Green
    Push-Location (Join-Path $ROOT "network")
    go build -o (Join-Path $ROOT "bin\mev-node.exe") ./cmd/mev-node/ 2>&1
    Pop-Location

    Write-Host "  [3/4] Build complete" -ForegroundColor Green
    Write-Host ""
}

# ── Kill old processes ───────────────────────────────────────────────────────
Stop-Process -Name "mev-node" -Force -ErrorAction SilentlyContinue
Stop-Process -Name "grpc_server" -Force -ErrorAction SilentlyContinue

# ── Start Rust core gRPC server ──────────────────────────────────────────────
Write-Host "  [>] Starting Rust gRPC core..." -ForegroundColor Cyan

$rustJob = Start-Job -ScriptBlock {
    param($coreDir, $envFile)
    Set-Location $coreDir
    if (Test-Path $envFile) {
        Get-Content $envFile | ForEach-Object {
            if ($_ -match '^\s*([^#][^=]+?)\s*=\s*(.+?)\s*$') {
                [System.Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], "Process")
            }
        }
    }
    cargo run --release --bin grpc_server 2>&1
} -ArgumentList (Join-Path $ROOT "core"), $envFile

Start-Sleep -Seconds 3
Write-Host "  [+] Rust core started (PID: $($rustJob.Id))" -ForegroundColor Green

# ── Start Go node (metrics + dashboard backend) ─────────────────────────────
Write-Host "  [>] Starting Go network node..." -ForegroundColor Cyan

$env:MEV_RPC_ENDPOINTS   = $rpcEndpoints
$env:MEV_MEMPOOL_MIN_VALUE = "0"
$env:MEV_MEMPOOL_FILTER   = "false"
$env:MEV_METRICS_ENABLED  = "true"
$env:MEV_METRICS_ADDR     = ":9091"
$env:EXECUTE_MODE        = $ExecutionMode

$goJob = Start-Job -ScriptBlock {
    param($networkDir, $rpcEndpoints, $executionMode)
    Set-Location $networkDir
    $env:MEV_RPC_ENDPOINTS    = $rpcEndpoints
    $env:MEV_MEMPOOL_MIN_VALUE = "0"
    $env:MEV_MEMPOOL_FILTER    = "false"
    $env:MEV_METRICS_ENABLED   = "true"
    $env:MEV_METRICS_ADDR      = ":9091"
    $env:EXECUTE_MODE         = $executionMode
    go run ./cmd/mev-node/ 2>&1
} -ArgumentList (Join-Path $ROOT "network"), $rpcEndpoints, $ExecutionMode

Start-Sleep -Seconds 3
Write-Host "  [+] Go node started (PID: $($goJob.Id))" -ForegroundColor Green

# ── Open dashboard ───────────────────────────────────────────────────────────
if (-not $NoDashboard) {
    $dashboardPath = Join-Path $ROOT "dashboard\index.html"
    Write-Host "  [>] Opening dashboard..." -ForegroundColor Cyan
    Start-Process $dashboardPath
    Write-Host "  [+] Dashboard opened in browser" -ForegroundColor Green
}

# ── Status ───────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "  ═══════════════════════════════════════════════════════════" -ForegroundColor DarkGray
Write-Host "  All systems running. Press Ctrl+C to stop." -ForegroundColor White
Write-Host "  ═══════════════════════════════════════════════════════════" -ForegroundColor DarkGray
Write-Host ""
Write-Host "  Go node logs:   Receive-Job -Id $($goJob.Id)" -ForegroundColor DarkGray
Write-Host "  Rust core logs: Receive-Job -Id $($rustJob.Id)" -ForegroundColor DarkGray
Write-Host ""

# ── Stream logs ──────────────────────────────────────────────────────────────
try {
    while ($true) {
        # Print Go node output
        $goOutput = Receive-Job -Id $goJob.Id -ErrorAction SilentlyContinue
        if ($goOutput) {
            $goOutput | ForEach-Object {
                Write-Host "  [GO]   $_" -ForegroundColor DarkCyan
            }
        }

        # Print Rust output
        $rustOutput = Receive-Job -Id $rustJob.Id -ErrorAction SilentlyContinue
        if ($rustOutput) {
            $rustOutput | ForEach-Object {
                Write-Host "  [RUST] $_" -ForegroundColor DarkYellow
            }
        }

        Start-Sleep -Milliseconds 500
    }
}
finally {
    # Cleanup on Ctrl+C
    Write-Host ""
    Write-Host "  Shutting down..." -ForegroundColor Yellow
    Stop-Job -Id $goJob.Id -ErrorAction SilentlyContinue
    Stop-Job -Id $rustJob.Id -ErrorAction SilentlyContinue
    Remove-Job -Id $goJob.Id -Force -ErrorAction SilentlyContinue
    Remove-Job -Id $rustJob.Id -Force -ErrorAction SilentlyContinue
    Stop-Process -Name "mev-node" -Force -ErrorAction SilentlyContinue
    Write-Host "  All processes stopped." -ForegroundColor Green
}
