# CORINT WASM Browser Deployment Design

## 1. Executive Summary

This document outlines the architecture and implementation plan for compiling the CORINT decision engine to WebAssembly (WASM) and deploying it to run in user browsers. This enables client-side risk assessment with zero latency, offline capability, and enhanced privacy.

### Key Design Decision: Reuse Existing Runtime

**Instead of creating a separate WASM-specific runtime**, we reuse the existing `corint-runtime` with the following approach:

1. **Reuse `corint-runtime`**: The existing `PipelineExecutor` already uses optional dependencies (`Option<Arc<...>>`), making it perfect for WASM deployment
2. **No Data Sources**: Simply don't configure `feature_extractor` - rules that require data sources (PostgreSQL/Redis) won't be used in browser
3. **No Internal Services**: Simply don't configure `service_client` - rules that require internal services won't be used in browser  
4. **External API via JS Bridge**: Create a WASM-compatible `ExternalApiClient` that uses JavaScript `fetch()` through a callback
5. **Rule Configuration**: Browser-compatible rules simply don't include data source or service dependencies

This approach is **simpler, more maintainable, and leverages existing code** rather than duplicating runtime logic.

## 2. Architecture Overview

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Browser Environment                      │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────┐         ┌──────────────────────────────┐ │
│  │   Web App    │◄───────►│   CORINT WASM Module         │ │
│  │  (TypeScript)│         │   (Rust Compiled)            │ │
│  └──────────────┘         └──────────────────────────────┘ │
│         │                           │                        │
│         │                           ├─ Parser               │
│         │                           ├─ Compiler             │
│         │                           ├─ Runtime              │
│         │                           └─ Decision Engine      │
│         │                                                    │
│  ┌──────▼──────┐         ┌──────────────────────────────┐ │
│  │   IndexedDB │         │   Web Worker (Optional)      │ │
│  │  (Storage)  │         │   (Background Processing)    │ │
│  └─────────────┘         └──────────────────────────────┘ │
│                                                               │
└─────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────▼──────────┐
                    │   CDN / Static     │
                    │   File Server      │
                    │   (.wasm files)    │
                    └────────────────────┘
```

### 2.2 Component Breakdown

#### WASM Module Components
1. **corint-core** (Pure Rust, no I/O)
   - AST definitions
   - Expression evaluator
   - Type system

2. **corint-parser** (Pure Rust, WASM-compatible)
   - YAML parsing (using pure Rust libs)
   - Rule/Pipeline/Registry parsing

3. **corint-compiler** (Pure Rust, WASM-compatible)
   - IR generation
   - Optimization passes

4. **corint-runtime** (Reused, with WASM adaptations)
   - ✅ Reuse existing `PipelineExecutor`
   - ✅ Optional dependencies (feature_extractor, llm_client, service_client)
   - ⚠️ External API client needs JS bridge (via callback)
   - ⚠️ No data sources (PostgreSQL/Redis) - not configured in rules
   - ⚠️ No internal services - not configured in rules

5. **corint-wasm** (New crate)
   - JavaScript/TypeScript bindings
   - WASM-compatible ExternalApiClient (JS fetch bridge)
   - Serialization/Deserialization
   - Error handling

## 3. Detailed Design

### 3.1 WASM Module Structure

```
crates/
├── corint-core/              # ✅ Already WASM-compatible
├── corint-parser/            # ✅ Already WASM-compatible
├── corint-compiler/          # ✅ Already WASM-compatible
├── corint-runtime/           # ✅ Reused with WASM adaptations
└── corint-wasm/               # 🆕 JS/TS bindings + WASM ExternalApiClient
```

#### 3.1.1 Reusing Existing `corint-runtime`

**Key Insight**: The existing `corint-runtime` is already designed with optional dependencies, making it suitable for WASM deployment.

**Current Architecture**:
- `PipelineExecutor` uses `Option<Arc<...>>` for most dependencies
- `feature_extractor`: Optional (not needed in browser - no data sources)
- `llm_client`: Optional (can be provided via JS bridge if needed)
- `service_client`: Optional (not needed - no internal services)
- `external_api_client`: Required, but can be adapted for WASM

**WASM Adaptations**:

1. **External API Client**: Create a WASM-compatible version that uses JS `fetch()` via callback
2. **No Data Sources**: Simply don't configure `feature_extractor` - rules won't use data source features
3. **No Internal Services**: Simply don't configure `service_client` - rules won't use service calls
4. **Async Support**: WASM supports async/await via `wasm-bindgen-futures`

**Usage Pattern**:
```rust
// In WASM bindings
let executor = PipelineExecutor::new()  // No storage, no services
    .with_external_api_client(Arc::new(WasmExternalApiClient::new(js_fetch_callback)));
    // Optional: .with_llm_client(...) if LLM is needed via JS bridge
```

**Rule Configuration**:
- Rules that require data sources (PostgreSQL/Redis) simply won't be used in browser
- Rules that require internal services won't be used in browser
- Rules with external API calls will work via JS bridge
- Pure expression-based rules work perfectly

#### 3.1.2 New Crate: `corint-wasm`

**Purpose**: Expose Rust API to JavaScript/TypeScript using `wasm-bindgen`, and provide WASM-compatible ExternalApiClient.

**WASM ExternalApiClient Implementation**:
```rust
// crates/corint-wasm/src/external_api.rs
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use js_sys::Promise;
use corint_runtime::external_api::{ApiConfig, EndpointConfig};
use corint_core::Value;
use std::collections::HashMap;

/// WASM-compatible External API Client using JS fetch()
pub struct WasmExternalApiClient {
    configs: HashMap<String, ApiConfig>,
    fetch_callback: js_sys::Function,
}

impl WasmExternalApiClient {
    pub fn new(fetch_callback: js_sys::Function) -> Self {
        Self {
            configs: HashMap::new(),
            fetch_callback,
        }
    }

    pub fn register_api(&mut self, config: ApiConfig) {
        self.configs.insert(config.name.clone(), config);
    }

