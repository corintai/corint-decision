# CDL Imports

This reference defines experimental authoring profile **`cdl-core-import-draft-1`**,
language version **`"0.1"`**. Imports compose a complete resource set for execution
profile **`cdl-core-risk-draft-1`**. They are resolved before strict Core compilation;
runtime evaluation consumes the resolved resources.

The source profile is selected explicitly. A `version: "0.1"` field alone does not
select this profile at a compatibility entry point. Resource definitions follow
the [CDL resource contract](overall.md#document-and-resource-constraints) and [resource Schema](schema/core.json).

## 1. Source structure

Each resource file uses exactly one of these forms:

| Form | Required structure |
|---|---|
| Single YAML document | Explicit string `version: "0.1"`, exactly one resource definition, and an optional `import` mapping alongside that definition. |
| Two YAML documents | A header containing only `version: "0.1"` and `import`, followed by exactly one resource document. Its `version` may be omitted to inherit the header's value; an explicit value must also be the string `"0.1"`. |

A resource definition is one `rule`, `ruleset`, `pipeline` or `registry`. The
two-document form requires `---` between the header and resource. The resource
document cannot contain another `import`; dependencies belong in the header.
Comments and mapping key order do not change the accepted structure.

Unknown fields, duplicate YAML keys, empty documents, extra resource definitions,
third documents, non-string versions and version conflicts are rejected. Two
documents without an import header are also rejected.

The [import header Schema](schema/import-header.json) validates the import mapping
before normalization. Resolution removes the validated `import`, combines a
two-document source into one document, and supplies its inherited version.
Each resulting resource must satisfy the resource Schema. Imports do not merge
resource fields or provide inheritance.

## 2. Import groups

| Group | Required resource in each target file |
|---|---|
| `import.rules` | One `rule` |
| `import.rulesets` | One `ruleset` |
| `import.pipelines` | One `pipeline` |

When `import` is present, it must be a mapping containing at least one of these
groups. Each present group is an array of **1–256 unique path strings**. An absent
group is allowed; an empty array, `null`, wrong value type or unknown group is rejected.
Omit `import` entirely when a single-document source has no imports.

A source may declare any of these groups regardless of its own resource kind.
The target's kind must match the group. Registry files are explicit entries;
there is no `import.registries` group, and a Registry cannot be an import target.
The input Schema is supplied separately and cannot be imported as a resource.

## 3. Paths and resource identity

All entry and import paths are relative to the **explicit repository root**,
including paths declared inside nested directories. Directory names such as
`rules/`, `rulesets/` and `pipelines/` are conventions rather than required prefixes.

Paths are case-sensitive ASCII labels matching
`^[A-Za-z0-9_-]+(/[A-Za-z0-9_-]+)*\.ya?ml$`, with a maximum of 256 bytes.
Both `.yaml` and `.yml` are accepted. Absolute paths, `.`/`..` segments, empty
components, backslashes, URLs and globs are rejected. For example,
`rules/fraud/amount.yaml` and `shared/amount.yml` are valid labels;
`./amount.yaml` and `../rules/amount.yaml` are invalid.

Path identity and resource identity have separate rules:

- The same exact path reached from multiple parents is loaded once. Repeating a
  path within one import group is invalid, even though sharing it across parents
  is allowed.
- Different labels are not merged as filesystem aliases. Different files with
  the same resource ID are rejected, even if their definitions are identical.
- Rule, Ruleset and Pipeline IDs share one namespace within the complete resolved
  resource set. They must be unique across kinds. Files outside that set do not
  participate in its ID checks.
- Imports name files; resource references name IDs. A filename does not determine
  the resource ID. ID syntax follows the [common resource constraints](overall.md#document-and-resource-constraints).

## 4. Dependency and execution semantics

Resolution starts from **1–256 unique explicit entry paths**, follows every declared
import transitively and forms the complete resource set. That set must contain
exactly one Registry and every referenced resource. A reference may use any
matching resource in the set; it need not be imported directly by the referencing
file. Referencing an ID does not discover a file in a global catalog.

`Registry → Pipeline → Ruleset → Rule` is a common organization, illustrated in the
[payment review project](examples/payment-review/guide.md).
It is not a restriction on which resource kinds may declare imports. For example,
a Pipeline can import Rules directly or import another Pipeline, and a Rule source
can declare dependencies.

Importing a resource makes it available for references. Evaluation is determined by
[Registry selection](registry.md#strict-core-registry), the [Pipeline's steps and decisions](pipeline.md),
and the [Ruleset's ordered Rule references](ruleset.md). Import group or path order
does not set execution order. Importing a Pipeline does not call it; a subpipeline
call requires an explicit Pipeline step and the corresponding execution capability.

Every imported resource is validated, including imports that execution never uses.
Missing files, wrong target kinds, unresolved references, duplicate IDs and direct
or indirect import cycles are rejected before evaluation. No missing dependency is
silently skipped or replaced. Resources outside the entry/import closure are not
discovered or parsed for resolution.

The authoring profile has these bounds:

| Limit | Maximum |
|---|---|
| Resource files in the resolved set | 256, excluding the input Schema |
| Active import stack | 32 resource files, counting the entry file as level 1 |
| Original bytes per resource or input Schema | 1 MiB |
| Combined original resource and input Schema bytes | 16 MiB |

Shared paths count once toward file and byte limits.

### Import diagnostics

| Failure | Diagnostic |
|---|---|
| Invalid import mapping/path, missing dependency, imported input Schema or import cycle | `E_INVALID_IMPORT` |
| Resource kind does not match its import group | `E_IMPORT_KIND` |
| File count, active import stack or byte bound exceeded | `E_IMPORT_LIMIT` |
| Repeated label in the supplied source collection | `E_DUPLICATE_SOURCE` |
| Conflicting resource IDs or multiple Registries | `E_DUPLICATE_ID` |
| Missing resource reference | `E_UNRESOLVED_REF` |

Dependency failures and cycles identify the import chain. After normalization, resource
errors retain the original source label and a field pointer into the normalized document.
A source line/column is reported only when available; normalization does not invent one.
Errors stop resolution and do not enable a different language profile.
