package main

import (
	"context"
	"crypto/ecdsa"
	"fmt"
	"math/big"
	"strings"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/common/hexutil"
	gethtypes "github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/crypto"
	"github.com/mev-protocol/network/internal/gas"
	"github.com/mev-protocol/network/internal/relay"
	"github.com/mev-protocol/network/internal/rpc"
	pb "github.com/mev-protocol/network/internal/strategy/proto"
	"github.com/mev-protocol/network/pkg/config"
	"github.com/rs/zerolog/log"
)

type bundleExecutor struct {
	cfg      *config.Node
	rpcPool  *rpc.Pool
	relayMgr *relay.Manager
	txKey    *ecdsa.PrivateKey
	sender   common.Address
}

func newBundleExecutor(cfg *config.Node, rpcPool *rpc.Pool, relayMgr *relay.Manager) (*bundleExecutor, error) {
	executor := &bundleExecutor{
		cfg:      cfg,
		rpcPool:  rpcPool,
		relayMgr: relayMgr,
	}

	if !cfg.Execution.Live() {
		return executor, nil
	}

	key, err := crypto.HexToECDSA(strings.TrimPrefix(cfg.Execution.PrivateKey, "0x"))
	if err != nil {
		return nil, fmt.Errorf("parse PRIVATE_KEY: %w", err)
	}

	executor.txKey = key
	executor.sender = crypto.PubkeyToAddress(key.PublicKey)
	return executor, nil
}

func (e *bundleExecutor) HandleOpportunityBundle(
	ctx context.Context,
	opp *pb.Opportunity,
	fallbackTarget uint64,
	estimate *gas.Estimate,
) error {
	if opp == nil || opp.Bundle == nil || len(opp.Bundle.Transactions) == 0 {
		return nil
	}

	expectedProfit := bytesToBigInt(opp.ExpectedProfit)
	if minProfit := big.NewInt(e.cfg.Multi.MinProfitWei); minProfit.Sign() > 0 && expectedProfit.Cmp(minProfit) < 0 {
		log.Debug().
			Str("profitWei", expectedProfit.String()).
			Str("minProfitWei", minProfit.String()).
			Msg("Skipping bundle below configured min profit")
		return nil
	}

	if !e.cfg.Execution.Live() {
		return nil
	}

	bundle, err := e.signBundle(ctx, opp.Bundle, fallbackTarget, estimate)
	if err != nil {
		return err
	}

	if e.cfg.Multi.RequireSimulation {
		relayName, simResult, err := e.relayMgr.SimulateBundle(ctx, bundle)
		if err != nil {
			return fmt.Errorf("relay preflight simulation: %w", err)
		}
		if err := validateSimulation(simResult); err != nil {
			return fmt.Errorf("relay simulation rejected on %s: %w", relayName, err)
		}
		log.Info().
			Str("relay", string(relayName)).
			Uint64("gasUsed", simResult.TotalGasUsed).
			Msg("Relay preflight simulation passed")
	}

	results, err := e.relayMgr.SubmitBundle(ctx, bundle)
	if err != nil {
		return fmt.Errorf("relay submit: %w", err)
	}

	accepted := 0
	for _, result := range results {
		if result.Error == nil {
			accepted++
		}
	}

	log.Info().
		Int("accepted", accepted).
		Int("attempted", len(results)).
		Uint64("targetBlock", opp.Bundle.TargetBlock).
		Msg("Bundle submitted to relay manager")

	return nil
}

func (e *bundleExecutor) signBundle(
	ctx context.Context,
	bundle *pb.Bundle,
	fallbackTarget uint64,
	estimate *gas.Estimate,
) (*relay.Bundle, error) {
	if e.txKey == nil {
		return nil, fmt.Errorf("execution key not configured")
	}

	client, err := e.rpcPool.GetClient()
	if err != nil {
		return nil, fmt.Errorf("rpc client: %w", err)
	}

	pendingNonce, err := client.PendingNonceAt(ctx, e.sender)
	if err != nil {
		return nil, fmt.Errorf("pending nonce: %w", err)
	}

	targetBlock := bundle.TargetBlock
	if targetBlock == 0 {
		targetBlock = fallbackTarget
	}
	if targetBlock == 0 {
		blockNumber, err := client.BlockNumber(ctx)
		if err != nil {
			return nil, fmt.Errorf("resolve target block: %w", err)
		}
		targetBlock = blockNumber + 1
	}

	signer := gethtypes.LatestSignerForChainID(new(big.Int).SetUint64(e.cfg.Execution.ChainID))
	rawTxs := make([]string, 0, len(bundle.Transactions))
	for index, tx := range bundle.Transactions {
		rawTx, err := e.signBundleTx(tx, pendingNonce+uint64(index), signer, estimate)
		if err != nil {
			return nil, fmt.Errorf("sign bundle tx %d: %w", index, err)
		}
		rawTxs = append(rawTxs, rawTx)
	}

	return &relay.Bundle{
		Txs:         rawTxs,
		BlockNumber: fmt.Sprintf("0x%x", targetBlock),
	}, nil
}