    pub async fn call(
        &self,
        api_name: &str,
        endpoint_name: &str,
        params: &HashMap<String, Value>,
        timeout: Option<u64>,
        ctx: &ExecutionContext,
    ) -> Result<Value> {
        // Build URL (same logic as original)
        let url = self.build_url(api_name, endpoint_name, params, ctx)?;
        
        // Call JS fetch via callback
        let promise = self.fetch_callback
            .call1(&JsValue::NULL, &JsValue::from_str(&url))
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("JS fetch failed: {:?}", e)))?;
        
        let promise = Promise::from(promise);
        let response = JsFuture::from(promise).await
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Fetch error: {:?}", e)))?;
        
        // Parse response and convert to Value
        // ... (implementation details)
    }
}
```

**WASM Bindings**:
```rust
// crates/corint-wasm/src/lib.rs
use wasm_bindgen::prelude::*;
use corint_runtime::engine::PipelineExecutor;
use corint_runtime::external_api::ExternalApiClient;
use corint_compiler::Compiler;
use corint_parser::*;
use std::sync::Arc;

#[wasm_bindgen]
pub struct CorintEngine {
    compiler: Compiler,
    executor: Arc<PipelineExecutor>,
    programs: HashMap<String, Program>,
}

#[wasm_bindgen]
impl CorintEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(fetch_callback: js_sys::Function) -> Result<CorintEngine, JsValue> {
        // Create WASM-compatible external API client
        let external_api = Arc::new(WasmExternalApiClient::new(fetch_callback));
        
        // Create executor without data sources or internal services
        let executor = Arc::new(
            PipelineExecutor::new()
                .with_external_api_client(external_api)
        );
        
        Ok(CorintEngine {
            compiler: Compiler::new(),
            executor,
            programs: HashMap::new(),
        })
    }

    #[wasm_bindgen]
    pub async fn load_rules(&mut self, yaml: &str) -> Result<(), JsValue> {
        // Parse and compile rules
        let pipeline = PipelineParser::parse(yaml)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;
        
        let program = self.compiler.compile_pipeline(pipeline)
            .map_err(|e| JsValue::from_str(&format!("Compile error: {}", e)))?;
        
        self.programs.insert(program.metadata.source_id.clone(), program);
        Ok(())
    }

    #[wasm_bindgen]
    pub async fn decide(&self, event_json: &str) -> Result<JsValue, JsValue> {
        // Execute decision using existing runtime
        let event_data: HashMap<String, Value> = serde_json::from_str(event_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid JSON: {}", e)))?;
        
        // Find matching pipeline (simplified)
        let program = self.programs.values().next()
            .ok_or_else(|| JsValue::from_str("No rules loaded"))?;
        
        let result = self.executor.execute(program, event_data).await
            .map_err(|e| JsValue::from_str(&format!("Execution error: {}", e)))?;
        
        Ok(JsValue::from_serde(&result)
            .map_err(|e| JsValue::from_str(&format!("Serialize error: {}", e)))?)
    }

    /// Decrypt encrypted rule package
    #[wasm_bindgen]
    pub async fn decrypt_rule(
        &self,
        encrypted_data: &str,
        encryption_key: &str,
        signature: &str,
        decryption_key: &str,
    ) -> Result<String, JsValue> {
        use crate::decryption::RuleDecryptor;
        
        let decryptor = RuleDecryptor::new(decryption_key)
            .map_err(|e| JsValue::from_str(&format!("Decryptor init failed: {}", e)))?;
        
        let package = EncryptedRulePackage {
            encrypted_data: encrypted_data.to_string(),
            encryption_key: encryption_key.to_string(),
            signature: signature.to_string(),
            version: String::new(),
            expires_at: None,
        };
        
        let decrypted = decryptor.decrypt_rule(&package)
            .map_err(|e| JsValue::from_str(&format!("Decryption failed: {}", e)))?;
        
        Ok(decrypted)
    }
}
```

### 3.2 TypeScript/JavaScript SDK

#### 3.2.1 NPM Package Structure

```
packages/
└── corint-wasm/
    ├── package.json
    ├── README.md
    ├── src/
    │   ├── index.ts           # Main entry point
    │   ├── engine.ts          # Engine wrapper
    │   ├── types.ts           # TypeScript types
    │   └── storage.ts         # IndexedDB adapter
    ├── dist/
    │   ├── corint.js          # Bundled JS
    │   ├── corint.d.ts        # Type definitions
    │   └── corint_bg.wasm     # WASM binary
    └── examples/
        ├── browser/           # Browser example
        └── react/             # React example
```

#### 3.2.2 TypeScript API

```typescript
// src/index.ts
export interface DecisionRequest {
  event_data: Record<string, any>;
  metadata?: Record<string, string>;
}

export interface DecisionResponse {
  action?: 'approve' | 'review' | 'deny';
  score: number;
  triggered_rules: string[];
  explanation: string;
  processing_time_ms: number;
  decision_id?: string;        // Unique decision ID for tracking
  timestamp?: number;           // Decision timestamp
}

export interface VerificationResult {
  verified: boolean;
  server_action?: 'approve' | 'review' | 'deny';
  server_score?: number;
  discrepancy?: string;          // If client and server disagree
  requires_override?: boolean;   // If server override is needed
}

export interface EngineConfig {
  rules?: string;        // YAML rules (deprecated, use loadRulesFromServer)
  registry?: string;     // YAML registry (deprecated, use loadRulesFromServer)
  serverUrl?: string;    // Server URL for dynamic rule loading
  apiKey?: string;      // API key for authentication
  reportToServer?: boolean;      // Auto-report decisions to server
  verifyWithServer?: boolean;    // Verify decisions with server
  reportBatchSize?: number;     // Batch size for reporting (default: 10)
  reportInterval?: number;      // Report interval in ms (default: 5000)
  features?: {
    // Feature provider callbacks
    [key: string]: (context: any) => Promise<any>;
  };
}

export interface EncryptedRulePackage {
  encrypted_data: string;      // Base64-encoded encrypted YAML
  encryption_key: string;       // Encrypted AES key (RSA-encrypted)
  signature: string;            // HMAC signature for integrity
  version: string;              // Rule version
  expires_at?: number;          // Unix timestamp (optional)
}

export class CorintEngine {
  private engine: any;  // WASM module
  private serverUrl?: string;
  private apiKey?: string;

  constructor(config?: EngineConfig);

  async init(): Promise<void>;

  // Legacy methods (for unencrypted rules)
  loadRules(yaml: string): void;
  loadRegistry(yaml: string): void;

  // Dynamic loading from server (encrypted)
  async loadRulesFromServer(ruleId?: string): Promise<void>;
  async loadEncryptedRules(encryptedPackage: EncryptedRulePackage, decryptionKey: string): Promise<void>;
  
  // Rule management
  async refreshRules(): Promise<void>;  // Reload from server
  getRuleVersion(): string | null;
  isRulesExpired(): boolean;

