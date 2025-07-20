use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{StateManager, StateManagerV2, StateError, RunMetadata, RunState, StepState, StateId, RunStatus, MutableConfig, RunStats};

/// In-memory state manager for development
#[derive(Clone)]
pub struct MemoryStateManager {
    runs: Arc<RwLock<HashMap<StateId, RunState>>>,
    events: Arc<RwLock<HashMap<StateId, Vec<serde_json::Value>>>>,
}

impl MemoryStateManager {
    pub fn new() -> Self {
        Self {
            runs: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl StateManager for MemoryStateManager {
    async fn create_run(&self, metadata: &RunMetadata) -> Result<(), StateError> {
        let mut runs = self.runs.write().await;
        
        if runs.contains_key(&metadata.id) {
            return Err(StateError::IdentifierExists { id: metadata.id.clone() });
        }
        
        let run_state = RunState {
            metadata: metadata.clone(),
            steps: Vec::new(),
            stack: Vec::new(),
            ctx: None,
        };
        
        runs.insert(metadata.id.clone(), run_state);
        Ok(())
    }
    
    async fn load_run(&self, id: &StateId) -> Result<RunState, StateError> {
        let runs = self.runs.read().await;
        runs.get(id)
            .cloned()
            .ok_or_else(|| StateError::RunNotFound { id: id.clone() })
    }
    
    async fn update_run_metadata(&self, metadata: &RunMetadata) -> Result<(), StateError> {
        let mut runs = self.runs.write().await;
        
        if let Some(run_state) = runs.get_mut(&metadata.id) {
            run_state.metadata = metadata.clone();
            Ok(())
        } else {
            Err(StateError::RunNotFound { id: metadata.id.clone() })
        }
    }
    
    async fn update_run_status(&self, id: &StateId, status: RunStatus) -> Result<(), StateError> {
        let mut runs = self.runs.write().await;
        
        if let Some(run_state) = runs.get_mut(id) {
            run_state.metadata.status = status;
            Ok(())
        } else {
            Err(StateError::RunNotFound { id: id.clone() })
        }
    }
    
    async fn save_step(&self, run_id: &StateId, step: &StepState) -> Result<(), StateError> {
        let mut runs = self.runs.write().await;
        
        if let Some(run_state) = runs.get_mut(run_id) {
            // Update existing step or add new one
            if let Some(existing_step) = run_state.steps.iter_mut().find(|s| s.id == step.id) {
                *existing_step = step.clone();
            } else {
                run_state.steps.push(step.clone());
            }
            Ok(())
        } else {
            Err(StateError::RunNotFound { id: run_id.clone() })
        }
    }
    
    async fn load_step(&self, run_id: &StateId, step_id: &str) -> Result<StepState, StateError> {
        let runs = self.runs.read().await;
        
        if let Some(run_state) = runs.get(run_id) {
            run_state.steps
                .iter()
                .find(|s| s.id == step_id)
                .cloned()
                .ok_or_else(|| StateError::StepNotFound { 
                    step_id: step_id.to_string(), 
                    run_id: run_id.clone() 
                })
        } else {
            Err(StateError::RunNotFound { id: run_id.clone() })
        }
    }
    
    async fn load_events(&self, id: &StateId) -> Result<Vec<serde_json::Value>, StateError> {
        let events = self.events.read().await;
        Ok(events.get(id).cloned().unwrap_or_default())
    }
    
    async fn save_events(&self, id: &StateId, events: &[serde_json::Value]) -> Result<(), StateError> {
        let mut event_store = self.events.write().await;
        event_store.insert(id.clone(), events.to_vec());
        Ok(())
    }
    
    async fn delete_run(&self, id: &StateId) -> Result<(), StateError> {
        let mut runs = self.runs.write().await;
        let mut events = self.events.write().await;
        
        runs.remove(id);
        events.remove(id);
        Ok(())
    }
    
    async fn run_exists(&self, id: &StateId) -> Result<bool, StateError> {
        let runs = self.runs.read().await;
        Ok(runs.contains_key(id))
    }
    
    async fn list_runs_by_function(&self, function_id: Uuid) -> Result<Vec<RunMetadata>, StateError> {
        let runs = self.runs.read().await;
        let matching_runs: Vec<RunMetadata> = runs
            .values()
            .filter(|run| run.metadata.function_id == function_id)
            .map(|run| run.metadata.clone())
            .collect();
        Ok(matching_runs)
    }
    
    async fn list_active_runs(&self) -> Result<Vec<RunMetadata>, StateError> {
        let runs = self.runs.read().await;
        let active_runs: Vec<RunMetadata> = runs
            .values()
            .filter(|run| matches!(run.metadata.status, RunStatus::Queued | RunStatus::Running | RunStatus::Paused))
            .map(|run| run.metadata.clone())
            .collect();
        Ok(active_runs)
    }
}

#[async_trait]
impl StateManagerV2 for MemoryStateManager {
    async fn update_metadata(&self, id: &StateId, config: MutableConfig) -> Result<(), StateError> {
        let mut runs = self.runs.write().await;
        
        if let Some(run_state) = runs.get_mut(id) {
            if let Some(started_at) = config.started_at {
                run_state.metadata.started_at = Some(started_at);
            }
            Ok(())
        } else {
            Err(StateError::RunNotFound { id: id.clone() })
        }
    }
    
    async fn cas_run_status(
        &self,
        id: &StateId,
        expected: RunStatus,
        new: RunStatus,
    ) -> Result<bool, StateError> {
        let mut runs = self.runs.write().await;
        
        if let Some(run_state) = runs.get_mut(id) {
            if run_state.metadata.status == expected {
                run_state.metadata.status = new;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Err(StateError::RunNotFound { id: id.clone() })
        }
    }
    
    async fn get_run_stats(&self, id: &StateId) -> Result<RunStats, StateError> {
        let runs = self.runs.read().await;
        
        if let Some(run_state) = runs.get(id) {
            let total_steps = run_state.steps.len();
            let completed_steps = run_state.steps.iter()
                .filter(|step| step.status == inngest_core::StepStatus::Completed)
                .count();
            let failed_steps = run_state.steps.iter()
                .filter(|step| step.status == inngest_core::StepStatus::Failed)
                .count();
            let pending_steps = run_state.steps.iter()
                .filter(|step| step.status == inngest_core::StepStatus::Pending)
                .count();
            
            let total_duration = if let (Some(start), Some(end)) = 
                (run_state.metadata.started_at, run_state.metadata.ended_at) {
                Some(end - start)
            } else {
                None
            };
            
            // Calculate state size (rough estimate)
            let state_size_bytes = serde_json::to_vec(run_state)
                .map(|bytes| bytes.len())
                .unwrap_or(0);
            
            Ok(RunStats {
                total_steps,
                completed_steps,
                failed_steps,
                pending_steps,
                total_duration,
                state_size_bytes,
            })
        } else {
            Err(StateError::RunNotFound { id: id.clone() })
        }
    }
}
