// Example usage of CORINT Decision Engine Node.js bindings

const { Engine, version } = require('./index.js');

async function main() {
  console.log('CORINT Decision Engine Version:', version());

  // Example 1: Load engine from repository
  try {
    const engine = await Engine.fromRepository('../../../../../repository');
    console.log('✓ Engine loaded from repository');

    // Prepare decision request
    const eventData = {
      user_id: 'user_123',
      transaction_amount: 5000,
      transaction_currency: 'USD',
      ip_address: '192.168.1.1',
      timestamp: new Date().toISOString(),
    };

    // Execute decision using simple API
    const responseJson = await engine.decideSimple(JSON.stringify(eventData));
    const response = JSON.parse(responseJson);

    console.log('\nDecision Response:');
    console.log('  Decision:', response.decision);
    console.log('  Outcomes:', response.outcomes);
    console.log('  Actions:', response.actions);
  } catch (error) {
    console.error('Example 1 failed:', error.message);
  }

  // Example 2: Load engine from YAML content
  try {
    const pipelineYaml = `
version: 1.0

metadata:
  id: simple_fraud_check
  name: Simple Fraud Check
  description: Basic fraud detection pipeline

when:
  - event_data.transaction_amount > 0

steps:
  - step: check_amount
    type: rule
    rule:
      name: High Amount Check
      when:
        - event_data.transaction_amount > 1000
      actions:
        - type: block
          reason: "High transaction amount"
`;

    const engine2 = await Engine.fromYaml('simple_fraud_check', pipelineYaml);
    console.log('\n✓ Engine loaded from YAML content');

    const eventData2 = {
      transaction_amount: 2500,
    };

    const response2Json = await engine2.decideSimple(JSON.stringify(eventData2));
    const response2 = JSON.parse(response2Json);

    console.log('\nDecision Response 2:');
    console.log('  Decision:', response2.decision);
    console.log('  Actions:', response2.actions);
  } catch (error) {
    console.error('Example 2 failed:', error.message);
  }

  // Example 3: Using full decision request format
  try {
    const engine3 = await Engine.fromYaml('test_pipeline', `
version: 1.0

metadata:
  id: feature_test
  name: Feature Test Pipeline

when:
  - event_data.user_id != null

steps:
  - step: analyze
    type: rule
    rule:
      name: User Analysis
      when:
        - event_data.transaction_count > 10
      actions:
        - type: flag
          reason: "High activity user"
`);

    const fullRequest = {
      event_data: {
        user_id: 'user_456',
        transaction_count: 15,
      },
      features: {
        user_risk_score: 0.75,
      },
      metadata: {
        request_id: 'req_123',
        source: 'api',
      },
      options: {
        enable_trace: true,
      },
    };

    const response3Json = await engine3.decide(JSON.stringify(fullRequest));
    const response3 = JSON.parse(response3Json);

    console.log('\nDecision Response 3:');
    console.log('  Decision:', response3.decision);
    console.log('  Has Trace:', !!response3.trace);
  } catch (error) {
    console.error('Example 3 failed:', error.message);
  }
}

main().catch(console.error);
