// TypeScript example for CORINT Decision Engine

import { Engine, version } from './index.js';

interface EventData {
  user_id: string;
  amount: number;
  transaction_currency?: string;
  ip_address?: string;
  timestamp?: string;
}

interface DecisionResponse {
  request_id: string;
  pipeline_id: string;
  result: {
    action: string | null;
    score: number;
    triggered_rules: string[];
    explanation: string;
    context: Record<string, any>;
  };
  processing_time_ms: number;
  metadata: Record<string, string>;
}

async function main() {
  console.log('CORINT Decision Engine Version:', version());

  // Example 1: Simple pipeline with inline rules
  try {
    const pipelineYaml = `
version: "1.0"

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
  
  decision_logic:
    - condition: total_score >= 50
      action: review
      reason: "Suspicious user behavior detected"

    - default: true
      action: approve
      reason: "User behavior normal"
---

pipeline:
  id: typescript_fraud_check
  name: TypeScript Fraud Check
  description: Simple fraud detection pipeline
  entry: amount_check

  when:
    all:
      - event.amount > 0

  steps:
    - step:
        id: amount_check
        name: Amount Check
        type: ruleset
        ruleset: amount_check_ruleset
        next: end
`;

    const engine = await Engine.fromYaml('typescript_fraud_check', pipelineYaml);
    console.log('✓ Engine loaded from YAML content\n');

    // Test with high amount
    const eventData: EventData = {
      user_id: 'user_ts_123',
      amount: 1500,
      transaction_currency: 'USD',
      ip_address: '192.168.1.100',
      timestamp: new Date().toISOString(),
    };

    const responseJson = await engine.decideSimple(JSON.stringify(eventData));
    const response: DecisionResponse = JSON.parse(responseJson);

    console.log('Example 1 - High Amount Transaction:');
    console.log(JSON.stringify(response, null, 2));
    console.log('');
  } catch (error) {
    console.error('Example 1 failed:', error instanceof Error ? error.message : error);
  }

  // Example 2: Pipeline with router and multiple rulesets
  try {
    const pipeline2 = `
version: "1.0"

---

rule:
  id: vip_user_rule
  name: VIP User Check
  when:
    all:
      - event.user_type == "vip"
  outcomes:
    - vip_approved
  actions:
    - type: allow
      reason: "VIP user approved"

---

rule:
  id: high_risk_rule
  name: High Risk Check
  when:
    any:
      - event.amount > 10000
      - event.risk_score > 80
  outcomes:
    - high_risk_detected
  actions:
    - type: review
      reason: "High risk transaction requires review"

---

rule:
  id: normal_user_rule
  name: Normal User Check
  when:
    all:
      - event.amount <= 10000
      - event.risk_score <= 80
  outcomes:
    - normal_approved
  actions:
    - type: allow
      reason: "Normal transaction approved"

---

ruleset:
  id: vip_ruleset
  name: VIP Ruleset
  rules:
    - vip_user_rule

---

ruleset:
  id: risk_ruleset
  name: Risk Assessment Ruleset
  rules:
    - high_risk_rule
    - normal_user_rule

---

pipeline:
  id: risk_routing
  name: Risk-based Routing
  entry: risk_router

  when:
    all:
      - event.user_id != null

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
`;

    const engine2 = await Engine.fromYaml('risk_routing', pipeline2);

    const normalEvent = {
      user_id: 'user_789',
      user_type: 'normal',
      amount: 500,
      risk_score: 25,
    };

    const response2Json = await engine2.decideSimple(JSON.stringify(normalEvent));
    const response2: DecisionResponse = JSON.parse(response2Json);

    console.log('Example 2 - Normal User Transaction:');
    console.log(JSON.stringify(response2, null, 2));
    console.log('');
  } catch (error) {
    console.error('Example 2 failed:', error instanceof Error ? error.message : error);
  }

  // Example 3: Using full decision request with tracing
  try {
    const pipeline3 = `
version: "1.0"

---

rule:
  id: small_payment_rule
  name: Small Payment Rule
  when:
    all:
      - event.payment_amount < 5000
  outcomes:
    - small_payment
  actions:
    - type: allow
      reason: "Small payment approved"

---

rule:
  id: large_payment_rule
  name: Large Payment Rule
  when:
    all:
      - event.payment_amount >= 5000
  outcomes:
    - large_payment
  actions:
    - type: review
      reason: "Large payment needs review"

---

ruleset:
  id: payment_ruleset
  name: Payment Ruleset
  rules:
    - small_payment_rule
    - large_payment_rule

---

pipeline:
  id: payment_verification
  name: Payment Verification Pipeline
  entry: verify_payment

  when:
    all:
      - event.payment_amount > 0

  steps:
    - step:
        id: verify_payment
        name: Verify Payment
        type: ruleset
        ruleset: payment_ruleset
        next: end
`;

    const engine3 = await Engine.fromYaml('payment_verification', pipeline3);

    const fullRequest = {
      event_data: {
        user_id: 'user_456',
        payment_amount: 250,
      },
      features: {
        user_risk_score: 0.75,
        account_age_days: 30,
      },
      metadata: {
        request_id: 'req_ts_001',
        source: 'typescript_example',
      },
      options: {
        enable_trace: true,
      },
    };

    const response3Json = await engine3.decide(JSON.stringify(fullRequest));
    const response3: DecisionResponse = JSON.parse(response3Json);

    console.log('Example 3 - With Full Request and Tracing:');
    console.log(JSON.stringify(response3, null, 2));
    console.log('');
  } catch (error) {
    console.error('Example 3 failed:', error instanceof Error ? error.message : error);
  }

  console.log('✅ All examples completed!');
}

main().catch(console.error);
