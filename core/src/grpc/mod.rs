//! gRPC server — exposes MEV engine to the Go network layer
//!
//! Implements the MevEngine service defined in proto/mev.proto.
//! The Go network layer streams classified pending transactions,
//! and this server returns detected opportunities with pre-built bundles.

pub mod server;

// Include the generated protobuf code
// (tonic_build outputs to src/grpc/ via build.rs)
pub mod mev {
    include!("mev.rs");
}

pub use server::MevGrpcServer;
