/**
 * Example usage of CORINT Decision Engine TypeScript bindings
 */

import { DecisionEngine, DecisionRequest } from './src';

async function main() {
  // Print version
  console.log(`CORINT Version: ${DecisionEngine.version()}`);

  // Initialize logging
  DecisionEngine.initLogging();

  // Create engine with file system repository
  // Assumes 'repository' directory exists in current working directory
  const engine = new DecisionEngine({ repositoryPath: 'repository' });

  try {
    // Create a decision request
    const request: DecisionRequest = {
      event_data: {
        user_id: 'user123',
        email: 'test@example.com',
        amount: 1000.0,
        ip: '192.168.1.1',
      },
      options: {
        enableTrace: true,
      },
    };

    // Execute decision
    const response = engine.decide(request);

    // Print results
    console.log(`Decision: ${response.decision}`);
    console.log(`Actions: ${JSON.stringify(response.actions)}`);

    if (response.trace) {
      console.log('Execution trace available');
    }

    console.log('Done!');
  } finally {
    // Clean up
    engine.close();
  }
}

main().catch(console.error);
