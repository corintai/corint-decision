//! Offline compatibility against declarations, never live attestation or authority.
use crate::package;
use corint_decision_compiler::core::{
    diagnostic, parse_core_input_schema, CoreError, CoreSource, CORE_INPUT_SCHEMA, PROFILE,
};
use corint_decision_engine::{Schema, ENGINE_VERSION};
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const BUSINESS_CONTEXT_SCHEMA: &str =
    include_str!("../../../docs/contracts/schema/business-context.json");
pub const TARGET_CAPABILITIES_SCHEMA: &str =
    include_str!("../../../docs/contracts/schema/target-capabilities.json");
const INVENTORY: &str = include_str!("../../../docs/contracts/schema/capabilities.json");

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub producer: String,
    pub reference: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldMeaning {
    pub description: String,
    pub unit: String,
    pub entity: String,
    pub time_basis: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusinessContext {
    pub kind: String,
    pub contract_version: String,
    pub id: String,
    pub revision: String,
    pub provenance: Provenance,
    pub input_schema: Schema,
    pub entities: BTreeMap<String, String>,
    pub fields: BTreeMap<String, FieldMeaning>,
    pub objectives: Vec<String>,
    pub constraints: Vec<String>,
    pub actions: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineCapabilities {
    pub name: String,
    pub version: String,
    pub profile: String,
    pub language_version: String,
    pub capabilities: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentBinding {
    pub id: String,
    pub revision: String,
    pub sha256: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetCapabilities {
    pub kind: String,
    pub contract_version: String,
    pub id: String,
    pub revision: String,
    pub provenance: Provenance,
    pub engine: EngineCapabilities,
    pub context: DocumentBinding,
    pub status: String,
    pub actions: Vec<String>,
    pub max_sources: usize,
    pub resources: Vec<Value>,
}
#[derive(Debug, Serialize)]
pub struct CompatibilityReport {
    pub report_version: &'static str,
    pub scope: &'static str,
    pub compatible: bool,
    pub policy_sha256: String,
    pub context: DocumentBinding,
    pub target: DocumentBinding,
    pub checker_version: String,
    pub checker_sha256: String,
    pub binding_sha256: String,
    pub execution_checked: bool,
    pub business_semantics_checked: bool,
    pub live_target_verified: bool,
    pub business_evaluation: &'static str,
    pub publication_approval: &'static str,
    pub authenticity: &'static str,
}

/// Only constructed after validation. Callers cannot mutate the validated data
/// while keeping its previous source fingerprint.
pub struct TargetContracts {
    business: BusinessContext,
    target: TargetCapabilities,
    context_path: String,
    target_path: String,
    context_sha256: String,
    target_sha256: String,
}

fn error(source: &str, field: &str, code: &str, message: impl Into<String>) -> CoreError {
    diagnostic(source, field, "compatibility", code, message)
}
fn pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

pub(crate) fn parse(source: &CoreSource, schema: &'static str) -> Result<Value, CoreError> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(&source.yaml).map_err(|e| {
        let mut err = error(&source.path, "", "E_CONTRACT_FORMAT", e.to_string());
        if let Some(pos) = e.location() {
            err.diagnostic.line = Some(pos.line());
            err.diagnostic.column = Some(pos.column());
        }
        err
    })?;
    let value = serde_json::to_value(yaml)
        .map_err(|e| error(&source.path, "", "E_CONTRACT_FORMAT", e.to_string()))?;
    if value.get("contract_version").and_then(Value::as_str) != Some("1") {
        return Err(error(
            &source.path,
            "/contract_version",
            "E_CONTRACT_VERSION",
            "Only explicit contract_version 1 is supported",
        ));
    }
    // Only built-in schemas reach this function; cache compiled validators, never
    // validation outcomes or mutable caller evidence.
    use std::sync::{Arc, Mutex, OnceLock};
    static VALIDATORS: OnceLock<Mutex<BTreeMap<&'static str, Arc<JSONSchema>>>> = OnceLock::new();
    let validator = {
        let mut validators = VALIDATORS
            .get_or_init(Default::default)
            .lock()
            .expect("validator cache");
        validators
            .entry(schema)
            .or_insert_with(|| {
                Arc::new(
                    JSONSchema::options()
                        .with_document(
                            "urn:corint:core-input".into(),
                            serde_json::from_str(CORE_INPUT_SCHEMA).expect("input schema"),
                        )
                        .compile(&serde_json::from_str(schema).expect("contract schema JSON"))
                        .expect("contract schema"),
                )
            })
            .clone()
    };
    if let Err(mut errors) = validator.validate(&value) {
        if let Some(e) = errors.next() {
            return Err(error(
                &source.path,
                &e.instance_path.to_string(),
                "E_CONTRACT_FORMAT",
                e.to_string(),
            ));
        }
    }
    Ok(value)
}

impl TargetContracts {
    pub fn load(context: &CoreSource, target: &CoreSource) -> Result<Self, CoreError> {
        let business_value = parse(context, BUSINESS_CONTEXT_SCHEMA)?;
        // Reuse the real input schema semantic gate, not just its JSON shape.
        parse_core_input_schema(&CoreSource {
            path: context.path.clone(),
            yaml: business_value["input_schema"].to_string(),
        })
        .map_err(|mut e| {
            e.diagnostic.field_path = Some(format!(
                "/input_schema{}",
                e.diagnostic.field_path.as_deref().unwrap_or("")
            ));
            e
        })?;
        let business: BusinessContext = serde_json::from_value(business_value)
            .map_err(|e| error(&context.path, "", "E_CONTRACT_FORMAT", e.to_string()))?;
        let declared: BTreeSet<_> = business.fields.keys().collect();
        let input: BTreeSet<_> = business.input_schema.fields.keys().collect();
        if declared != input {
            return Err(error(
                &context.path,
                "/fields",
                "E_CONTEXT_FIELDS",
                "Field meanings must cover exactly the declared input fields",
            ));
        }
        for (name, field) in &business.fields {
            if !business.entities.contains_key(&field.entity) {
                return Err(error(
                    &context.path,
                    &format!("/fields/{}/entity", pointer(name)),
                    "E_CONTEXT_ENTITY",
                    "Unknown entity reference",
                ));
            }
        }
        let mut target_value = parse(target, TARGET_CAPABILITIES_SCHEMA)?;
        // JSON Schema integers include 4.0; normalize only after bounded schema validation.
        target_value["max_sources"] = json!(target_value["max_sources"]
            .as_f64()
            .expect("bounded integer") as usize);
        let target_data: TargetCapabilities = serde_json::from_value(target_value)
            .map_err(|e| error(&target.path, "", "E_CONTRACT_FORMAT", e.to_string()))?;
        let context_sha256 = package::hash(context.yaml.as_bytes());
        if target_data.context.id != business.id
            || target_data.context.revision != business.revision
            || target_data.context.sha256 != context_sha256
        {
            return Err(error(
                &target.path,
                "/context",
                "E_CONTEXT_BINDING",
                "Target must pin the exact context ID, revision and source SHA-256",
            ));
        }
        if target_data.engine.version != ENGINE_VERSION
            || target_data.engine.profile != PROFILE
            || target_data.engine.language_version != "0.1"
        {
            return Err(error(
                &target.path,
                "/engine",
                "E_TARGET_VERSION",
                "This checker only supports the local engine version and Core draft-1 profile",
            ));
        }
        // Draft-1 is an atomic profile, not independently negotiated language fragments.
        let inventory: Value = serde_json::from_str(INVENTORY).expect("inventory");
        let required: BTreeSet<_> = inventory["capabilities"]
            .as_array()
            .expect("capabilities")
            .iter()
            .filter(|c| c["status"] == "supported")
            .map(|c| c["id"].as_str().expect("ID"))
            .collect();
        let offered: BTreeSet<_> = target_data
            .engine
            .capabilities
            .iter()
            .map(String::as_str)
            .collect();
        if required != offered {
            return Err(error(
                &target.path,
                "/engine/capabilities",
                "E_TARGET_CAPABILITIES",
                format!(
                    "Core profile capability mismatch; missing: {:?}; unknown: {:?}",
                    required.difference(&offered).collect::<Vec<_>>(),
                    offered.difference(&required).collect::<Vec<_>>()
                ),
            ));
        }
        if target_data.status != "ready" {
            return Err(error(
                &target.path,
                "/status",
                "E_TARGET_UNAVAILABLE",
                "Declared target is unavailable",
            ));
        }
        Ok(Self {
            business,
            target: target_data,
            context_path: context.path.clone(),
            target_path: target.path.clone(),
            context_sha256,
            target_sha256: package::hash(target.yaml.as_bytes()),
        })
    }

    pub fn validate_input(&self, input: &CoreSource) -> Result<(), CoreError> {
        if parse_core_input_schema(input)? != self.business.input_schema {
            return Err(error(
                &input.path,
                "",
                "E_CONTEXT_INPUT",
                "Input schema differs from the business context (including declared metadata)",
            ));
        }
        Ok(())
    }

    /// Caller explicitly chooses to send this declared context to its model.
    /// No test inputs, external fetches, credentials or authorization are added.
    pub fn prompt_context(&self) -> Value {
        json!({"business_context": self.business, "target_capabilities": self.target})
    }

    pub fn check(
        &self,
        sources: &[CoreSource],
        input: &CoreSource,
        expected_binding: Option<&str>,
    ) -> Result<CompatibilityReport, CoreError> {
        self.validate_input(input)?;
        if sources.len() > self.target.max_sources {
            return Err(error(
                &self.target_path,
                "/max_sources",
                "E_TARGET_LIMIT",
                "Source closure exceeds the declared target budget",
            ));
        }
        let policy_sha256 = package::policy_identity(sources, input)?;
        // Inspect only structurally and semantically validated source documents.
        // Check all decision branches, not just branches reached by a test suite.
        for source in sources {
            let value: Value = serde_yaml::from_str(&source.yaml).expect("compiled YAML");
            if let Some(decisions) = value["pipeline"]["decision"].as_array() {
                for (i, decision) in decisions.iter().enumerate() {
                    if let Some(actions) = decision["actions"].as_array() {
                        for (j, action) in actions.iter().enumerate() {
                            let name = action.as_str().expect("Core action");
                            if !self.business.actions.iter().any(|a| a == name)
                                || !self.target.actions.iter().any(|a| a == name)
                            {
                                return Err(error(&source.path, &format!("/pipeline/decision/{i}/actions/{j}"), "E_ACTION_UNAVAILABLE",
                                    format!("Action {name} must be declared by both context ({}) and target; declarations are not authority", self.context_path)));
                            }
                        }
                    }
                }
            }
        }
        let (checker_version, checker_sha256) = package::checker_identity()?;
        let binding_sha256 = package::json_hash(
            "core-target-binding-v1",
            json!({
                "policy_sha256":policy_sha256, "context_sha256":self.context_sha256,
                "target_sha256":self.target_sha256, "checker_version":checker_version,
                "checker_sha256":checker_sha256
            }),
        );
        if expected_binding.is_some_and(|expected| expected != binding_sha256) {
            return Err(error(
                "<binding>",
                "",
                "E_STALE_BINDING",
                "Previous binding is not applicable to the current policy, contracts or checker",
            ));
        }
        Ok(CompatibilityReport {
            report_version: "1",
            scope: "declared_target_compatibility",
            compatible: true,
            policy_sha256,
            context: DocumentBinding {
                id: self.business.id.clone(),
                revision: self.business.revision.clone(),
                sha256: self.context_sha256.clone(),
            },
            target: DocumentBinding {
                id: self.target.id.clone(),
                revision: self.target.revision.clone(),
                sha256: self.target_sha256.clone(),
            },
            checker_version,
            checker_sha256,
            binding_sha256,
            execution_checked: false,
            business_semantics_checked: false,
            live_target_verified: false,
            business_evaluation: "not_performed",
            publication_approval: "not_granted",
            authenticity: "unsigned",
        })
    }
}

/// Exact publication subject for the current resource-free Core profile.
/// Callers must not reuse this empty closure for connector/feature/model profiles.
pub fn core_evidence_subject(report: &CompatibilityReport) -> Result<Value, CoreError> {
    let binding = serde_json::json!({"kind":"corint-resource-bindings","contract_version":"1",
        "id":"core-empty-bindings","revision":"1",
        "provenance":{"producer":"corint-core","reference":"cdl-core-risk-draft-1"},
        "target_sha256":report.target.sha256,"resources":[]});
    let binding = crate::phase0::Contract::load(
        "resource-bindings",
        &CoreSource {
            path: "core-empty-bindings".into(),
            yaml: binding.to_string(),
        },
    )?;
    Ok(
        serde_json::json!({"policy_sha256":report.policy_sha256,"context_sha256":report.context.sha256,
        "target_sha256":report.target.sha256,"bindings_sha256":binding.sha256(),"checker_sha256":report.checker_sha256}),
    )
}
