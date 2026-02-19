//! Error types for the chess client

use thiserror::Error;

pub type ClientResult<T> = Result<T, ClientError>;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Invalid server address: {0}")]
    InvalidAddress(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(#[from] tonic::transport::Error),

    #[error("RPC failed: {0}")]
    RpcError(#[from] tonic::Status),

    #[error("No active session")]
    NoActiveSession,

    #[error("Server returned invalid data: {0}")]
    InvalidData(String),

    #[error("Mock response not configured for: {0}")]
    NotConfigured(String),
}
