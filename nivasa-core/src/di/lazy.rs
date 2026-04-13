use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::OnceCell;

use crate::di::error::DiError;

type LazyFuture<T> = Pin<Box<dyn Future<Output = Result<T, DiError>> + Send>>;
type LazyResolver<T> = dyn Fn() -> LazyFuture<T> + Send + Sync;

/// Lazy dependency wrapper.
///
/// Resolve only on first `get()`. Useful for breaking circular provider chains.
///
/// # Examples
///
/// ```rust
/// # use nivasa_core::di::Lazy;
/// # let rt = tokio::runtime::Runtime::new().unwrap();
/// # rt.block_on(async {
/// let lazy = Lazy::new(|| async { Ok::<_, nivasa_core::DiError>(7_u32) });
/// assert_eq!(lazy.get().await.unwrap(), 7);
/// # });
/// ```
pub struct Lazy<T: Clone + Send + Sync + 'static> {
    resolver: Arc<LazyResolver<T>>,
    cell: OnceCell<T>,
}

impl<T: Clone + Send + Sync + 'static> Lazy<T> {
    /// Build lazy resolver.
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

    /// Resolve and clone cached value.
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
