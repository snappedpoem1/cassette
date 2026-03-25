use crate::library::error::Result;
use crate::library::manager::LibraryManager;
use crate::library::state::{Module, OperationStatus};

impl LibraryManager {
    pub async fn run_enricher_operation<F, Fut, T>(&self, phase: &str, f: F) -> Result<T>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let operation_id = self.start_operation(Module::Enricher, phase).await?;
        let result = f(operation_id.clone()).await;
        match &result {
            Ok(_) => {
                self.complete_operation(&operation_id, OperationStatus::Success)
                    .await?;
            }
            Err(error) => {
                self.complete_operation(
                    &operation_id,
                    OperationStatus::FailedAt(error.to_string()),
                )
                .await?;
            }
        }
        result
    }
}
