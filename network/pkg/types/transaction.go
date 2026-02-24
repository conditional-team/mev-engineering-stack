package types

import (
	"github.com/ethereum/go-ethereum/common"
)

// Opportunity represents a MEV opportunity
type Opportunity struct {
	Type           OpportunityType
	TokenIn        common.Address
	TokenOut       common.Address
	AmountIn       uint64
	ExpectedProfit uint64
	GasEstimate    uint64
	Deadline       uint64
	Path           []DexType
	TargetTxHash   *common.Hash
}

// OpportunityType enum
type OpportunityType int

const (
	Arbitrage OpportunityType = iota
	Backrun
	Liquidation
	Sandwich
)

// DexType enum
type DexType int

const (
	UniswapV2 DexType = iota
	UniswapV3
	SushiSwap
	Curve
	Balancer
)

// PendingTx represents a pending transaction
type PendingTx struct {
	Hash      common.Hash
	From      common.Address
	To        *common.Address
	Value     uint64
	GasPrice  uint64
	GasLimit  uint64
	Input     []byte
	Nonce     uint64
	ChainID   uint64
	Timestamp int64
}

// Bundle represents a bundle of transactions
type Bundle struct {
	Transactions      []BundleTx
	TargetBlock       uint64
	MaxBlockNumber    *uint64
	MinTimestamp      *uint64
	MaxTimestamp      *uint64
	RevertingTxHashes []common.Hash
}

// BundleTx represents a transaction in a bundle
type BundleTx struct {
	To                   common.Address
	Value                uint64
	GasLimit             uint64
	GasPrice             *uint64
	MaxFeePerGas         *uint64
	MaxPriorityFeePerGas *uint64
	Data                 []byte
	Nonce                *uint64
}

// SimulationResult from bundle simulation
type SimulationResult struct {
	Success      bool
	Profit       int64
	GasUsed      uint64
	Error        string
	StateChanges []StateChange
}

// StateChange from simulation
type StateChange struct {
	Address  common.Address
	Slot     common.Hash
	OldValue common.Hash
	NewValue common.Hash
}
