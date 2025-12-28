# TODO List

## Pipeline Features

### Shadow Mode / Observe Mode (P1 - Important)

**Problem**: New pipeline versions need to be validated without affecting business decisions.

**Proposed Syntax**:
```yaml
pipeline:
  id: fraud_detection_v2
  mode: shadow                     # decision | shadow
  # decision: affects business decisions (default)
  # shadow: executes fully and logs, but doesn't affect decisions

  # Optional: traffic sampling
  traffic:
    percentage: 10                 # 10% traffic
    hash_key: event.user.id        # distribution key
```

**Implementation Approach**:
- Runtime flag: `context._mode = "shadow"`
- Execute all steps, but don't update `sys.decision`
- Full trace recording for comparison analysis

**Priority**: P1

**Status**: Pending

---

## Other TODO Items

(Add other TODO items here)
