//! # ingest-cli
//!
//! Command-line interface for the Inngest durable functions platform.
//!
//! This crate provides comprehensive CLI tools for:
//! - Project initialization and scaffolding
//! - Local development server with hot reload
//! - Function deployment and management
//! - Configuration management
//! - Event sending and monitoring
//!
//! ## Usage
//!
//! ```bash
//! # Initialize a new project
//! ingest init my-project
//!
//! # Start development server
//! ingest dev
//!
//! # Deploy functions
//! ingest deploy
//!
//! # List functions
//! ingest functions list
//!
//! # Send an event
//! ingest events send app/hello '{"name": "World"}'
//! ```

pub mod cli;
pub mod config;
pub mod deploy;
pub mod dev;
pub mod utils;

pub use config::ProjectConfig;
pub use deploy::DeploymentPipeline;
pub use dev::DevServer;
