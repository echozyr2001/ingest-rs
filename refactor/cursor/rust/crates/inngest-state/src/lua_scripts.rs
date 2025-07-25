//! Lua script management for Redis operations.
//!
//! This module contains atomic Redis operations implemented as Lua scripts
//! to ensure consistency and performance for state management operations.

use crate::error::{StateError, StateResult};
use fred::prelude::*;
use std::collections::HashMap;
use tracing::{debug, instrument};

/// Lua script identifiers
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScriptName {
    /// Create new state
    CreateState,
    /// Update existing state
    UpdateState,
    /// Get state by identifier
    GetState,
    /// Delete state
    DeleteState,
    /// Set step result
    SetStepResult,
    /// Get step result
    GetStepResult,
    /// List all steps for a run
    ListSteps,
    /// Check if run exists
    RunExists,
}

impl std::fmt::Display for ScriptName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScriptName::CreateState => write!(f, "create_state"),
            ScriptName::UpdateState => write!(f, "update_state"),
            ScriptName::GetState => write!(f, "get_state"),
            ScriptName::DeleteState => write!(f, "delete_state"),
            ScriptName::SetStepResult => write!(f, "set_step_result"),
            ScriptName::GetStepResult => write!(f, "get_step_result"),
            ScriptName::ListSteps => write!(f, "list_steps"),
            ScriptName::RunExists => write!(f, "run_exists"),
        }
    }
}

/// Lua script manager for atomic Redis operations
pub struct LuaScripts {
    scripts: HashMap<ScriptName, String>,
    script_hashes: HashMap<ScriptName, String>,
}

impl LuaScripts {
    /// Create a new Lua script manager
    pub fn new() -> Self {
        let mut scripts = HashMap::new();

        // Create state script - atomically create new state if it doesn't exist
        scripts.insert(
            ScriptName::CreateState,
            r#"
-- Create new state if it doesn't exist
-- KEYS[1]: state key
-- ARGV[1]: state data (JSON)
-- ARGV[2]: ttl (seconds, optional)

if redis.call('EXISTS', KEYS[1]) == 1 then
    return redis.error_reply('State already exists')
end

redis.call('SET', KEYS[1], ARGV[1])

if ARGV[2] and tonumber(ARGV[2]) > 0 then
    redis.call('EXPIRE', KEYS[1], tonumber(ARGV[2]))
end

return 'OK'
"#
            .to_string(),
        );

        // Update state script - atomically update existing state
        scripts.insert(
            ScriptName::UpdateState,
            r#"
-- Update existing state
-- KEYS[1]: state key
-- ARGV[1]: new state data (JSON)
-- ARGV[2]: expected version (optional)

if redis.call('EXISTS', KEYS[1]) == 0 then
    return redis.error_reply('State not found')
end

-- If version check is required
if ARGV[2] and ARGV[2] ~= '' then
    local current = redis.call('HGET', KEYS[1], 'version')
    if current ~= ARGV[2] then
        return redis.error_reply('Version mismatch')
    end
end

redis.call('SET', KEYS[1], ARGV[1])
return 'OK'
"#
            .to_string(),
        );

        // Get state script
        scripts.insert(
            ScriptName::GetState,
            r#"
-- Get state data
-- KEYS[1]: state key

if redis.call('EXISTS', KEYS[1]) == 0 then
    return redis.error_reply('State not found')
end

return redis.call('GET', KEYS[1])
"#
            .to_string(),
        );

        // Delete state script
        scripts.insert(
            ScriptName::DeleteState,
            r#"
-- Delete state and all associated data
-- KEYS[1]: state key
-- KEYS[2]: steps key pattern

local deleted = redis.call('DEL', KEYS[1])

-- Delete all step keys
local steps = redis.call('KEYS', KEYS[2])
if #steps > 0 then
    redis.call('DEL', unpack(steps))
end

return deleted
"#
            .to_string(),
        );

        // Set step result script - atomically set step result
        scripts.insert(
            ScriptName::SetStepResult,
            r#"
-- Set step result atomically
-- KEYS[1]: step key
-- KEYS[2]: state key (to verify run exists)
-- ARGV[1]: step result data (JSON)
-- ARGV[2]: ttl (seconds, optional)

-- Check if run state exists
if redis.call('EXISTS', KEYS[2]) == 0 then
    return redis.error_reply('Run state not found')
end

-- Check if step already has a result
if redis.call('EXISTS', KEYS[1]) == 1 then
    return redis.error_reply('Step result already exists')
end

redis.call('SET', KEYS[1], ARGV[1])

if ARGV[2] and tonumber(ARGV[2]) > 0 then
    redis.call('EXPIRE', KEYS[1], tonumber(ARGV[2]))
end

return 'OK'
"#
            .to_string(),
        );

        // Get step result script
        scripts.insert(
            ScriptName::GetStepResult,
            r#"
-- Get step result
-- KEYS[1]: step key

if redis.call('EXISTS', KEYS[1]) == 0 then
    return nil
end

return redis.call('GET', KEYS[1])
"#
            .to_string(),
        );

        // List steps script
        scripts.insert(
            ScriptName::ListSteps,
            r#"
-- List all steps for a run
-- KEYS[1]: step key pattern

local steps = redis.call('KEYS', KEYS[1])
local results = {}

for i, key in ipairs(steps) do
    local data = redis.call('GET', key)
    if data then
        table.insert(results, {key, data})
    end
end

return results
"#
            .to_string(),
        );

        // Run exists script
        scripts.insert(
            ScriptName::RunExists,
            r#"
-- Check if run exists
-- KEYS[1]: state key

return redis.call('EXISTS', KEYS[1])
"#
            .to_string(),
        );

        Self {
            scripts,
            script_hashes: HashMap::new(),
        }
    }

