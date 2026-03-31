// Package strategy connects the Go network layer to the Rust MEV core via gRPC.
//
// The network layer handles mempool monitoring, tx classification, and gas oracle.
// Classified transactions are forwarded to the Rust core for MEV detection,
// simulation, and bundle construction.
package strategy

import (
	"context"
	"encoding/hex"
	"fmt"
	"sync"
	"sync/atomic"
	"time"

	pb "github.com/mev-protocol/network/internal/strategy/proto"
	"github.com/rs/zerolog/log"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/keepalive"
)

// Client connects to the Rust MEV core gRPC server.
type Client struct {
	conn   *grpc.ClientConn
	client pb.MevEngineClient
	addr   string

	// Counters
	sent     atomic.Uint64
	received atomic.Uint64
	errors   atomic.Uint64

	mu      sync.RWMutex
	running bool
}

// Config for the gRPC strategy client.
type Config struct {
	Address        string        // e.g. "127.0.0.1:50051"
	ConnectTimeout time.Duration // default 5s
	RequestTimeout time.Duration // default 100ms — must be fast for MEV
}

func DefaultConfig() Config {
	return Config{
		Address:        "127.0.0.1:50051",
		ConnectTimeout: 5 * time.Second,
		RequestTimeout: 100 * time.Millisecond,
	}
}

// NewClient creates a gRPC client to the Rust core.
func NewClient(cfg Config) (*Client, error) {
	ctx, cancel := context.WithTimeout(context.Background(), cfg.ConnectTimeout)
	defer cancel()

	conn, err := grpc.DialContext(ctx, cfg.Address,
		grpc.WithBlock(),
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithKeepaliveParams(keepalive.ClientParameters{
			Time:                10 * time.Second,
			Timeout:             3 * time.Second,
			PermitWithoutStream: true,
		}),
		grpc.WithDefaultCallOptions(
			grpc.MaxCallRecvMsgSize(4*1024*1024), // 4 MB
		),
	)
	if err != nil {
		return nil, fmt.Errorf("grpc dial %s: %w", cfg.Address, err)
	}

	return &Client{
		conn:    conn,
		client:  pb.NewMevEngineClient(conn),
		addr:    cfg.Address,
		running: true,
	}, nil
}

// DetectOpportunity sends a classified transaction to the Rust core and
// returns any detected MEV opportunities with pre-built bundles.
func (c *Client) DetectOpportunity(
	ctx context.Context,
	txHash []byte,
	from []byte,
	to []byte,
	value uint64,
	gasPrice uint64,
	gasLimit uint64,
	input []byte,
	nonce uint64,
	txClass int32,
	targetBlock uint64,
	baseFee []byte,
) (*pb.DetectionResult, error) {

	c.sent.Add(1)

	req := &pb.ClassifiedTransaction{
		TxHash:      txHash,
		From:        from,
		To:          to,
		Value:       uint64ToBytes(value),
		GasPrice:    gasPrice,
		GasLimit:    gasLimit,
		Input:       input,
		Nonce:       nonce,
		TxClass:     pb.TxClass(txClass),
		TargetBlock: targetBlock,
		BaseFee:     baseFee,
	}

	result, err := c.client.DetectOpportunity(ctx, req)
	if err != nil {
		c.errors.Add(1)
		return nil, fmt.Errorf("detect: %w", err)
	}

	if result.Found {
		c.received.Add(uint64(len(result.Opportunities)))
	}

	return result, nil
}

// GetStatus queries the Rust engine health.
func (c *Client) GetStatus(ctx context.Context) (*pb.StatusResponse, error) {
	return c.client.GetStatus(ctx, &pb.StatusRequest{})
}

// Stats returns client-side counters.
func (c *Client) Stats() (sent, received, errors uint64) {
	return c.sent.Load(), c.received.Load(), c.errors.Load()
}

// Close shuts down the connection.
func (c *Client) Close() error {
	c.mu.Lock()
	c.running = false
	c.mu.Unlock()

	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}

// LogOpportunities logs detected opportunities from a detection result.
func LogOpportunities(result *pb.DetectionResult, txHash string) {
	if !result.Found {
		return
	}
	for _, opp := range result.Opportunities {
		oppType := "unknown"
		switch opp.Type {
		case pb.OpportunityType_ARBITRAGE:
			oppType = "arbitrage"
		case pb.OpportunityType_BACKRUN:
			oppType = "backrun"
		case pb.OpportunityType_LIQUIDATION_OPP:
			oppType = "liquidation"
		}

		profit := "0"
		if len(opp.ExpectedProfit) > 0 {
			profit = "0x" + hex.EncodeToString(opp.ExpectedProfit)
		}

		log.Info().
			Str("tx", txHash).
			Str("type", oppType).
			Str("tokenIn", opp.TokenIn).
			Str("tokenOut", opp.TokenOut).
			Str("profit", profit).
			Uint64("gas", opp.GasEstimate).
			Uint64("latencyNs", result.DetectionLatencyNs).
			Msg("MEV opportunity detected")
	}
}

func uint64ToBytes(v uint64) []byte {
	b := make([]byte, 8)
	b[0] = byte(v >> 56)
	b[1] = byte(v >> 48)
	b[2] = byte(v >> 40)
	b[3] = byte(v >> 32)
	b[4] = byte(v >> 24)
	b[5] = byte(v >> 16)
	b[6] = byte(v >> 8)
	b[7] = byte(v)
	return b
}
