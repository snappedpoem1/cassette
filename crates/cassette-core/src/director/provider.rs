use crate::director::error::ProviderError;
use crate::director::models::{
    CandidateAcquisition, ProviderDescriptor, ProviderHealthState, ProviderSearchCandidate,
    TrackTask,
};
use crate::director::strategy::StrategyPlan;
use crate::director::temp::TaskTempContext;
use async_trait::async_trait;

#[async_trait]
pub trait Provider: Send + Sync {
    fn descriptor(&self) -> ProviderDescriptor;

    async fn health_check(&self) -> Result<ProviderHealthState, ProviderError> {
        Ok(ProviderHealthState {
            provider_id: self.descriptor().id,
            status: crate::director::models::ProviderHealthStatus::Healthy,
            checked_at: chrono::Utc::now(),
            message: None,
        })
    }

    async fn search(
        &self,
        task: &TrackTask,
        strategy: &StrategyPlan,
    ) -> Result<Vec<ProviderSearchCandidate>, ProviderError>;

    async fn acquire(
        &self,
        task: &TrackTask,
        candidate: &ProviderSearchCandidate,
        temp_context: &TaskTempContext,
        strategy: &StrategyPlan,
    ) -> Result<CandidateAcquisition, ProviderError>;
}
