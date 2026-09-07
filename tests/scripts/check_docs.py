#!/usr/bin/env python3
"""Check CDL and docs scopes, local links and executable example bindings."""
import json
import hashlib
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "docs"
CDL = ROOT / "CDL"
CONTRACT_SCHEMAS = DOCS / "contracts/schema"
DOCUMENTATION_FIXTURES = ROOT / "tests/conformance/documentation"
inventory = json.loads((DOCS / "inventory.json").read_text())
pages = {page["path"]: page for page in inventory["pages"]}
errors = []
if inventory.get("version") != 2 or inventory.get("roots") != ["CDL", "docs"]:
    errors.append("Document inventory must declare repository-relative CDL and docs roots")
actual = {
    str(path.relative_to(ROOT))
    for directory in (CDL, DOCS)
    for path in directory.rglob("*.md")
}
capabilities = json.loads((CONTRACT_SCHEMAS / "capabilities.json").read_text())
for name, tool in capabilities["tools"].items():
    if not tool.get("scope") or tool.get("implementation_status") != "implemented_in_declared_scope":
        errors.append(f"Tool {name}: declare implementation scope separately from maturity")
    evidence = tool.get("evidence")
    if not evidence or not (CONTRACT_SCHEMAS / evidence).is_file():
        errors.append(f"Tool {name}: missing evidence")
for field in (
    "schema", "input_schema", "test_suite_schema", "source_package_schema",
    "generation_response_schema", "source_bundle_schema", "example_registry",
):
    if not (CONTRACT_SCHEMAS / capabilities[field]).is_file():
        errors.append(f"Capability inventory: missing {field}")
for field in ("condition_trace", "call_trace"):
    if not (CONTRACT_SCHEMAS / capabilities[field]["schema"]).is_file():
        errors.append(f"Capability inventory: missing {field} schema")
example_registry = json.loads((DOCUMENTATION_FIXTURES / "examples.json").read_text())
snippet_manifest = json.loads((DOCUMENTATION_FIXTURES / "snippets.json").read_text())
for name, manifest in (("examples.json", example_registry), ("snippets.json", snippet_manifest)):
    if manifest.get("version") != 2 or manifest.get("path_base") != "repository":
        errors.append(f"{name}: require version 2 with repository-relative paths")
snippet_ids = set()
for page_name in example_registry["compatibility_pages"]:
    page_text = (ROOT / page_name).read_text()
    blocks = re.findall(r"^```([^\n]*)\n(.*?)^```\s*$", page_text, re.M | re.S)
    bindings = [s for s in snippet_manifest["snippets"] if s["page"] == page_name]
    if len(bindings) != len(blocks):
        errors.append(f"{page_name}: every historical block must have an explicit classification")
    for index, (language, code) in enumerate(blocks, 1):
        matches = [s for s in bindings if s["block"] == index]
        if len(matches) != 1:
            errors.append(f"{page_name} block {index}: missing/duplicate binding")
            continue
        binding = matches[0]
        if binding["id"] in snippet_ids:
            errors.append(f"Duplicate snippet ID {binding['id']}")
        snippet_ids.add(binding["id"])
        if binding["sha256"] != hashlib.sha256(code.encode()).hexdigest() or binding["language"] != language:
            errors.append(f"{page_name} block {index}: content changed; reclassify and run snippet admission tests")
        expected = "core-negative" if language in {"yaml", "json"} else "syntax-reference"
        if binding["kind"] != expected and not (binding["kind"] == "core-fragment" and binding.get("wrapper")):
            errors.append(f"{page_name} block {index}: promote supported examples into executable fixtures")
if snippet_ids != {s["id"] for s in snippet_manifest["snippets"]}:
    errors.append("Stale snippet bindings")
if len(pages) != len(inventory["pages"]) or actual != set(pages):
    errors.append(f"Document inventory mismatch: missing={actual-set(pages)}, stale={set(pages)-actual}")
def link_targets(text):
    inline = re.findall(r"\[[^\]\n]*\]\(([^)\n]+)\)", text)
    references = re.findall(r"^\s{0,3}\[[^]\n]+\]:\s*(\S+)", text, re.M)
    html = re.findall(r'(?:href|src)=[\"\']([^\"\']+)[\"\']', text)
    autolinks = re.findall(r"<((?:https?|file)://[^>]+)>", text)
    return inline + references + html + autolinks


for name in sorted(actual):
    page = pages.get(name, {})
    if page.get("scope") not in {"contract", "compatibility-reference", "design", "navigation"}:
        errors.append(f"{name}: declare a valid document scope")
    text = (ROOT / name).read_text()
    for target in link_targets(text):
        target = target.strip().strip("<>").split(' "', 1)[0]
        url = urlsplit(target)
        if url.scheme or url.netloc:
            if name.startswith("CDL/"):
                errors.append(f"{name}: external document reference {target}")
            continue
        if not url.path:
            continue
        resolved = ((ROOT / name).parent / unquote(url.path)).resolve()
        if name.startswith("CDL/") and not resolved.is_relative_to(CDL.resolve()):
            errors.append(f"{name}: language reference leaves CDL: {target}")
        if not resolved.exists():
            errors.append(f"{name}: broken local link {target}")
    for marker in re.findall(r"<!-- executable-example: ([\w-]+) -->", text):
        bindings = [item for item in inventory["executable_examples"] if item["page"] == name and item["id"] == marker]
        if len(bindings) != 1:
            errors.append(f"{name}: example {marker} requires exactly one test binding")
for binding in inventory["executable_examples"]:
    page = ROOT / binding["page"]
    test = ROOT / binding["test_file"]
    marker = f'<!-- executable-example: {binding["id"]} -->'
    if not page.exists() or marker not in page.read_text():
        errors.append(f"Missing executable example {binding['id']}")
    if "source" in binding:
        source = (ROOT / binding["source"]).resolve()
        targets = {(page.parent / unquote(urlsplit(target).path)).resolve()
                   for target in link_targets(page.read_text())
                   if not urlsplit(target).scheme and urlsplit(target).path}
        if not source.is_file() or source not in targets:
            errors.append(f"Missing linked source for {binding['id']}")
    if not test.exists() or f'fn {binding["test_name"]}(' not in test.read_text():
        errors.append(f"Missing executable test for {binding['id']}")
if errors:
    raise SystemExit("\n".join(errors))
print(f"Checked {len(actual)} document scopes, all local links and executable example bindings.")
