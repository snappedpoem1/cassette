use crate::library::error::{ManagerError, Result};
use crate::library::manager::LibraryManager;
use sqlx::SqliteConnection;
use std::future::Future;
use std::pin::Pin;

impl LibraryManager {
    pub async fn execute_atomic<T, F>(&self, operation_id: &str, f: F) -> Result<T>
    where
        T: Send,
        F: for<'a> FnOnce(
            &'a mut SqliteConnection,
        ) -> Pin<
            Box<dyn Future<Output = std::result::Result<T, sqlx::Error>> + Send + 'a>,
        >,
    {
        let mut tx = self.db_pool.begin().await?;
        let result = {
            let conn = tx.as_mut();
            f(conn).await
        };

        match result {
            Ok(result) => {
                tx.commit().await?;
                tracing::debug!(operation_id = operation_id, "Transaction committed");
                Ok(result)
            }
            Err(error) => {
                tx.rollback().await?;
                tracing::warn!(operation_id = operation_id, error = %error, "Transaction rolled back");
                Err(ManagerError::TransactionFailed(error.to_string()))
            }
        }
    }
}