  decide(request: DecisionRequest): DecisionResponse;

  // Batch processing
  decideBatch(requests: DecisionRequest[]): DecisionResponse[];

  // Feature management
  registerFeature(name: string, provider: (ctx: any) => Promise<any>): void;

  // Storage
  saveToIndexedDB(key: string): Promise<void>;
  loadFromIndexedDB(key: string): Promise<void>;

  // Metrics
  getMetrics(): EngineMetrics;

  // Server interaction
  async reportDecision(request: DecisionRequest, response: DecisionResponse): Promise<void>;
  async verifyDecision(request: DecisionRequest, response: DecisionResponse): Promise<VerificationResult>;
  async syncDecision(request: DecisionRequest, response: DecisionResponse): Promise<void>;

  // Cleanup
  dispose(): void;
}
```

#### 3.2.3 Usage Example

```typescript
import { CorintEngine } from 'corint-wasm';

// Initialize engine
const engine = new CorintEngine();
await engine.init();

// Load rules from YAML
const rules = `
version: "0.1"
pipeline:
  id: fraud_check
  when:
    event.type: payment
  steps:
    - include:
        ruleset: fraud_rules
`;

engine.loadRules(rules);

// Load registry
const registry = `
version: "0.1"
registry:
  - pipeline: fraud_check
    when:
      event.type: payment
`;

engine.loadRegistry(registry);

// Make decision
const response = engine.decide({
  event_data: {
    event_type: 'payment',
    amount: 1500,
    user_id: 'user123',
    geo: { country: 'BR' }
  }
});

console.log('Decision:', response.action);
console.log('Score:', response.score);
console.log('Rules triggered:', response.triggered_rules);

// Report decision to server (if enabled)
if (engine.config.reportToServer) {
  await engine.reportDecision(event, response);
}

// Verify with server (optional, for high-risk decisions)
if (response.score > 100 && engine.config.verifyWithServer) {
  const verification = await engine.verifyDecision(event, response);
  if (!verification.verified || verification.requires_override) {
    // Use server decision instead
    console.warn('Server override:', verification.server_action);
  }
}
```

## 3.4 Decision Result & Server Interaction

### 3.4.1 Interaction Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Browser (Client-Side)                      │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  1. Execute Decision (WASM)                                   │
│     ↓                                                         │
│  2. Generate DecisionResult                                  │
│     ↓                                                         │
│  3. Report to Server (Async, Non-blocking)                  │
│     ├─ Audit Logging                                         │
│     ├─ Analytics                                             │
│     └─ Monitoring                                            │
│     ↓                                                         │
│  4. Optional: Verify with Server                              │
│     ├─ High-risk decisions                                   │
│     ├─ Server override check                                 │
│     └─ Discrepancy detection                                 │
│                                                               │
└───────────────────────────┬─────────────────────────────────┘
                            │ HTTPS
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    Server-Side (Backend)                     │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  POST /v1/decisions/report                                   │
│    - Store audit log                                         │
│    - Update analytics                                        │
│    - Trigger alerts if needed                                │
│                                                               │
│  POST /v1/decisions/verify                                   │
│    - Re-execute decision (server-side)                      │
│    - Compare with client result                              │
│    - Return verification result                              │
│                                                               │
│  POST /v1/decisions/sync                                      │
│    - Sync decision state                                     │
│    - Update user session                                     │
│    - Maintain consistency                                    │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

### 3.4.2 Decision Reporting

**Purpose**: 
- Audit logging for compliance
- Analytics and statistics
- Real-time monitoring and alerting
- Fraud detection pattern analysis

**Implementation**:

```typescript
// src/engine.ts - Decision reporting
export class CorintEngine {
  private reportQueue: Array<{request: DecisionRequest, response: DecisionResponse}> = [];
  private reportTimer: number | null = null;

  async reportDecision(
    request: DecisionRequest,
    response: DecisionResponse
  ): Promise<void> {
    // Add to queue for batch reporting
    this.reportQueue.push({ request, response });

    // Flush if queue is full
    if (this.reportQueue.length >= (this.config.reportBatchSize || 10)) {
      await this.flushReportQueue();
    } else {
      // Schedule flush
      this.scheduleReportFlush();
    }
  }

  private async flushReportQueue(): Promise<void> {
    if (this.reportQueue.length === 0) return;

    const batch = this.reportQueue.splice(0, this.config.reportBatchSize || 10);
    
    try {
      const response = await fetch(`${this.serverUrl}/v1/decisions/report`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${this.apiKey}`,
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          decisions: batch.map(({ request, response }) => ({
            decision_id: response.decision_id || this.generateDecisionId(),
            event_data: request.event_data,
            action: response.action,
            score: response.score,
            triggered_rules: response.triggered_rules,
            explanation: response.explanation,
            processing_time_ms: response.processing_time_ms,
            timestamp: response.timestamp || Date.now(),
            client_version: '1.0.0',
            rule_version: this.getRuleVersion(),
          }))
        })
      });

      if (!response.ok) {
        throw new Error(`Report failed: ${response.statusText}`);
      }
    } catch (error) {
      // Retry logic or store in IndexedDB for later
      console.error('Failed to report decisions:', error);
      this.storeFailedReports(batch);
    }
  }

  private scheduleReportFlush(): void {
    if (this.reportTimer) return;
    
    this.reportTimer = window.setTimeout(() => {
      this.flushReportQueue();
      this.reportTimer = null;
    }, this.config.reportInterval || 5000);
  }

  private generateDecisionId(): string {
    return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
  }
}
```

### 3.4.3 Decision Verification

**Purpose**:
- Verify high-risk decisions with server
- Detect discrepancies between client and server
- Enable server override for critical decisions
- Prevent client-side tampering

**Implementation**:

```typescript
// src/engine.ts - Decision verification
export class CorintEngine {
  async verifyDecision(
    request: DecisionRequest,
    clientResponse: DecisionResponse
  ): Promise<VerificationResult> {
    try {
      const response = await fetch(`${this.serverUrl}/v1/decisions/verify`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${this.apiKey}`,
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          decision_id: clientResponse.decision_id,
          event_data: request.event_data,
          client_result: {
            action: clientResponse.action,
            score: clientResponse.score,
            triggered_rules: clientResponse.triggered_rules,
          },
          rule_version: this.getRuleVersion(),
        })
      });

      if (!response.ok) {
        throw new Error(`Verification failed: ${response.statusText}`);
      }

      const result = await response.json();
      
      return {
        verified: result.verified,
        server_action: result.server_action,
        server_score: result.server_score,
        discrepancy: result.discrepancy,
        requires_override: result.requires_override,
      };
    } catch (error) {
      console.error('Verification error:', error);
      // Return safe default: not verified, but don't block
      return {
        verified: false,
        requires_override: false,
      };
    }
  }
}
```

**Server-Side Verification**:

```rust
// Server-side verification endpoint
// crates/corint-server/src/api/rest.rs

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub decision_id: String,
    pub event_data: HashMap<String, serde_json::Value>,
    pub client_result: ClientResult,
    pub rule_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClientResult {
    pub action: Option<String>,
    pub score: i32,
    pub triggered_rules: Vec<String>,
}

