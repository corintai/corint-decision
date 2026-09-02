// TypeScript example for CORINT Decision Engine

import { Engine, version } from './index.js';

interface ApiEvent {
  type: string;
  timestamp: string;
  user_id: string;
  amount?: number;
  currency?: string;
  ip_address?: string;
  device_id?: string;
  transaction_count?: number;
  user_type?: string;
  risk_score?: number;
  location?: {
    country: string;
    city?: string;
    latitude?: number;
    longitude?: number;
  };
}

interface ApiUser {
  account_age_days?: number;
  email_verified?: boolean;
  phone_verified?: boolean;
  timezone?: string;
  risk_level?: string;
}

interface ApiRequest {
  event: ApiEvent;
  user?: ApiUser;
  options?: {
    enable_trace?: boolean;
  };
  metadata?: Record<string, string>;
  features?: Record<string, any>;
}

interface DecisionRequest {
  event_data: Record<string, any>;
  options?: {
    enable_trace?: boolean;
  };
  metadata?: Record<string, string>;
  features?: Record<string, any>;
}

interface DecisionResponse {
  request_id: string;
  pipeline_id: string | null;
  result: {
    signal: { type: string } | null;
    actions: string[];
    score: number;
    triggered_rules: string[];
    explanation: string;
    context: Record<string, any>;
  };
  processing_time_ms: number;
  metadata: Record<string, string>;
  trace?: Record<string, any>;
}

function buildDecisionRequest(apiRequest: ApiRequest): DecisionRequest {
  const eventData = { ...apiRequest.event };
  if (apiRequest.user) {
    eventData.user = apiRequest.user;
  }

  const request: DecisionRequest = {
    event_data: eventData,
    options: {
      enable_trace: Boolean(apiRequest.options && apiRequest.options.enable_trace),
    },
  };

  if (apiRequest.metadata) {
    request.metadata = apiRequest.metadata;
  }

  if (apiRequest.features) {
    request.features = apiRequest.features;
  }

  return request;
}

