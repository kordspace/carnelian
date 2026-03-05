//! `Arc<T>` implementations for EntropyProvider trait
//!
//! This module provides EntropyProvider implementations for Arc-wrapped providers
//! to enable calling async trait methods on Arc references.

use async_trait::async_trait;
use std::sync::Arc;

use crate::entropy::{EntropyHealth, EntropyProvider, MixedEntropyProvider};
use crate::error::Result;

#[async_trait]
impl EntropyProvider for Arc<MixedEntropyProvider> {
    fn source_name(&self) -> &str {
        self.as_ref().source_name()
    }

    async fn is_available(&self) -> bool {
        self.as_ref().is_available().await
    }

    async fn get_bytes(&self, n: usize) -> Result<Vec<u8>> {
        self.as_ref().get_bytes(n).await
    }

    async fn health(&self) -> EntropyHealth {
        self.as_ref().health().await
    }
}
