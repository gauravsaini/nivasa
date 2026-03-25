use std::sync::Arc;
use tokio::sync::OnceCell;
use crate::di::container::DependencyContainer;
use crate::di::error::DiError;

/// A lazy wrapper for a dependency that is resolved only when accessed.
/// This allows breaking circular dependency chains where two providers
/// depend on each other, as long as at least one of them uses `Lazy`.
pub struct Lazy<T: Send + Sync + 'static> {
    container: Arc<DependencyContainer>,
    cell: OnceCell<Arc<T>>,
}

impl<T: Send + Sync + 'static> Lazy<T> {
    pub fn new(container: Arc<DependencyContainer>) -> Self {
        Self {
            container,
            cell: OnceCell::new(),
        }
    }

    /// Resolves and returns the underlying dependency instance.
    pub async fn get(&self) -> Result<Arc<T>, DiError> {
        self.cell.get_or_try_init(|| async {
            self.container.resolve::<T>().await
        }).await.cloned()
    }
}

// Implement Clone to allow multiple parts of the same provider to share the lazy instance.
impl<T: Send + Sync + 'static> Clone for Lazy<T> {
    fn clone(&self) -> Self {
        Self {
            container: self.container.clone(),
            cell: self.cell.clone(),
        }
    }
}
