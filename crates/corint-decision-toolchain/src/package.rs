//! Experimental source packages. Digests identify content; they do not authenticate
//! an author, approve deployment, or turn local examples into business evaluation.
use crate::{behavior, failure, label};
use corint_decision_compiler::core::{
    compile_core, diagnostic, parse_core_input_schema, validate_core_document, CoreError,
    CoreSource, PROFILE,
};
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

const PACKAGE_SCHEMA: &str = include_str!("../../../docs/cdl/schema/source-package.json");
const INPUT_PATH: &str = "input-schema.yaml";
const SUITE_PATH: &str = "test-suite.yaml";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    format: String,
    format_version: String,
    profile: String,
    language_version: String,
    policy: Policy,
    evidence: Evidence,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    path: String,
    sha256: String,
    yaml: String,
}
impl Document {
    fn new(path: String, yaml: String) -> Self {
        Self {
            path,
            sha256: hash(yaml.as_bytes()),
            yaml,
        }
    }
    fn source(&self) -> CoreSource {
        CoreSource {
            path: self.path.clone(),
            yaml: self.yaml.clone(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    sha256: String,
    input_schema: Document,
    sources: Vec<Document>,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Tool {
    version: String,
    executable_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    policy_sha256: String,
    suite_sha256: String,
    tool: Tool,
    validation: String,
    behavior: String,
    test_report_sha256: String,
    #[serde(deserialize_with = "count")]
    total: usize,
    #[serde(deserialize_with = "count")]
    passed: usize,
    business_evaluation: String,
    publication_approval: String,
    authenticity: String,
}

// JSON Schema's integer type includes 5.0. Normalize only exact bounded counts;
// do not silently round floats or let schema/serde acceptance diverge.
fn count<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<usize, D::Error> {
    let number = serde_json::Number::deserialize(deserializer)?;
    let n = number
        .as_f64()
        .ok_or_else(|| serde::de::Error::custom("Invalid count"))?;
    if n.fract() != 0.0 || !(1.0..=1000.0).contains(&n) {
        return Err(serde::de::Error::custom(
            "Count must be an integer from 1 to 1000",
        ));
    }
    Ok(n as usize)
}
#[derive(Serialize)]
pub struct Receipt {
    pub path: String,
    pub policy_sha256: String,
    pub package_sha256: String,
    pub suite_sha256: String,
    pub test_report_sha256: String,
    pub tool_sha256: String,
    pub authenticity: &'static str,
    pub publication_approval: &'static str,
}

pub struct Verification {
    pub receipt: Option<Receipt>,
    pub tests: behavior::TestResults,
    pub error: Option<CoreError>,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// Not RFC 8785/JCS: sorted UTF-8 JSON keys recursively, array order preserved,
// primitive encoding from serde_json. Domain-separated, explicitly versioned.
fn canonical(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Object(object) => {
            output.push(b'{');
            let mut entries: Vec<_> = object.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            for (i, (key, value)) in entries.into_iter().enumerate() {
                if i > 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, key).expect("JSON key");
                output.push(b':');
                canonical(value, output);
            }
            output.push(b'}');
        }
        Value::Array(items) => {
            output.push(b'[');
            for (i, value) in items.iter().enumerate() {
                if i > 0 {
                    output.push(b',');
                }
                canonical(value, output);
            }
            output.push(b']');
        }
        other => serde_json::to_writer(output, other).expect("JSON primitive"),
    }
}
fn json_hash(domain: &str, value: Value) -> String {
    let mut bytes = b"corint-canonical-json-v1\0".to_vec();
    canonical(&json!({"domain":domain,"value":value}), &mut bytes);
    hash(&bytes)
}

fn tool() -> Result<Tool, CoreError> {
    let io_error = |e: std::io::Error| failure("<tool>", "load", "E_IO", e.to_string());
    let executable = std::env::current_exe().map_err(io_error)?;
    let mut file = std::fs::File::open(executable).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(Tool {
        version: env!("CARGO_PKG_VERSION").into(),
        executable_sha256: format!("{hasher:x}", hasher = hasher.finalize()),
    })
}

fn canonical_sources(sources: &[CoreSource]) -> Result<Vec<Document>, CoreError> {
    let mut docs = Vec::new();
    for source in sources {
        validate_core_document(source)?;
        // Shape/duplicate-key validation above is the shared Core gate. This
        // reads only the validated ID to give each source a location-free label.
        let value: serde_yaml::Value = serde_yaml::from_str(&source.yaml).expect("validated YAML");
        let path = if value.get("registry").is_some() {
            "registry.yaml".into()
        } else {
            let kind = ["rule", "ruleset", "pipeline"]
                .into_iter()
                .find(|k| value.get(*k).is_some())
                .expect("validated kind");
            format!(
                "{kind}/{}.yaml",
                value[kind]["id"].as_str().expect("validated ID")
            )
        };
        docs.push(Document::new(path, source.yaml.clone()));
    }
    docs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(docs)
}

fn policy_hash(policy: &Policy) -> String {
    let sources: Vec<_> = policy
        .sources
        .iter()
        .map(|d| json!({"path":d.path,"sha256":d.sha256}))
        .collect();
    json_hash(
        "core-source-policy-v1",
        json!({
            "profile":PROFILE,"language_version":"0.1",
            "input_schema_sha256":policy.input_schema.sha256,"sources":sources
        }),
    )
}

pub fn prepare(
    sources: &[CoreSource],
    input: &CoreSource,
    suite: &CoreSource,
) -> Result<(Option<Package>, behavior::TestResults), CoreError> {
    if sources.len() > 10000 {
        return Err(failure(
            "<bundle>",
            "package",
            "E_PACKAGE_FORMAT",
            "At most 10000 sources per package",
        ));
    }
    let schema = parse_core_input_schema(input)?;
    // Validate the original closure before canonicalizing labels, so a malformed
    // caller source has its original diagnostic and duplicate IDs cannot collapse.
    compile_core(sources, schema.clone())?;
    let mut policy = Policy {
        sha256: String::new(),
        input_schema: Document::new(INPUT_PATH.into(), input.yaml.clone()),
        sources: canonical_sources(sources)?,
    };
    policy.sha256 = policy_hash(&policy);
    let sources: Vec<_> = policy.sources.iter().map(Document::source).collect();
    let tool = tool()?;
    let tests = behavior::test(
        &sources,
        schema,
        &CoreSource {
            path: SUITE_PATH.into(),
            yaml: suite.yaml.clone(),
        },
    )?;
    if tests.failed != 0 {
        return Ok((None, tests));
    }
    let evidence = Evidence {
        policy_sha256: policy.sha256.clone(),
        suite_sha256: hash(suite.yaml.as_bytes()),
        tool,
        validation: "passed".into(),
        behavior: "passed".into(),
        test_report_sha256: json_hash(
            "core-test-report-v1",
            serde_json::to_value(&tests).expect("test report"),
        ),
        total: tests.total,
        passed: tests.passed,
        business_evaluation: "not_performed".into(),
        publication_approval: "not_granted".into(),
        authenticity: "unsigned".into(),
    };
    let package = Package {
        format: "corint-core-source-package".into(),
        format_version: "1".into(),
        profile: PROFILE.into(),
        language_version: "0.1".into(),
        policy,
        evidence,
    };
    validate_package(&package, "<build>")?;
    Ok((Some(package), tests))
}

fn validate_package(package: &Package, source: &str) -> Result<(), CoreError> {
    let schema =
        JSONSchema::compile(&serde_json::from_str(PACKAGE_SCHEMA).expect("package schema JSON"))
            .expect("embedded package schema");
    let value = serde_json::to_value(package).expect("package JSON");
    if let Err(mut errors) = schema.validate(&value) {
        if let Some(error) = errors.next() {
            return Err(diagnostic(
                source,
                &error.instance_path.to_string(),
                "package",
                "E_PACKAGE_FORMAT",
                error.to_string(),
            ));
        }
    }
    Ok(())
}

pub fn check_output(path: &Path) -> Result<(), CoreError> {
    match path.symlink_metadata() {
        Ok(_) => Err(failure(
            &label(path),
            "write",
            "E_OUTPUT_EXISTS",
            "Output already exists; never overwriting",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failure(&label(path), "write", "E_IO", error.to_string())),
    }
}

fn receipt(package: &Package, path: String, bytes: &[u8]) -> Receipt {
    Receipt {
        path,
        policy_sha256: package.policy.sha256.clone(),
        package_sha256: hash(bytes),
        suite_sha256: package.evidence.suite_sha256.clone(),
        test_report_sha256: package.evidence.test_report_sha256.clone(),
        tool_sha256: package.evidence.tool.executable_sha256.clone(),
        authenticity: "unsigned",
        publication_approval: "not_granted",
    }
}

pub fn write(package: &Package, path: &Path) -> Result<Receipt, CoreError> {
    check_output(path)?;
    let mut bytes = serde_json::to_vec_pretty(package).expect("package JSON");
    bytes.push(b'\n');
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let io_error = |e: std::io::Error| failure(&label(path), "write", "E_IO", e.to_string());
    // Same-directory temporary + no-clobber persist, including a competing writer.
    // Tempfile's Drop removes the pending file on any error; source files are untouched.
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
    file.write_all(&bytes).map_err(io_error)?;
    file.as_file().sync_all().map_err(io_error)?;
    file.persist_noclobber(path).map_err(|e| {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            failure(
                &label(path),
                "write",
                "E_OUTPUT_EXISTS",
                "Another writer created the output; never overwriting",
            )
        } else {
            io_error(e.error)
        }
    })?;
    Ok(receipt(package, label(path), &bytes))
}

pub fn verify(stored: &CoreSource, suite: &CoreSource) -> Result<Verification, CoreError> {
    // Derived strict structs reject duplicate/unknown keys at every level. We
    // never read embedded labels as paths, extract archives, or follow imports.
    let package: Package = serde_json::from_str(&stored.yaml).map_err(|e| {
        let mut error = failure(&stored.path, "package", "E_PACKAGE_FORMAT", e.to_string());
        error.diagnostic.line = Some(e.line());
        error.diagnostic.column = Some(e.column());
        error
    })?;
    validate_package(&package, &stored.path)?;
    let invalid = |path: &str, code: &str, message: &str| {
        diagnostic(&stored.path, path, "package", code, message)
    };
    for (path, doc) in std::iter::once((
        "/policy/input_schema".to_string(),
        &package.policy.input_schema,
    ))
    .chain(
        package
            .policy
            .sources
            .iter()
            .enumerate()
            .map(|(i, d)| (format!("/policy/sources/{i}"), d)),
    ) {
        if hash(doc.yaml.as_bytes()) != doc.sha256 {
            return Err(invalid(
                &path,
                "E_PACKAGE_INTEGRITY",
                "Embedded source bytes do not match their SHA-256",
            ));
        }
    }
    if package.policy.sha256 != policy_hash(&package.policy)
        || package.evidence.policy_sha256 != package.policy.sha256
    {
        return Err(invalid(
            "/evidence/policy_sha256",
            "E_POLICY_BINDING",
            "Policy or report binding mismatch",
        ));
    }
    if package.evidence.suite_sha256 != hash(suite.yaml.as_bytes()) {
        return Err(invalid(
            "/evidence/suite_sha256",
            "E_SUITE_MISMATCH",
            "Supply the exact test file used by this package",
        ));
    }
    if package.evidence.tool != tool()? {
        return Err(invalid("/evidence/tool","E_TOOL_MISMATCH","Evidence requires the exact host executable; rebuild under the current tool to create new evidence"));
    }
    if package.policy.input_schema.path != INPUT_PATH {
        return Err(invalid(
            "/policy/input_schema/path",
            "E_PACKAGE_FORMAT",
            "Noncanonical input schema label",
        ));
    }
    let sources: Vec<_> = package
        .policy
        .sources
        .iter()
        .map(Document::source)
        .collect();
    let canonical = canonical_sources(&sources)?;
    if !canonical
        .iter()
        .zip(&package.policy.sources)
        .all(|(a, b)| a.path == b.path)
    {
        return Err(invalid(
            "/policy/sources",
            "E_PACKAGE_FORMAT",
            "Source labels/order must match their resource kind/ID",
        ));
    }
    // Recompile/retest from the embedded snapshot, not the original filesystem.
    // Recomputed digests alone cannot make a stale or fabricated report pass.
    let (fresh, tests) = prepare(&sources, &package.policy.input_schema.source(), suite)?;
    let error = if let Some(fresh) = fresh {
        if fresh.evidence.test_report_sha256 != package.evidence.test_report_sha256
            || fresh.evidence.total != package.evidence.total
            || fresh.evidence.passed != package.evidence.passed
        {
            Some(invalid(
                "/evidence/test_report_sha256",
                "E_REPORT_MISMATCH",
                "Fresh behavior results do not match the stored evidence",
            ))
        } else {
            None
        }
    } else {
        Some(invalid(
            "/evidence",
            "E_PACKAGE_TEST_FAILED",
            "Embedded policy failed the supplied tests",
        ))
    };
    Ok(Verification {
        receipt: error
            .is_none()
            .then(|| receipt(&package, stored.path.clone(), stored.yaml.as_bytes())),
        tests,
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_encoding_is_versioned_domain_separated_and_sorted() {
        assert_eq!(
            hash(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut bytes = Vec::new();
        canonical(&json!({"z":[2,1],"a":{"z":"é","a":true}}), &mut bytes);
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "{\"a\":{\"a\":true,\"z\":\"é\"},\"z\":[2,1]}"
        );
        assert_ne!(
            json_hash("policy", json!({"a":1})),
            json_hash("report", json!({"a":1}))
        );
        assert_ne!(
            json_hash("policy", json!([1, 2])),
            json_hash("policy", json!([2, 1]))
        );
    }
}