async fn verify_decision(
    State(state): State<AppState>,
    Json(payload): Json<VerifyRequest>,
) -> Result<Json<VerificationResponse>, ServerError> {
    // Re-execute decision on server
    let event_data = convert_to_corint_values(payload.event_data);
    let request = DecisionRequest::new(event_data);
    let server_response = state.engine.decide(request).await?;

    // Compare client and server results
    let client_action = payload.client_result.action.as_deref();
    let server_action = server_response.result.action
        .map(|a| format!("{:?}", a));

    let verified = client_action == server_action.as_deref()
        && (server_response.result.score - payload.client_result.score).abs() < 10; // Allow small variance

    let discrepancy = if !verified {
        Some(format!(
            "Client: {:?} (score: {}), Server: {:?} (score: {})",
            client_action,
            payload.client_result.score,
            server_action,
            server_response.result.score
        ))
    } else {
        None
    };

    // Determine if override is needed
    let requires_override = !verified && (
        server_response.result.score > 150 || // High risk
        server_response.result.action == Some(Action::Deny) // Denial
    );

    Ok(Json(VerificationResponse {
        verified,
        server_action,
        server_score: Some(server_response.result.score),
        discrepancy,
        requires_override,
    }))
}
```

### 3.4.4 Decision Synchronization

**Purpose**:
- Sync decision state with server
- Update user session state
- Maintain consistency across devices
- Support for decision history

**Implementation**:

```typescript
// src/engine.ts - Decision synchronization
export class CorintEngine {
  async syncDecision(
    request: DecisionRequest,
    response: DecisionResponse
  ): Promise<void> {
    try {
      await fetch(`${this.serverUrl}/v1/decisions/sync`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${this.apiKey}`,
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          decision_id: response.decision_id,
          event_data: request.event_data,
          action: response.action,
          score: response.score,
          triggered_rules: response.triggered_rules,
          timestamp: response.timestamp,
          session_id: this.getSessionId(),
        })
      });
    } catch (error) {
      console.error('Sync failed:', error);
      // Non-critical, don't throw
    }
  }

  private getSessionId(): string {
    // Get or create session ID
    let sessionId = sessionStorage.getItem('corint_session_id');
    if (!sessionId) {
      sessionId = this.generateDecisionId();
      sessionStorage.setItem('corint_session_id', sessionId);
    }
    return sessionId;
  }
}
```

### 3.4.5 Server API Endpoints

```typescript
// Server API for decision interaction

interface DecisionServerAPI {
  // Report decisions (batch)
  POST /v1/decisions/report
    Headers: {
      Authorization: "Bearer {apiKey}"
    }
    Body: {
      decisions: Array<{
        decision_id: string;
        event_data: Record<string, any>;
        action?: string;
        score: number;
        triggered_rules: string[];
        explanation: string;
        processing_time_ms: number;
        timestamp: number;
        client_version: string;
        rule_version?: string;
      }>;
    }
    Response: {
      reported: number;
      failed: number;
    }

  // Verify decision
  POST /v1/decisions/verify
    Body: {
      decision_id: string;
      event_data: Record<string, any>;
      client_result: {
        action?: string;
        score: number;
        triggered_rules: string[];
      };
      rule_version?: string;
    }
    Response: {
      verified: boolean;
      server_action?: string;
      server_score?: number;
      discrepancy?: string;
      requires_override: boolean;
    }

  // Sync decision state
  POST /v1/decisions/sync
    Body: {
      decision_id: string;
      event_data: Record<string, any>;
      action?: string;
      score: number;
      triggered_rules: string[];
      timestamp: number;
      session_id: string;
    }
    Response: {
      synced: boolean;
      session_updated: boolean;
    }
}
```

### 3.4.6 Usage Patterns

**Pattern 1: Auto-Report (Default)**
```typescript
const engine = new CorintEngine({
  serverUrl: 'https://api.corint.io',
  apiKey: 'your-api-key',
  reportToServer: true,  // Auto-report all decisions
});

const response = engine.decide(event);
// Decision is automatically queued for reporting
```

**Pattern 2: Verify High-Risk Decisions**
```typescript
const engine = new CorintEngine({
  serverUrl: 'https://api.corint.io',
  apiKey: 'your-api-key',
  verifyWithServer: true,  // Verify when needed
});

const response = engine.decide(event);

if (response.score > 100) {
  const verification = await engine.verifyDecision(event, response);
  
  if (verification.requires_override) {
    // Use server decision
    response.action = verification.server_action;
    response.score = verification.server_score || response.score;
  }
}
```

**Pattern 3: Manual Reporting**
```typescript
const engine = new CorintEngine({
  serverUrl: 'https://api.corint.io',
  apiKey: 'your-api-key',
  reportToServer: false,  // Manual control
});

const response = engine.decide(event);

// Report only important decisions
if (response.action === 'deny' || response.score > 50) {
  await engine.reportDecision(event, response);
}
```

**Pattern 4: Full Integration**
```typescript
const engine = new CorintEngine({
  serverUrl: 'https://api.corint.io',
  apiKey: 'your-api-key',
  reportToServer: true,
  verifyWithServer: true,
  reportBatchSize: 20,
  reportInterval: 3000,
});

// Decision execution with full server integration
const response = engine.decide(event);

// Sync state
await engine.syncDecision(event, response);

// Verify if high risk
if (response.score > 100) {
  const verification = await engine.verifyDecision(event, response);
  if (verification.requires_override) {
    // Handle override
  }
}
```

### 3.4.7 Error Handling & Retry

```typescript
// src/engine.ts - Error handling
export class CorintEngine {
  private failedReports: Array<{request: DecisionRequest, response: DecisionResponse, retries: number}> = [];

  private async storeFailedReports(batch: Array<{request: DecisionRequest, response: DecisionResponse}>): Promise<void> {
    // Store in IndexedDB for retry
    const db = await this.openDB();
    const tx = db.transaction('failed_reports', 'readwrite');
    const store = tx.objectStore('failed_reports');
    
    for (const item of batch) {
      await store.add({
        ...item,
        timestamp: Date.now(),
        retries: 0,
      });
    }
  }

  async retryFailedReports(): Promise<void> {
    const db = await this.openDB();
    const tx = db.transaction('failed_reports', 'readwrite');
    const store = tx.objectStore('failed_reports');
    const index = store.index('timestamp');
    
    const cursor = await index.openCursor();
    const reports: any[] = [];
    
    cursor?.continue();
    
    for (const report of reports) {
      if (report.retries < 3) {
        try {
          await this.reportDecision(report.request, report.response);
          await store.delete(report.id);
        } catch (error) {
          report.retries++;
          await store.put(report);
        }
      } else {
        // Max retries reached, move to dead letter queue
        await this.moveToDeadLetterQueue(report);
        await store.delete(report.id);
      }
    }
  }
}
```

### 3.4.8 Privacy & Data Minimization

```typescript
// Only report necessary data
interface MinimalDecisionReport {
  decision_id: string;
  action?: string;
  score: number;
  triggered_rules: string[];
  // PII removed or hashed
  event_hash: string;  // Hash of event data instead of full data
  timestamp: number;
}
```

### 3.4.9 Performance Considerations

1. **Batch Reporting**: Queue decisions and report in batches
2. **Async Non-blocking**: Reporting doesn't block decision execution
3. **Retry with Backoff**: Exponential backoff for failed reports
4. **Local Storage**: Store failed reports for retry when online
5. **Rate Limiting**: Respect server rate limits

#### 3.2.4 Dynamic Rule Loading with Encryption

**Security Requirements**:
- Rules must be encrypted and cannot be decrypted by unauthorized parties
- Rules should be loaded dynamically from server
- Support for rule versioning and expiration
- Integrity verification to prevent tampering

**Usage with Encrypted Rules**:
```typescript
import { CorintEngine } from 'corint-wasm';

// Initialize engine with server URL
const engine = new CorintEngine({
  serverUrl: 'https://api.corint.io/v1/rules',
  apiKey: 'your-api-key'
});

await engine.init();

// Load encrypted rules from server
await engine.loadRulesFromServer('fraud_detection_v2');

// Rules are automatically decrypted and loaded
// The decryption key is obtained securely from the server

// Make decision
const response = engine.decide({
  event_data: { event_type: 'payment', amount: 1500 }
});

// Check if rules need refresh
if (engine.isRulesExpired()) {
  await engine.refreshRules();
}
```

## 3.3 Rule Encryption & Security

### 3.3.1 Encryption Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Server-Side (Backend)                    │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  1. Rule YAML → Compress → Encrypt (AES-256-GCM)           │
│  2. AES Key → Encrypt with RSA-2048 (Public Key)             │
│  3. Generate HMAC-SHA256 signature                           │
│  4. Package: { encrypted_data, encryption_key, signature }   │
│                                                               │
└───────────────────────────┬─────────────────────────────────┘
                             │ HTTPS
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                    Client-Side (Browser)                     │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  1. Fetch encrypted package from server                     │
│  2. Decrypt AES key using RSA private key (in WASM)         │
│  3. Verify HMAC signature                                     │
│  4. Decrypt rule data → Decompress → Parse YAML             │
│  5. Compile and load into engine                             │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

### 3.3.2 Encryption Implementation

#### Server-Side Encryption (Rust)

```rust
// Server-side rule encryption
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rsa::{RsaPublicKey, pkcs1v15::EncryptingKey};
use sha2::{Sha256, Digest};
use hmac::{Hmac, Mac};

pub struct RuleEncryptor {
    rsa_public_key: RsaPublicKey,
    hmac_key: Vec<u8>,
}

impl RuleEncryptor {
    pub fn encrypt_rule(&self, yaml_content: &str) -> EncryptedRulePackage {
        // 1. Compress YAML
        let compressed = self.compress(yaml_content);
        
        // 2. Generate random AES-256 key
        let aes_key = Aes256Gcm::generate_key(&mut OsRng);
        let cipher = Aes256Gcm::new(&aes_key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        
        // 3. Encrypt with AES-256-GCM
        let encrypted_data = cipher.encrypt(&nonce, compressed.as_ref())
            .expect("Encryption failed");
        
        // 4. Encrypt AES key with RSA
        let rsa_encryptor = EncryptingKey::<Sha256>::new(self.rsa_public_key.clone());
        let encrypted_key = rsa_encryptor.encrypt(&mut OsRng, &aes_key.as_slice())
            .expect("RSA encryption failed");
        
        // 5. Generate HMAC signature
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.hmac_key)
            .expect("HMAC key invalid");
        mac.update(&encrypted_data);
        mac.update(&encrypted_key);
        let signature = mac.finalize().into_bytes();
        
        EncryptedRulePackage {
            encrypted_data: base64::encode(&encrypted_data),
            encryption_key: base64::encode(&encrypted_key),
            signature: base64::encode(&signature),
            version: self.generate_version(),
            expires_at: Some(chrono::Utc::now().timestamp() + 86400), // 24h
        }
    }
}
```

#### Client-Side Decryption (WASM)

```rust
// crates/corint-wasm/src/decryption.rs
use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
use rsa::{RsaPrivateKey, pkcs1v15::DecryptingKey};
use sha2::Sha256;
use hmac::{Hmac, Mac};

pub struct RuleDecryptor {
    rsa_private_key: RsaPrivateKey,
    hmac_key: Vec<u8>,
}

impl RuleDecryptor {
    pub fn decrypt_rule(&self, package: &EncryptedRulePackage) -> Result<String> {
        // 1. Verify HMAC signature
        self.verify_signature(package)?;
        
        // 2. Decrypt AES key with RSA
        let decryptor = DecryptingKey::<Sha256>::new(self.rsa_private_key.clone());
        let encrypted_key = base64::decode(&package.encryption_key)?;
        let aes_key_bytes = decryptor.decrypt(&encrypted_key)
            .map_err(|_| DecryptionError::InvalidKey)?;
        
        // 3. Decrypt rule data with AES-256-GCM
        let encrypted_data = base64::decode(&package.encrypted_data)?;
        let cipher = Aes256Gcm::new_from_slice(&aes_key_bytes)?;
        let nonce = &encrypted_data[..12]; // Extract nonce
        let ciphertext = &encrypted_data[12..];
        
        let decrypted = cipher.decrypt(nonce.into(), ciphertext.as_ref())
            .map_err(|_| DecryptionError::DecryptionFailed)?;
        
        // 4. Decompress
        let yaml_content = self.decompress(&decrypted)?;
        
        Ok(yaml_content)
    }
    
    fn verify_signature(&self, package: &EncryptedRulePackage) -> Result<()> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.hmac_key)?;
        let encrypted_data = base64::decode(&package.encrypted_data)?;
        let encrypted_key = base64::decode(&package.encryption_key)?;
        
        mac.update(&encrypted_data);
        mac.update(&encrypted_key);
        
        let signature = base64::decode(&package.signature)?;
        mac.verify_slice(&signature)
            .map_err(|_| DecryptionError::InvalidSignature)?;
        
        Ok(())
    }
}
```

### 3.3.3 Key Management

#### Key Distribution Strategy

1. **RSA Key Pair**:
   - Private key: Embedded in WASM binary (obfuscated)
   - Public key: Used by server to encrypt AES keys
   - Key rotation: Deploy new WASM version with new key pair

2. **HMAC Key**:
   - Shared secret between server and client
   - Embedded in WASM binary (obfuscated)
   - Used for integrity verification

3. **AES Key**:
   - Generated per-rule-package on server
   - Encrypted with RSA public key
   - Included in encrypted package

#### Key Obfuscation

```rust
// Obfuscate keys in WASM binary
// Use compile-time string obfuscation to prevent static analysis

#[inline(never)]
fn get_rsa_private_key() -> Vec<u8> {
    // Key is split and XORed at compile time
    let key_parts = [
        obfuscate!(b"-----BEGIN PRIVATE KEY-----\n"),
        // ... key parts ...
    ];
    deobfuscate_key(key_parts)
}
```

### 3.3.4 Server API

#### Rule Loading Endpoint

```typescript
// Server API for rule loading
interface RuleServerAPI {
  // Get encrypted rules
  GET /v1/rules/{ruleId}
    Headers: {
      Authorization: "Bearer {apiKey}",
      X-Client-Version: "1.0.0"
    }
    Response: {
      package: EncryptedRulePackage;
      metadata: {
        rule_id: string;
        version: string;
        created_at: number;
        expires_at: number;
      };
    }
  
  // Check for rule updates
  GET /v1/rules/{ruleId}/version
    Response: {
      current_version: string;
      latest_version: string;
      requires_update: boolean;
    }
}
```

### 3.3.5 Anti-Tampering Measures

1. **Code Obfuscation**:
   - WASM binary obfuscation
   - String encryption
   - Control flow flattening

2. **Runtime Integrity Checks**:
   - Verify WASM module integrity
   - Check for debugging tools
   - Detect code modification

3. **Secure Storage**:
   - Store decrypted rules in memory only (never persist)
   - Clear memory after use
   - Use secure memory allocation

4. **Network Security**:
   - HTTPS only
   - Certificate pinning
   - Request signing

### 3.3.6 Implementation in TypeScript SDK

```typescript
// src/engine.ts - Enhanced with encryption support
export class CorintEngine {
  private decryptor: RuleDecryptor | null = null;
  private currentRuleVersion: string | null = null;
  private ruleExpiresAt: number | null = null;

  async loadRulesFromServer(ruleId: string = 'default'): Promise<void> {
    if (!this.serverUrl) {
      throw new Error('Server URL not configured');
    }

    // Fetch encrypted package
    const response = await fetch(`${this.serverUrl}/rules/${ruleId}`, {
      headers: {
        'Authorization': `Bearer ${this.apiKey}`,
        'X-Client-Version': '1.0.0'
      }
    });

    if (!response.ok) {
      throw new Error(`Failed to load rules: ${response.statusText}`);
    }

    const data = await response.json();
    const package: EncryptedRulePackage = data.package;

    // Decrypt and load
    await this.loadEncryptedRules(package, this.getDecryptionKey());
    
    // Store metadata
    this.currentRuleVersion = data.metadata.version;
    this.ruleExpiresAt = data.metadata.expires_at;
  }

  private async loadEncryptedRules(
    package: EncryptedRulePackage,
    decryptionKey: string
  ): Promise<void> {
    // Call WASM decryption function
    const decryptedYaml = await this.engine.decrypt_rule(
      package.encrypted_data,
      package.encryption_key,
      package.signature,
      decryptionKey
    );

    // Load decrypted rules
    this.engine.load_rules(decryptedYaml);
  }

  private getDecryptionKey(): string {
    // Get obfuscated key from WASM module
    // Key is embedded in WASM binary and obfuscated
    return this.engine.get_decryption_key();
  }

  isRulesExpired(): boolean {
    if (!this.ruleExpiresAt) return false;
    return Date.now() / 1000 > this.ruleExpiresAt;
  }

  async refreshRules(): Promise<void> {
    // Check for updates
    const response = await fetch(`${this.serverUrl}/rules/version`, {
      headers: { 'Authorization': `Bearer ${this.apiKey}` }
    });
    
    const { requires_update } = await response.json();
    
    if (requires_update) {
      await this.loadRulesFromServer();
    }
  }
}
```

### 3.3.7 Security Best Practices

1. **Never expose decryption keys in client code**
2. **Use key rotation for compromised keys**
3. **Implement rate limiting on rule loading**
4. **Monitor for suspicious decryption attempts**
5. **Use short expiration times for sensitive rules**
6. **Implement rule versioning and rollback capability**
7. **Log all rule loading events for audit**

### 3.3 Limitations & Trade-offs

#### What Works in Browser:
✅ Rule parsing and compilation
✅ Expression evaluation
✅ Pipeline execution
✅ Registry-based routing
✅ Decision logic
✅ External API calls (via JS fetch bridge)
✅ Offline capability (for pure expression rules)

#### What Doesn't Work (Not Configured):
❌ Data source features (PostgreSQL/Redis) → Rules simply don't use these features
❌ Internal service calls → Rules simply don't use service steps
❌ LLM calls → Optional, can be bridged via JS if needed

**Key Design Decision**: Instead of creating a separate runtime, we:
1. Reuse existing `corint-runtime` with optional dependencies
2. Don't configure data sources or internal services in browser rules
3. Provide JS bridge for External API calls (which are still needed)
4. Rules that require unavailable features simply won't be used in browser context

#### Configuration Strategy:

**1. External API Calls** (Supported via JS Bridge)
```typescript
// External API calls work automatically via WASM ExternalApiClient
// The WASM module calls JS fetch() through the callback
const engine = new CorintEngine(fetch);  // Pass native fetch function

// Rules with external API calls work normally:
// - external: { api: "ipinfo", endpoint: "lookup", params: {...} }
```

**2. Data Source Features** (Not Used in Browser)
```yaml
# Rules that require data sources simply won't be loaded/used in browser
# Example: This rule won't work in browser (requires PostgreSQL)
# rules:
#   - id: user_history_check
#     when:
#       conditions:
#         - count(user.transactions, last_7d) > 10  # Requires data source
```

**3. Internal Services** (Not Used in Browser)
```yaml
# Service steps are not configured in browser rules
# Example: This step won't be used in browser
# steps:
#   - service:
#       service: "user_service"
#       operation: "get_risk_score"  # Requires internal service
```

**4. Browser-Compatible Rules**
```yaml
# These rules work perfectly in browser:
rules:
  - id: simple_check
    when:
      conditions:
        - event.amount > 1000
        - event.country == "US"
    score: 50

# External API calls work:
steps:
  - extract:
      features:
        - name: ip_info
          value: external(api="ipinfo", endpoint="lookup", params={ip: event.ip})
```

## 4. Build & Deployment Pipeline

### 4.1 Build Process

```bash
# Install wasm-pack
cargo install wasm-pack

# Build WASM module
cd crates/corint-wasm
wasm-pack build --target web --out-dir ../../packages/corint-wasm/dist

# Build TypeScript wrapper
cd ../../packages/corint-wasm
npm install
npm run build

# Publish to NPM
npm publish
```

### 4.2 Cargo.toml Configuration

```toml
# crates/corint-wasm/Cargo.toml
[package]
name = "corint-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
corint-core = { path = "../corint-core" }
corint-parser = { path = "../corint-parser" }
corint-compiler = { path = "../corint-compiler" }
corint-runtime = { path = "../corint-runtime", default-features = false }
# Note: Exclude sqlx feature to avoid database dependencies

wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"  # For async support in WASM
js-sys = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde-wasm-bindgen = "0.6"
console_error_panic_hook = "0.1"
wee_alloc = "0.4"  # Smaller allocator for WASM

# Encryption dependencies
aes-gcm = "0.10"           # AES-256-GCM encryption
rsa = { version = "0.9", features = ["sha2"] }  # RSA encryption for key wrapping
sha2 = "0.10"              # SHA-256 for HMAC
hmac = "0.12"              # HMAC-SHA256 for integrity
base64 = "0.21"            # Base64 encoding
flate2 = "1.0"             # Compression (gzip)

[dev-dependencies]
wasm-bindgen-test = "0.3"

[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Link-time optimization
codegen-units = 1   # Better optimization
```

**Note on `corint-runtime` dependencies**:
- `reqwest` is used by `ExternalApiClient`, but we'll replace it with JS fetch in WASM
- `sqlx` is optional (via feature flag), so we exclude it
- `tokio` async runtime works in WASM via `wasm-bindgen-futures`

### 4.3 Package Size Optimization

**Target Sizes**:
- **Initial WASM**: ~500KB (gzipped: ~150KB)
- **With all features**: ~800KB (gzipped: ~250KB)

**Optimization Techniques**:
1. Remove async runtime (tokio) → **-200KB**
2. Use `wee_alloc` → **-50KB**
3. Strip debug info → **-100KB**
4. Enable LTO → **-80KB**
5. Remove unused dependencies → **-70KB**

```bash
# Build optimized WASM
wasm-pack build --target web --release -- \
  --no-default-features \
  -Z build-std=std,panic_abort \
  -Z build-std-features=panic_immediate_abort

# Further optimize with wasm-opt
wasm-opt dist/corint_bg.wasm -O4 -o dist/corint_bg.wasm
```

### 4.4 CDN Deployment

```
https://cdn.corint.io/
├── v0.1.0/
│   ├── corint.js           # JS wrapper
│   ├── corint_bg.wasm      # WASM binary
│   ├── corint.d.ts         # TypeScript types
│   └── integrity.json      # SRI hashes
├── v0.2.0/
│   └── ...
└── latest/                 # Symlink to latest version
```

**HTML Integration**:
```html
<script type="module">
  import init, { CorintEngine } from 'https://cdn.corint.io/latest/corint.js';

  await init();  // Load WASM
  const engine = new CorintEngine();
  // Use engine...
</script>
```

## 5. Implementation Roadmap

### Phase 1: Core WASM Module (Week 1-2)
- [ ] Create `corint-wasm` crate
- [ ] Implement WASM-compatible `ExternalApiClient` (JS fetch bridge)
- [ ] Create WASM bindings for `PipelineExecutor` (reuse existing runtime)
- [ ] Basic rule parsing and execution
- [ ] Unit tests for WASM module

### Phase 2: Rule Encryption & Security (Week 2-3)
- [ ] Implement rule encryption module (AES-256-GCM + RSA)
- [ ] Implement rule decryption in WASM
- [ ] Add HMAC signature verification
- [ ] Key obfuscation and secure storage
- [ ] Server-side encryption API
- [ ] Security testing and audit

### Phase 3: TypeScript SDK (Week 3-4)
- [ ] Create NPM package structure
- [ ] Implement TypeScript wrapper
- [ ] Add dynamic rule loading from server
- [ ] Add encrypted rule support
- [ ] Add IndexedDB storage adapter (for metadata only, not decrypted rules)
- [ ] Feature callback system
- [ ] Error handling and logging

### Phase 3.5: Server Interaction (Week 4)
- [ ] Implement decision reporting (batch, async)
- [ ] Implement decision verification endpoint
- [ ] Implement decision synchronization
- [ ] Add retry logic and error handling
- [ ] Add offline support (queue failed reports)
- [ ] Server-side verification logic
- [ ] Performance optimization (batching, rate limiting)

### Phase 4: Build & Optimization (Week 5)
- [ ] Build pipeline setup
- [ ] Size optimization
- [ ] Performance benchmarks
- [ ] Browser compatibility testing
- [ ] Documentation

### Phase 4: Examples & Integration (Week 5)
- [ ] Browser example (vanilla JS)
- [ ] React example
- [ ] Vue example
- [ ] E-commerce demo
- [ ] Migration guide

### Phase 5: Production Readiness (Week 6)
- [ ] CDN setup
- [ ] Versioning strategy
- [ ] Security audit
- [ ] Performance monitoring
- [ ] Release v0.1.0

## 6. Performance Considerations

### 6.1 Expected Performance

| Metric | Target | Notes |
|--------|--------|-------|
| WASM Load Time | < 200ms | First load (cached: < 50ms) |
| Engine Init | < 100ms | Parse rules and compile |
| Decision Latency | < 5ms | Simple rules |
| Decision Latency | < 20ms | Complex pipelines |
| Memory Usage | < 10MB | Per engine instance |
| Batch Processing | 1000+ req/sec | In Web Worker |

### 6.2 Optimization Strategies

1. **Lazy Loading**: Load WASM on-demand
2. **Code Splitting**: Separate parser from runtime
3. **Caching**: Cache compiled programs in IndexedDB
4. **Web Workers**: Offload processing to background
5. **Streaming**: Use WebAssembly streaming compilation

```typescript
// Streaming compilation (faster startup)
const module = await WebAssembly.compileStreaming(
  fetch('https://cdn.corint.io/latest/corint_bg.wasm')
);
```

## 7. Security Considerations

### 7.1 Threats
- XSS attacks in rule definitions
- Malicious YAML injection
- Resource exhaustion (infinite loops)
- Memory leaks
- **Rule reverse engineering and tampering** (NEW)
- **Decryption key extraction** (NEW)
- **Man-in-the-middle attacks on rule loading** (NEW)

### 7.2 Mitigations

1. **Rule Encryption**: All rules encrypted with AES-256-GCM + RSA-2048
   - Prevents rule inspection and modification
   - Keys obfuscated in WASM binary
   - HMAC signature prevents tampering

2. **Rule Validation**: Strict YAML parsing with schema validation
   - Validate decrypted rules before loading
   - Reject malformed or suspicious rules

3. **Sandboxing**: WASM provides memory isolation
   - Decrypted rules stored in memory only
   - Never persist decrypted rules to disk/IndexedDB

4. **Resource Limits**:
   - Max rule complexity
   - Execution timeout
   - Memory cap
   - Rate limiting on rule loading

5. **Network Security**:
   - HTTPS only for rule loading
   - Certificate pinning
   - Request signing with API keys

6. **Code Protection**:
   - WASM binary obfuscation
   - String encryption
   - Control flow flattening
   - Anti-debugging measures

7. **Content Security Policy**:
```html
<meta http-equiv="Content-Security-Policy"
      content="script-src 'self' https://cdn.corint.io">
```

8. **Subresource Integrity**:
```html
<script src="https://cdn.corint.io/v0.1.0/corint.js"
        integrity="sha384-..."
        crossorigin="anonymous"></script>
```

### 7.3 Key Management Security

**Key Storage**:
- RSA private key embedded in WASM (obfuscated)
- HMAC key embedded in WASM (obfuscated)
- Keys never transmitted over network
- Keys rotated with WASM version updates

**Key Rotation Strategy**:
1. Generate new RSA key pair
2. Deploy new WASM version with new keys
3. Server encrypts new rules with new public key
4. Old rules continue to work during transition
5. Deprecate old keys after transition period

**Compromise Response**:
- Immediately revoke compromised keys
- Deploy emergency WASM update
- Invalidate all rules encrypted with compromised keys
- Force rule refresh for all clients

## 8. Testing Strategy

### 8.1 Unit Tests (Rust)
```bash
# Test WASM bindings
wasm-pack test --headless --chrome
wasm-pack test --headless --firefox
```

### 8.2 Integration Tests (TypeScript)
```typescript
import { test } from '@playwright/test';

test('decision execution', async ({ page }) => {
  await page.goto('http://localhost:3000');

  const result = await page.evaluate(() => {
    const engine = new CorintEngine();
    return engine.decide({ event_data: { ... } });
  });

  expect(result.action).toBe('approve');
});
```

### 8.3 Performance Tests
- Lighthouse CI
- WebPageTest
- Custom benchmarks

## 9. Browser Compatibility

### 9.1 Minimum Requirements
- **Chrome**: 57+ (2017)
- **Firefox**: 52+ (2017)
- **Safari**: 11+ (2017)
- **Edge**: 16+ (2017)

### 9.2 Polyfills
- WebAssembly polyfill for IE11 (degraded performance)
- IndexedDB shim for older browsers

## 10. Alternative Architectures

### 10.1 Hybrid Approach
```
┌─────────────────┐
│    Browser      │
│  (Light Rules)  │ ← Fast, offline
└────────┬────────┘
         │
         ↓ (Fallback for complex rules)
┌─────────────────┐
│  Server/Edge    │
│ (Full Engine)   │ ← Complete features
└─────────────────┘
```

### 10.2 Edge Computing
Deploy WASM to Cloudflare Workers / Fastly Compute@Edge:
- Lower latency than server
- Higher capability than browser
- Shared infrastructure

## 11. Migration Path

### 11.1 From Server to Browser
```typescript
// Before (server-side)
const response = await fetch('/api/decide', {
  method: 'POST',
  body: JSON.stringify(event)
});

// After (client-side WASM)
const engine = new CorintEngine();
await engine.init();
const response = engine.decide(event);
```

### 11.2 Gradual Rollout
1. **Week 1**: A/B test 5% traffic
2. **Week 2**: 25% traffic
3. **Week 3**: 50% traffic
4. **Week 4**: 100% traffic
5. **Monitor**: Latency, errors, user experience

## 12. Success Metrics

| Metric | Baseline (Server) | Target (WASM) |
|--------|------------------|---------------|
| P50 Latency | 100ms | 5ms |
| P99 Latency | 500ms | 20ms |
| Offline Support | ❌ | ✅ |
| Server Costs | $1000/mo | $100/mo |
| User Privacy | Lower | Higher |

## 13. Future Enhancements

1. **WASM SIMD**: Parallel rule evaluation
2. **Streaming Decisions**: Real-time risk scoring
3. **ML Integration**: TensorFlow.js for learned models
4. **Multi-threading**: SharedArrayBuffer + Workers
5. **Progressive Loading**: Stream rules on-demand

## 14. References

- [WebAssembly.org](https://webassembly.org/)
- [wasm-bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)
- [Rust WASM Book](https://rustwasm.github.io/docs/book/)
- [MDN: WebAssembly](https://developer.mozilla.org/en-US/docs/WebAssembly)
