package main

import (
	"context"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/mev-protocol/network/internal/mempool"
	"github.com/mev-protocol/network/internal/relay"
	"github.com/mev-protocol/network/internal/rpc"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
)

func main() {
	// Setup logging
	zerolog.TimeFieldFormat = zerolog.TimeFormatUnixMs
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})

	log.Info().Msg("MEV Protocol Network Node v0.1.0")
	log.Info().Msg("================================")

	// Load configuration
	cfg, err := loadConfig()
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to load config")
	}

	// Create context with cancellation
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Initialize components
	rpcPool := rpc.NewPool(cfg.RPC)
	mempoolMonitor := mempool.NewMonitor(cfg.Mempool, rpcPool)
	flashbotsRelay := relay.NewFlashbots(cfg.Relay)

	// Start components
	if err := rpcPool.Start(ctx); err != nil {
		log.Fatal().Err(err).Msg("Failed to start RPC pool")
	}

	if err := mempoolMonitor.Start(ctx); err != nil {
		log.Fatal().Err(err).Msg("Failed to start mempool monitor")
	}

	if err := flashbotsRelay.Start(ctx); err != nil {
		log.Fatal().Err(err).Msg("Failed to start relay")
	}

	log.Info().Msg("All components started successfully")

	// Handle shutdown
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	<-sigChan
	log.Info().Msg("Shutdown signal received")

	// Graceful shutdown
	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer shutdownCancel()

	mempoolMonitor.Stop(shutdownCtx)
	flashbotsRelay.Stop(shutdownCtx)
	rpcPool.Stop(shutdownCtx)

	log.Info().Msg("Shutdown complete")
}

type Config struct {
	RPC     rpc.Config
	Mempool mempool.Config
	Relay   relay.Config
}

func loadConfig() (*Config, error) {
	// Default configuration
	return &Config{
		RPC: rpc.Config{
			Endpoints: []string{
				"wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY",
				"wss://mainnet.infura.io/ws/v3/YOUR_KEY",
			},
			MaxConns:            10,
			RequestTimeout:      5 * time.Second,
			ReconnectDelay:      time.Second,
			HealthCheckInterval: 30 * time.Second,
		},
		Mempool: mempool.Config{
			BufferSize:      10000,
			FilterEnabled:   true,
			MinValue:        1e17,                                 // 0.1 ETH
			TargetSelectors: []string{"0x38ed1739", "0x414bf389"}, // UniV2, UniV3
		},
		Relay: relay.Config{
			FlashbotsURL:  "https://relay.flashbots.net",
			BloXrouteURL:  "https://mev.api.blxrbdn.com",
			SigningKey:    os.Getenv("FLASHBOTS_SIGNING_KEY"),
			MaxRetries:    3,
			SubmitTimeout: 2 * time.Second,
		},
	}, nil
}
