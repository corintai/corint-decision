const { Engine, version } = require('./index.js');

async function test() {
  console.log('Version:', version());

  try {
    // Test with repository path (absolute path to the repository)
    const engine = await Engine.fromRepository('/Users/bryanzh/Workspace/corint/corint-decision/repository');
    console.log('✓ Engine loaded from repository');

    const result = await engine.decideSimple(JSON.stringify({
      type: 'payment',
      payment_amount: 1500,
      ip_address: '192.168.1.1'
    }));

    console.log('\nDecision result:');
    const parsed = JSON.parse(result);
    console.log('  Decision:', parsed.decision);
    console.log('  Outcomes:', parsed.outcomes);
    console.log('  Actions:', parsed.actions);
  } catch (error) {
    console.error('✗ Error:', error.message);
  }
}

test();
