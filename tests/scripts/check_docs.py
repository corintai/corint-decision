#!/usr/bin/env python3
"""Check every docs page's declared scope, local links and executable example bindings."""
import json
import hashlib
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "docs"
inventory = json.loads((DOCS / "inventory.json").read_text())
pages = {page["path"]: page for page in inventory["pages"]}
actual = {str(path.relative_to(DOCS)) for path in DOCS.rglob("*.md")}
errors = []
capabilities = json.loads((DOCS / "cdl/schema/capabilities.json").read_text())
if not (DOCS / "cdl/schema" / capabilities["stability_policy"]).is_file():
    errors.append("Missing version and stability contract")
for name, tool in capabilities["tools"].items():
    if not tool.get("scope") or tool.get("implementation_status") != "implemented_in_declared_scope":
        errors.append(f"Tool {name}: declare implementation scope separately from maturity")
    evidence = tool.get("evidence")
    if not evidence or not (DOCS / "cdl/schema" / evidence).is_file():
        errors.append(f"Tool {name}: missing evidence")
snippet_manifest = json.loads((DOCS / "cdl/snippets.json").read_text())
snippet_ids = set()
for page_name in json.loads((DOCS / "cdl/examples.json").read_text())["compatibility_pages"]:
    page_text = (DOCS / "cdl" / page_name).read_text()
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
for name in sorted(actual):
    page = pages.get(name, {})
    if page.get("scope") not in {"contract", "compatibility-reference", "design", "navigation"}:
        errors.append(f"{name}: declare a valid document scope")
    text = (DOCS / name).read_text()
    for target in re.findall(r"\[[^\]\n]*\]\(([^)\n]+)\)", text):
        target = target.strip().strip("<>").split(' "', 1)[0]
        url = urlsplit(target)
        if url.scheme or url.netloc or not url.path:
            continue
        if not ((DOCS / name).parent / unquote(url.path)).exists():
            errors.append(f"{name}: broken local link {target}")
    for marker in re.findall(r"<!-- executable-example: ([\w-]+) -->", text):
        bindings = [item for item in inventory["executable_examples"] if item["page"] == name and item["id"] == marker]
        if len(bindings) != 1:
            errors.append(f"{name}: example {marker} requires exactly one test binding")
for binding in inventory["executable_examples"]:
    page = DOCS / binding["page"]
    test = ROOT / binding["test_file"]
    marker = f'<!-- executable-example: {binding["id"]} -->'
    if not page.exists() or marker not in page.read_text():
        errors.append(f"Missing executable example {binding['id']}")
    if not test.exists() or f'fn {binding["test_name"]}(' not in test.read_text():
        errors.append(f"Missing executable test for {binding['id']}")
if errors:
    raise SystemExit("\n".join(errors))
print(f"Checked {len(actual)} document scopes, all local links and executable example bindings.")
