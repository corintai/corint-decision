//! Normalize source layout before validating individual CDL resources.
//! Repeated resource declarations are retained; ordinary mapping fields stay unique.
use serde::de::{Error, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fmt;

const RESOURCES: &[&str] = &[
    "rule", "ruleset", "pipeline", "registry", "features", "lists",
];

struct Entries(Vec<(String, Value)>);
impl<'de> Deserialize<'de> for Entries {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Root;
        impl<'de> Visitor<'de> for Root {
            type Value = Entries;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a CDL mapping")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Entries, M::Error> {
                let mut entries = vec![];
                let mut seen = BTreeSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !seen.insert(key.clone()) && !RESOURCES.contains(&key.as_str()) {
                        return Err(M::Error::custom(format!("duplicate field {key}")));
                    }
                    // serde_yaml::Value rejects nested duplicates instead of losing data.
                    let yaml = map.next_value::<serde_yaml::Value>()?;
                    let value = serde_yaml::from_value(yaml).map_err(M::Error::custom)?;
                    entries.push((key, value));
                }
                Ok(Entries(entries))
            }
        }
        deserializer.deserialize_map(Root)
    }
}

/// Accept adjacent top-level declarations or explicit YAML document separators.
/// The first document's version/import header is inherited by following resources.
/// Bare service/list documents retain their existing representation.
pub(super) fn parse(text: &str) -> Result<Vec<Value>, serde_yaml::Error> {
    let mut output = vec![];
    let mut shared = Map::new();
    for (index, document) in serde_yaml::Deserializer::from_str(text).enumerate() {
        let Entries(entries) = Entries::deserialize(document)?;
        let mut common: Map<String, Value> = entries
            .iter()
            .filter(|(key, _)| !RESOURCES.contains(&key.as_str()))
            .cloned()
            .collect();
        let explicit_version = common.contains_key("version");
        let header_only = !common.is_empty()
            && entries
                .iter()
                .all(|(key, _)| key == "version" || key == "import");
        if index == 0 {
            for key in ["version", "import"] {
                if let Some(value) = common.get(key) {
                    shared.insert(key.into(), value.clone());
                }
            }
        } else {
            for (key, value) in &shared {
                if common.contains_key(key) {
                    if key == "import" {
                        return Err(serde_yaml::Error::custom(format!(
                            "Conflicting {key} in source header and resource document"
                        )));
                    }
                } else {
                    common.insert(key.clone(), value.clone());
                }
            }
        }
        if header_only && index == 0 {
            continue;
        }
        let mut declarations = 0;
        for (key, value) in entries {
            if RESOURCES.contains(&key.as_str()) {
                let mut resource = common.clone();
                if key == "lists" {
                    resource.remove("version");
                }
                resource.insert(key, value);
                output.push(Value::Object(resource));
                declarations += 1;
            }
        }
        if declarations == 0 {
            // Services and bare lists do not have a version field in their schema.
            if common.contains_key("base_url")
                || common.contains_key("backend")
                || common.contains_key("datasource")
            {
                if index > 0 && !explicit_version {
                    common.remove("version");
                }
            }
            output.push(Value::Object(common));
        }
    }
    if output.is_empty() {
        return Err(serde_yaml::Error::custom(
            "Expected at least one CDL resource",
        ));
    }
    Ok(output)
}
