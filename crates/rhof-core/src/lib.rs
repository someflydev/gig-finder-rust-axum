//! Core domain model and provenance types for RHOF.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CRATE_NAME: &str = "rhof-core";

/// Provenance pointer attached to canonical extracted values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub raw_artifact_id: Uuid,
    pub source_url: String,
    pub selector_or_pointer: String,
    pub snippet: String,
    pub fetched_at: DateTime<Utc>,
    pub extractor_version: String,
}

/// Canonical field wrapper with optional value + evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Field<T> {
    pub value: Option<T>,
    pub evidence: Option<EvidenceRef>,
}

impl<T> Field<T> {
    pub fn empty() -> Self {
        Self {
            value: None,
            evidence: None,
        }
    }

    pub fn with_value_and_evidence(value: T, evidence: EvidenceRef) -> Self {
        Self {
            value: Some(value),
            evidence: Some(evidence),
        }
    }
}

/// Parsed/pre-normalized handoff contract from adapters into the sync pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpportunityDraft {
    pub source_id: String,
    pub listing_url: Option<String>,
    pub detail_url: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub extractor_version: String,
    pub title: Field<String>,
    pub description: Field<String>,
    pub pay_model: Field<String>,
    pub pay_rate_min: Field<f64>,
    pub pay_rate_max: Field<f64>,
    pub currency: Field<String>,
    pub min_hours_per_week: Field<f64>,
    pub verification_requirements: Field<String>,
    pub geo_constraints: Field<String>,
    pub one_off_vs_ongoing: Field<String>,
    pub payment_methods: Field<Vec<String>>,
    pub apply_url: Field<String>,
    pub requirements: Field<Vec<String>>,
}

/// Canonical persisted opportunity representation with provenance-bearing fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Opportunity {
    pub id: Uuid,
    pub source_id: String,
    pub canonical_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub title: Field<String>,
    pub description: Field<String>,
    pub pay_model: Field<String>,
    pub pay_rate_min: Field<f64>,
    pub pay_rate_max: Field<f64>,
    pub currency: Field<String>,
    pub min_hours_per_week: Field<f64>,
    pub verification_requirements: Field<String>,
    pub geo_constraints: Field<String>,
    pub one_off_vs_ongoing: Field<String>,
    pub payment_methods: Field<Vec<String>>,
    pub apply_url: Field<String>,
    pub requirements: Field<Vec<String>>,
}
