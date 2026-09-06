#!/usr/bin/env python3
"""Check every docs page's declared scope, local links and executable example bindings."""
import json
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "docs"
inventory = json.loads((DOCS / "inventory.json").read_text())
pages = {page["path"]: page for page in inventory["pages"]}
actual = {str(path.relative_to(DOCS)) for path in DOCS.rglob("*.md")}
errors = []
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
