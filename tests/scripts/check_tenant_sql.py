#!/usr/bin/env python3
"""Keep online tenant SQL behind fixed operations with mandatory identity binds.

Schema migrations and compatibility-only adapters are outside this architecture
check. Behavioral isolation is additionally exercised against SQLite and PG.
"""
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
SERVER = ROOT / "crates/corint-decision-server/src"


def unscoped_queries(source):
    errors = []
    for match in re.finditer(r'"(?:[^"\\]|\\.)*"', source):
        sql = json.loads(match.group())
        operation = sql.split(" ", 1)[0]
        if operation not in {"SELECT", "INSERT", "UPDATE", "DELETE"}:
            continue
        if operation == "INSERT":
            valid = bool(re.search(r"\(tenant_id,.*\) VALUES\(\$1,", sql))
        else:
            valid = "WHERE tenant_id=$1" in sql
            if operation == "SELECT":
                valid = valid and sql.count("WHERE tenant_id=$1") >= sql.count(" FROM ")
        if not valid:
            errors.append(sql)
    return errors


def main():
    errors = []
    for name in ["journal/queries.rs", "tenancy/queries.rs"]:
        for sql in unscoped_queries((SERVER / name).read_text()):
            errors.append(f"{name}: SQL is missing an automatically bound tenant predicate: {sql}")
    raw = r"sqlx::(?:query\w*|raw_sql)|(?:Any|Pg|Sqlite)Pool"
    if re.search(raw, (SERVER / "journal.rs").read_text()):
        errors.append("journal.rs: business operations must use Storage's tenant-bound operations")
    control = (SERVER / "tenancy/store.rs").read_text().split("pub async fn state(", 1)[1]
    if re.search(raw, control):
        errors.append("tenancy/store.rs: online operations must use the scoped query constructor")
    if "corint_tenant_publication" in (SERVER / "repo_source.rs").read_text():
        errors.append("repo_source.rs: tenant publication SQL belongs in tenancy/queries.rs")
    if re.search(r"pub(?:\([^)]*\))?\s+pool\s*:", (SERVER / "journal/storage.rs").read_text()):
        errors.append("Journal pool must remain private to the tenant-bound storage layer")
    # Check the checker against the exact regressions it is intended to catch.
    for unsafe in ['SELECT * FROM events', 'UPDATE events SET delivered=1 WHERE digest=$2',
                   'DELETE FROM request_keys WHERE expires<$2', 'INSERT INTO events(body) VALUES($1)',
                   'SELECT (SELECT COUNT(*) FROM request_keys) FROM journal_usage WHERE tenant_id=$1']:
        assert unscoped_queries(json.dumps(unsafe)), unsafe
    if errors:
        raise SystemExit("\n".join(errors))
    print("Checked mandatory tenant SQL boundaries and SELECT/INSERT/UPDATE/DELETE predicates.")


if __name__ == "__main__":
    main()
