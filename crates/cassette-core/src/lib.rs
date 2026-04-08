//! # Cassette Core
//!
//! Shared Rust domain logic for the Cassette desktop application.
//!
//! ## Canonical Project Docs
//!
//! - `docs/PROJECT_INDEX.md` - project map, status, architecture, and quality gates
//! - `docs/AGENT_CODEX.md` - agent operating manual
//! - `docs/AGENT_BRIEFING.md` - quick onboarding
//! - `docs/DECISIONS.md` - architecture rationale and tradeoffs
//! - `docs/PATTERNS.md` - implementation and testing patterns
//! - `docs/TODO.md` - living task list
//! - `docs/TELEMETRY.md` - build and runtime confidence notes
//!
//! ## Major Modules
//!
//! - `librarian` - scan, import, normalize, and classify library state
//! - `custodian` - organize, validate, quarantine, and stage files safely
//! - `orchestrator` - reconcile desired and local state, then plan work
//! - `director` - acquire media through providers and sources
//! - `gatekeeper` - validate and admit staged arrivals
//! - `library` - operational management, locking, recovery, and observability
//! - `validation` - sandbox validation, logging checks, and operational audit helpers
//! - `metadata` - supporting metadata lookups and tag-fix flows
//!
pub mod acquisition;
pub mod custodian;
pub mod db;
pub mod director;
pub mod gatekeeper;
pub mod identity;
pub mod librarian;
pub mod library;
pub mod metadata;
pub mod models;
pub mod orchestrator;
pub mod player;
pub mod provider_settings;
pub mod sources;
pub mod validation;

pub use anyhow::{anyhow, Result};
