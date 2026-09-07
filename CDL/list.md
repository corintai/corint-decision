# Corint Definition Language (CDL)

## List membership (compatibility execution)

This page defines dynamic membership expressions for the compatibility parser/compiler and
an executor with a bound List service. This execution path does not enable dynamic
List access in the strict Core profile.

## 1. Scope

`list.<id>` refers to a named collection supplied by the host. It is not a CDL resource declaration
and does not create a collection or select a storage backend. A membership expression produces a
Boolean only after a successful lookup.

The strict profile `cdl-core-risk-draft-1` rejects dynamic List lookups with
`E_UNSUPPORTED_CAPABILITY` during type checking, including when every resource, reference and input
field is otherwise valid. Adding `version: "0.1"` does not enable the capability. Literal-array
membership such as `event.country in ["US", "CA"]` has a separate, supported Core meaning; see
[expressions](expression.md), the [CDL reference](overall.md) and the
[language scope](overall.md#language-scope).

## 2. Expression syntax

| Expression | Meaning after a successful lookup |
|---|---|
| `event.email in list.email_blocklist` | True when the queried value is a member |
| `event.user_id not in list.trusted_users` | Negation of the membership result |

The left operand is an expression whose evaluated value is passed to the backend; the backend
limits the accepted types. Conditions may combine lookups with `all`, `any` and other expressions.
Only evaluated conditions perform a lookup: normal short-circuiting still applies.

On the right of `in` / `not in`, the `list.` prefix identifies a List reference, not an array or an
ordinary input field. It does not expose list contents or a lookup-result namespace in the context.
List IDs are case-sensitive. Each dot-separated segment must be an expression identifier;
`list.fraud.emails` refers to the single registered ID `fraud.emails`. Prefer simple IDs such as
`email_blocklist` or `trusted_users` using ASCII letters, digits and underscores, starting with a
letter or underscore. Naming style is a convention, not a declaration of business behavior.

## 3. Matching and value types

CDL does not currently impose a common typed-equality contract on all compatibility List adapters.
Use non-null strings with consistent formatting for portable policies, and verify the chosen
adapter before relying on other types. The built-in Memory and File adapters behave as follows:

| Query value | Memory | File |
|---|---|---|
| String | Exact, case-sensitive key | Exact, case-sensitive key |
| Number or Boolean | Converted to a string key | Converted to a string key |
| `null` | Uses the key `"null"` | Membership is always false |
| Array or object | Uses its serialized JSON text as the key | Lookup fails with an invalid-value error |

These adapters do not preserve type distinctions in scalar keys: number `42` and string `"42"`
match the same entry, as do Boolean `true` and string `"true"`. Memory also merges `null` with the
string `"null"`. Serialized object keys are not a portable structural-equality guarantee.
These details describe the current adapters, not requirements for every external backend.

Queries are not automatically trimmed, case-folded or normalized. File loading trims stored lines
and discards blank lines and comment lines; it does not trim the query value. Email-domain extraction,
IP ranges/CIDR, regular expressions and expiry are not implied by `in list.<id>`.
A missing input may fail before a lookup, depending on the entry point; the `null` row describes a
value actually passed to the backend, not a guarantee that an absent field is accepted.

## 4. Membership and policy effects

A list's name or purpose does not select a decision. A blocklist can be used in a scoring rule or
an explicit decline route; an allowlist can reduce a score or select an explicit route. The policy
must state the effect. Watchlists and greylists do not automatically trigger review or expiry.

In particular, a Rule with `score: -200` reduces the aggregate score when matched. It does not skip
later rules or guarantee approval. To bypass checks, define that behavior explicitly in the
Pipeline, including precedence when the same entity appears in both allowlists and blocklists.

## 5. Identity and binding

The host must register every referenced List ID and supply a List-capable executor. Successful
parsing or compilation does not prove that a List exists or can be queried. Multiple conditions
may reference the same ID; the reference does not select storage, connections, caching or reloading.
Those host responsibilities are outside the List language definition.

## 6. Error Handling

Compatibility compilation emits the List reference without contacting a catalog or backend. A
missing List ID or absent List service therefore fails at lookup time with `E_LIST_UNAVAILABLE`.
Strict Core rejects the capability before execution.

Both `in list.<id>` and `not in list.<id>` propagate lookup errors. Negation applies only to a
successful Boolean result. An explicitly registered empty list is valid: for a supported query
value, membership is false and negated membership is true. An absent or failed-to-load configuration
must not be treated as an empty list.

Backend lookup failures propagate as execution errors; they do not automatically select approve,
decline or a default route. The historical `fallback: allow` / `deny` API-list proposal does not
define an implicit language fallback. Loading, retained snapshots after reload failures and service
registration are host behavior.
