// Example usage of CORINT Decision Engine Node.js bindings

const { Engine, version } = require('./index.js');
const path = require('path');

function buildDecisionRequest(apiRequest) {
  const eventData = { ...apiRequest.event };
  if (apiRequest.user) {
    eventData.user = apiRequest.user;
  }

  const request = {
    event_data: eventData,
    options: {
      enable_trace: Boolean(apiRequest.options && apiRequest.options.enable_trace),
    },
  };

  if (apiRequest.features) {
    request.features = apiRequest.features;
  }

  if (apiRequest.metadata) {
    request.metadata = apiRequest.metadata;
  }

  return request;
}

async function main() {
  console.log('CORINT Decision Engine Version:', version());

  // Example 1: Load engine from repository (event + user mapped to event_data)
  try {
    const repoPath = path.resolve(__dirname, '../../../../repository');
    const engine = await Engine.fromRepository(repoPath);
    console.log('✓ Engine loaded from repository');

    const apiRequest = {
      event: {
        type: 'login',
        timestamp: new Date().toISOString(),
        user_id: 'user_123',
        ip_address: '192.168.1.1',
        device_id: 'device_abc',
        device: {
          is_emulator: false,
          is_rooted: false,
          fingerprint_confidence: 1.0,
        },
        location: {
          country: 'US',
          city: 'San Francisco',
          latitude: 37.7749,
          longitude: -122.4194,
        },
      },
      user: {
        account_age_days: 365,
        email_verified: true,
        phone_verified: true,
        timezone: 'America/Los_Angeles',
      },
      options: {
        enable_trace: true,
      },
      // Precomputed features are optional in the SDK and avoid feature extractor setup.
      features: {
        failed_login_count_1h: 0,
        unique_devices_7d: 1,
        user_risk_score: 0.1,
        login_count_1h: 0,
        login_count_24h: 0,
        is_new_device: false,
        country_changed: false,
        is_off_hours: false,
      },
    };

    const decisionRequest = buildDecisionRequest(apiRequest);
    const responseJson = await engine.decide(JSON.stringify(decisionRequest));
    const response = JSON.parse(responseJson);
    const decision = response.result?.signal?.type ?? 'pass';

    console.log('\nDecision Response:');
    console.log('  Decision:', decision.toUpperCase());
    console.log('  Actions:', response.result?.actions ?? []);
  } catch (error) {
    console.error('Example 1 failed:', error.message);
  }

  // Example 2: Load engine from YAML content
  try {
    const pipelineYaml = `
version: "0.1"

---
rule:
  id: high_amount_rule
  name: High Amount Check
  when:
    all:
      - event.amount > 1000
  score: 100

---
ruleset:
  id: simple_amount_ruleset
  name: Simple Amount Ruleset
  rules:
    - high_amount_rule
  conclusion:
    - when: triggered_rules contains "high_amount_rule"
      signal: review
      reason: "High transaction amount"
    - default: true
      signal: approve
      reason: "Normal transaction amount"

---
pipeline:
  id: simple_fraud_check
  name: Simple Fraud Check
  entry: check_amount

  when:
    all:
      - event.type == "transaction"
      - event.amount > 0

  steps:
    - step:
        id: check_amount
        name: Amount Check
        type: ruleset
        ruleset: simple_amount_ruleset
        next: end

  decision:
    - when: results.simple_amount_ruleset.signal == "review"
      result: review
      actions: ["manual_review"]
      reason: "{results.simple_amount_ruleset.reason}"
    - when: results.simple_amount_ruleset.signal == "approve"
      result: approve
      reason: "{results.simple_amount_ruleset.reason}"
    - default: true
      result: pass
      reason: "No decision"
`;

    const engine2 = await Engine.fromYaml('simple_fraud_check', pipelineYaml);
    console.log('\n✓ Engine loaded from YAML content');

    const apiRequest2 = {
      event: {
        type: 'transaction',
        timestamp: new Date().toISOString(),
        user_id: 'user_456',
        amount: 2500,
        currency: 'USD',
      },
      user: {
        account_age_days: 45,
        email_verified: true,
      },
    };

    const decisionRequest2 = buildDecisionRequest(apiRequest2);
    const response2Json = await engine2.decide(JSON.stringify(decisionRequest2));
    const response2 = JSON.parse(response2Json);
    const decision2 = response2.result?.signal?.type ?? 'pass';

    console.log('\nDecision Response 2:');
    console.log('  Decision:', decision2.toUpperCase());
    console.log('  Actions:', response2.result?.actions ?? []);
  } catch (error) {
    console.error('Example 2 failed:', error.message);
  }

  // Example 3: Using full decision request format
  try {
    const engine3 = await Engine.fromYaml('feature_test', `
version: "0.1"

---
rule:
  id: activity_rule
  name: User Activity Check
  when:
    all:
      - event.transaction_count > 10
  score: 50

---
ruleset:
  id: activity_ruleset
  name: Activity Ruleset
  rules:
    - activity_rule
  conclusion:
    - when: triggered_rules contains "activity_rule"
      signal: review
      actions: ["flag_user"]
      reason: "High activity user"
    - default: true
      signal: approve
      reason: "Normal activity"

---
pipeline:
  id: feature_test
  name: Feature Test Pipeline
  entry: analyze

  when:
    all:
      - event.user_id

  steps:
    - step:
        id: analyze
        name: User Analysis
        type: ruleset
        ruleset: activity_ruleset
        next: end

  decision:
    - when: results.activity_ruleset.signal == "review"
      result: review
      actions: ["flag_user"]
      reason: "{results.activity_ruleset.reason}"
    - when: results.activity_ruleset.signal == "approve"
      result: approve
      reason: "{results.activity_ruleset.reason}"
    - default: true
      result: pass
      reason: "No decision"
`);

    const apiRequest3 = {
      event: {
        type: 'login',
        timestamp: new Date().toISOString(),
        user_id: 'user_789',
        transaction_count: 15,
      },
      user: {
        account_age_days: 12,
        email_verified: false,
      },
      metadata: {
        request_id: 'req_123',
        source: 'sdk_example',
      },
      options: {
        enable_trace: true,
      },
    };

    const decisionRequest3 = buildDecisionRequest(apiRequest3);
    const response3Json = await engine3.decide(JSON.stringify(decisionRequest3));
    const response3 = JSON.parse(response3Json);
    const decision3 = response3.result?.signal?.type ?? 'pass';

    console.log('\nDecision Response 3:');
    console.log('  Decision:', decision3.toUpperCase());
    console.log('  Actions:', response3.result?.actions ?? []);
    console.log('  Has Trace:', !!response3.trace);
  } catch (error) {
    console.error('Example 3 failed:', error.message);
  }
}

main().catch(console.error);