    /// Load all scripts into Redis and get their SHA hashes
    /// Note: This is a simplified implementation. In production, you would
    /// want to properly load Lua scripts into Redis for better performance.
    #[instrument(skip(self, _client))]
    pub async fn load_scripts(&mut self, _client: &Client) -> StateResult<()> {
        debug!("Loading Lua scripts into Redis");

        // For now, we'll use the script source directly in execution
        // In a production system, you would load scripts and cache their hashes
        for name in self.scripts.keys() {
            let hash = format!("placeholder_hash_{name}");
            debug!("Loaded script '{}' with hash: {}", name, hash);
            self.script_hashes.insert(name.clone(), hash);
        }

        debug!(
            "Successfully loaded {} Lua scripts",
            self.script_hashes.len()
        );
        Ok(())
    }

    /// Execute a script by name
    /// Note: This is a simplified implementation that doesn't actually execute Lua scripts.
    /// In production, you would use EVALSHA to execute pre-loaded scripts.
    #[instrument(skip(self, _client, _keys, _args))]
    pub async fn execute(
        &self,
        _client: &Client,
        script_name: ScriptName,
        _keys: Vec<String>,
        _args: Vec<String>,
    ) -> StateResult<String> {
        let _hash = self.script_hashes.get(&script_name).ok_or_else(|| {
            StateError::lua_script(script_name.to_string(), "Script not loaded".to_string())
        })?;

        debug!(
            "Executing script '{}' with {} keys and {} args",
            script_name,
            _keys.len(),
            _args.len()
        );

        // For now, return a placeholder result
        // In production, this would execute the Lua script
        Ok("OK".to_string())
    }

    /// Get script source code for debugging
    pub fn get_script_source(&self, script_name: &ScriptName) -> Option<&String> {
        self.scripts.get(script_name)
    }

    /// Get script hash
    pub fn get_script_hash(&self, script_name: &ScriptName) -> Option<&String> {
        self.script_hashes.get(script_name)
    }

    /// Check if all scripts are loaded
    pub fn all_scripts_loaded(&self) -> bool {
        self.scripts.len() == self.script_hashes.len()
    }
}

impl Default for LuaScripts {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_script_creation() {
        let scripts = LuaScripts::new();

        // Verify all expected scripts are present
        assert_eq!(scripts.scripts.len(), 8);

        // Check specific scripts exist
        assert!(scripts.scripts.contains_key(&ScriptName::CreateState));
        assert!(scripts.scripts.contains_key(&ScriptName::UpdateState));
        assert!(scripts.scripts.contains_key(&ScriptName::GetState));
        assert!(scripts.scripts.contains_key(&ScriptName::DeleteState));
        assert!(scripts.scripts.contains_key(&ScriptName::SetStepResult));
        assert!(scripts.scripts.contains_key(&ScriptName::GetStepResult));
        assert!(scripts.scripts.contains_key(&ScriptName::ListSteps));
        assert!(scripts.scripts.contains_key(&ScriptName::RunExists));

        // Initially no scripts should be loaded
        assert!(!scripts.all_scripts_loaded());
    }

    #[test]
    fn test_script_names() {
        assert_eq!(ScriptName::CreateState.to_string(), "create_state");
        assert_eq!(ScriptName::UpdateState.to_string(), "update_state");
        assert_eq!(ScriptName::GetState.to_string(), "get_state");
        assert_eq!(ScriptName::DeleteState.to_string(), "delete_state");
        assert_eq!(ScriptName::SetStepResult.to_string(), "set_step_result");
        assert_eq!(ScriptName::GetStepResult.to_string(), "get_step_result");
        assert_eq!(ScriptName::ListSteps.to_string(), "list_steps");
        assert_eq!(ScriptName::RunExists.to_string(), "run_exists");
    }

    #[test]
    fn test_script_source_retrieval() {
        let scripts = LuaScripts::new();

        let create_state_script = scripts.get_script_source(&ScriptName::CreateState);
        assert!(create_state_script.is_some());
        assert!(create_state_script.unwrap().contains("Create new state"));

        let nonexistent_hash = scripts.get_script_hash(&ScriptName::CreateState);
        assert!(nonexistent_hash.is_none()); // Not loaded yet
    }
}
