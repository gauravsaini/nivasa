use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::OnceCell;

use crate::di::error::DiError;

type LazyFuture<T> = Pin<Box<dyn Future<Output = Result<T, DiError>> + Send>>;
type LazyResolver<T> = dyn Fn() -> LazyFuture<T> + Send + Sync;

/// A lazy wrapper for a dependency that is resolved only when accessed.
/// This allows breaking circular dependency chains where two providers
/// depend on each other, as long as at least one of them uses `Lazy`.
pub struct Lazy<T: Clone + Send + Sync + 'static> {
    resolver: Arc<LazyResolver<T>>,
    cell: OnceCell<T>,
}

impl<T: Clone + Send + Sync + 'static> Lazy<T> {
    pub fn new<F, Fut>(resolver: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, DiError>> + Send + 'static,
    {
        Self {
            resolver: Arc::new(move || Box::pin(resolver())),
            cell: OnceCell::new(),
        }
    }

    /// Resolves and returns the underlying dependency instance.
    pub async fn get(&self) -> Result<T, DiError> {
        let value = self.cell.get_or_try_init(|| (self.resolver)()).await?;
        Ok(value.clone())
    }
}

// Implement Clone to allow multiple parts of the same provider to share the lazy instance.
impl<T: Clone + Send + Sync + 'static> Clone for Lazy<T> {
    fn clone(&self) -> Self {
        Self {
            resolver: self.resolver.clone(),
            cell: self.cell.clone(),
        }
    }
}