async function main() {
  console.log('CORINT Decision Engine Version:', version());

  // Example 1: Simple pipeline with inline rules
  try {
    const pipelineYaml = `
version: "0.1"

# Define inline rules first
---

rule:
  id: high_amount_rule
  name: High Amount Detection
  when:
    all:
      - event.amount > 1000
  
  score: 100

---

rule:
  id: low_amount_rule
  name: Low Amount Processing
  when:
    all:
      - event.amount <= 1000
  score: -50

---

ruleset:
  id: amount_check_ruleset
  name: Amount Check Ruleset
  rules:
    - high_amount_rule
    - low_amount_rule
  
  conclusion:
    - when: total_score >= 50
      signal: review
      reason: "Suspicious user behavior detected"

    - default: true
      signal: approve
      reason: "User behavior normal"
---

pipeline:
  id: typescript_fraud_check
  name: TypeScript Fraud Check
  description: Simple fraud detection pipeline
  entry: amount_check

  when:
    all:
      - event.type == "transaction"
      - event.amount > 0

  steps:
    - step:
        id: amount_check
        name: Amount Check
        type: ruleset
        ruleset: amount_check_ruleset
        next: end

  decision:
    - when: results.amount_check_ruleset.signal == "review"
      result: review
      actions: ["manual_review"]
      reason: "{results.amount_check_ruleset.reason}"
    - when: results.amount_check_ruleset.signal == "approve"
      result: approve
      reason: "{results.amount_check_ruleset.reason}"
    - default: true
      result: pass
      reason: "No decision"
`;

    const engine = await Engine.fromYaml('typescript_fraud_check', pipelineYaml);
    console.log('✓ Engine loaded from YAML content\n');

    // Test with high amount
    const apiRequest: ApiRequest = {
      event: {
        type: 'transaction',
        timestamp: new Date().toISOString(),
        user_id: 'user_ts_123',
        amount: 1500,
        currency: 'USD',
        ip_address: '192.168.1.100',
      },
      user: {
        account_age_days: 120,
        email_verified: true,
      },
      options: {
        enable_trace: true,
      },
    };

    const decisionRequest = buildDecisionRequest(apiRequest);
    const responseJson = await engine.decide(JSON.stringify(decisionRequest));
    const response: DecisionResponse = JSON.parse(responseJson);
    const decision = response.result?.signal?.type ?? 'pass';

    console.log('Example 1 - High Amount Transaction:');
    console.log('Decision:', decision.toUpperCase());
    console.log('Actions:', response.result?.actions ?? []);
    console.log(JSON.stringify(response, null, 2));
    console.log('');
  } catch (error) {
    console.error('Example 1 failed:', error instanceof Error ? error.message : error);
  }

  // Example 2: Pipeline with router and multiple rulesets
  try {
    const pipeline2 = `
version: "0.1"

---

rule:
  id: vip_user_rule
  name: VIP User Check
  when:
    all:
      - event.user_type == "vip"
  score: 100

---

rule:
  id: high_risk_rule
  name: High Risk Check
  when:
    any:
      - event.amount > 10000
      - event.risk_score > 80
  score: 80

---

rule:
  id: normal_user_rule
  name: Normal User Check
  when:
    all:
      - event.amount <= 10000
      - event.risk_score <= 80
  score: 10

---

ruleset:
  id: vip_ruleset
  name: VIP Ruleset
  rules:
    - vip_user_rule
  conclusion:
    - when: triggered_rules contains "vip_user_rule"
      signal: approve
      actions: ["allow"]
      reason: "VIP user approved"
    - default: true
      signal: pass
      reason: "Not a VIP user"

---

ruleset:
  id: risk_ruleset
  name: Risk Assessment Ruleset
  rules:
    - high_risk_rule
    - normal_user_rule
  conclusion:
    - when: triggered_rules contains "high_risk_rule"
      signal: review
      actions: ["manual_review"]
      reason: "High risk transaction requires review"
    - when: triggered_rules contains "normal_user_rule"
      signal: approve
      actions: ["allow"]
      reason: "Normal transaction approved"
    - default: true
      signal: pass
      reason: "No risk decision"

---

pipeline:
  id: risk_routing
  name: Risk-based Routing
  entry: risk_router

  when:
    all:
      - event.user_id

  steps:
    - step:
        id: risk_router
        name: Risk Router
        type: router
        routes:
          - next: vip_check
            when:
              all:
                - event.user_type == "vip"
        default: risk_assessment

    - step:
        id: vip_check
        name: VIP Check
        type: ruleset
        ruleset: vip_ruleset
        next: end

    - step:
        id: risk_assessment
        name: Risk Assessment
        type: ruleset
        ruleset: risk_ruleset
        next: end

  decision:
    - when: results.vip_ruleset.signal == "approve"
      result: approve
      actions: ["allow"]
      reason: "{results.vip_ruleset.reason}"
    - when: results.risk_ruleset.signal == "review"
      result: review
      actions: ["manual_review"]
      reason: "{results.risk_ruleset.reason}"
    - when: results.risk_ruleset.signal == "approve"
      result: approve
      actions: ["allow"]
      reason: "{results.risk_ruleset.reason}"
    - default: true
      result: pass
      reason: "No decision"
`;

    const engine2 = await Engine.fromYaml('risk_routing', pipeline2);

    const normalRequest: ApiRequest = {
      event: {
        type: 'transaction',
        timestamp: new Date().toISOString(),
        user_id: 'user_789',
        amount: 500,
        currency: 'USD',
        user_type: 'normal',
        risk_score: 25,
      },
      user: {
        account_age_days: 90,
        email_verified: true,
      },
      metadata: {
        request_id: 'req_ts_002',
      },
      options: {
        enable_trace: false,
      },
    };

    const decisionRequest2 = buildDecisionRequest(normalRequest);
    const response2Json = await engine2.decide(JSON.stringify(decisionRequest2));
    const response2: DecisionResponse = JSON.parse(response2Json);
    const decision2 = response2.result?.signal?.type ?? 'pass';

    console.log('Example 2 - Normal User Transaction:');
    console.log('Decision:', decision2.toUpperCase());
    console.log('Actions:', response2.result?.actions ?? []);
    console.log(JSON.stringify(response2, null, 2));
    console.log('');
  } catch (error) {
    console.error('Example 2 failed:', error instanceof Error ? error.message : error);
  }

  // Example 3: Using full decision request with tracing
  try {
    const pipeline3 = `
version: "0.1"

---

rule:
  id: small_payment_rule
  name: Small Payment Rule
  when:
    all:
      - event.amount < 5000
  score: 10

---

rule:
  id: large_payment_rule
  name: Large Payment Rule
  when:
    all:
      - event.amount >= 5000
  score: 80

---

ruleset:
  id: payment_ruleset
  name: Payment Ruleset
  rules:
    - small_payment_rule
    - large_payment_rule
  conclusion:
    - when: triggered_rules contains "large_payment_rule"
      signal: review
      actions: ["manual_review"]
      reason: "Large payment needs review"
    - when: triggered_rules contains "small_payment_rule"
      signal: approve
      actions: ["allow"]
      reason: "Small payment approved"
    - default: true
      signal: pass
      reason: "No payment decision"

---

pipeline:
  id: payment_verification
  name: Payment Verification Pipeline
  entry: verify_payment

  when:
    all:
      - event.amount > 0

  steps:
    - step:
        id: verify_payment
        name: Verify Payment
        type: ruleset
        ruleset: payment_ruleset
        next: end

  decision:
    - when: results.payment_ruleset.signal == "review"
      result: review
      actions: ["manual_review"]
      reason: "{results.payment_ruleset.reason}"
    - when: results.payment_ruleset.signal == "approve"
      result: approve
      actions: ["allow"]
      reason: "{results.payment_ruleset.reason}"
    - default: true
      result: pass
      reason: "No decision"
`;

    const engine3 = await Engine.fromYaml('payment_verification', pipeline3);

    const apiRequest3: ApiRequest = {
      event: {
        type: 'payment',
        timestamp: new Date().toISOString(),
        user_id: 'user_456',
        amount: 250,
        currency: 'USD',
      },
      user: {
        account_age_days: 30,
        email_verified: false,
      },
      metadata: {
        request_id: 'req_ts_001',
        source: 'typescript_example',
      },
      options: {
        enable_trace: true,
      },
    };

    const decisionRequest3 = buildDecisionRequest(apiRequest3);
    const response3Json = await engine3.decide(JSON.stringify(decisionRequest3));
    const response3: DecisionResponse = JSON.parse(response3Json);
    const decision3 = response3.result?.signal?.type ?? 'pass';

    console.log('Example 3 - With Full Request and Tracing:');
    console.log('Decision:', decision3.toUpperCase());
    console.log('Actions:', response3.result?.actions ?? []);
    console.log('Has Trace:', !!response3.trace);
    console.log(JSON.stringify(response3, null, 2));
    console.log('');
  } catch (error) {
    console.error('Example 3 failed:', error instanceof Error ? error.message : error);
  }

  console.log('✅ All examples completed!');
}

main().catch(console.error);