func (e *bundleExecutor) signBundleTx(
	tx *pb.BundleTx,
	nonce uint64,
	signer gethtypes.Signer,
	estimate *gas.Estimate,
) (string, error) {
	if tx == nil {
		return "", fmt.Errorf("bundle tx is nil")
	}
	if len(tx.To) != common.AddressLength {
		return "", fmt.Errorf("bundle tx to must be %d bytes, got %d", common.AddressLength, len(tx.To))
	}
	if tx.GasLimit == 0 {
		return "", fmt.Errorf("bundle tx gas limit must be non-zero")
	}

	maxFee, maxPriority := resolveFeeCaps(tx, estimate)
	to := common.BytesToAddress(tx.To)
	value := bytesToBigInt(tx.Value)

	dynamicTx := &gethtypes.DynamicFeeTx{
		ChainID:   new(big.Int).SetUint64(e.cfg.Execution.ChainID),
		Nonce:     nonce,
		GasTipCap: maxPriority,
		GasFeeCap: maxFee,
		Gas:       tx.GasLimit,
		To:        &to,
		Value:     value,
		Data:      tx.Data,
	}

	signedTx, err := gethtypes.SignNewTx(e.txKey, signer, dynamicTx)
	if err != nil {
		return "", err
	}

	rawTx, err := signedTx.MarshalBinary()
	if err != nil {
		return "", err
	}

	return hexutil.Encode(rawTx), nil
}

func resolveFeeCaps(tx *pb.BundleTx, estimate *gas.Estimate) (*big.Int, *big.Int) {
	priorityFee := bytesToBigInt(tx.MaxPriorityFeePerGas)
	if priorityFee.Sign() == 0 {
		if estimate != nil && estimate.SuggestedPriority != nil {
			priorityFee = new(big.Int).Set(estimate.SuggestedPriority)
		} else {
			priorityFee = big.NewInt(2_000_000_000)
		}
	}

	maxFee := bytesToBigInt(tx.MaxFeePerGas)
	if maxFee.Sign() == 0 {
		if estimate != nil && estimate.MaxFeePerGas != nil {
			maxFee = new(big.Int).Set(estimate.MaxFeePerGas)
		} else {
			baseFee := big.NewInt(1_000_000_000)
			if estimate != nil {
				switch {
				case estimate.PredictedBaseFee != nil:
					baseFee = new(big.Int).Set(estimate.PredictedBaseFee)
				case estimate.BaseFee != nil:
					baseFee = new(big.Int).Set(estimate.BaseFee)
				}
			}

			maxFee = new(big.Int).Mul(baseFee, big.NewInt(2))
			maxFee.Add(maxFee, priorityFee)
		}
	}

	if maxFee.Cmp(priorityFee) < 0 {
		maxFee = new(big.Int).Set(priorityFee)
	}

	return maxFee, priorityFee
}

func validateSimulation(result *relay.SimulationResult) error {
	if result == nil {
		return fmt.Errorf("empty simulation result")
	}
	for index, tx := range result.Results {
		if tx.Error != "" {
			return fmt.Errorf("tx %d simulation error: %s", index, tx.Error)
		}
		if tx.Revert != "" {
			return fmt.Errorf("tx %d simulation revert: %s", index, tx.Revert)
		}
	}
	return nil
}

func bytesToBigInt(value []byte) *big.Int {
	if len(value) == 0 {
		return big.NewInt(0)
	}
	return new(big.Int).SetBytes(value)
}
