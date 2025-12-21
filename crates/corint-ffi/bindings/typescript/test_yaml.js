const { Engine, version } = require('./index.js');
const fs = require('fs');

async function test() {
  console.log('Version:', version());

  const yamlContent = fs.readFileSync('test_simple.yaml', 'utf8');
  console.log('\nYAML Content:');
  console.log(yamlContent);

  try {
    const engine = await Engine.fromYaml('test_simple', yamlContent);
    console.log('\n✓ Engine created successfully!');

    const result = await engine.decideSimple(JSON.stringify({ amount: 150 }));
    console.log('\nDecision result:', JSON.parse(result));
  } catch (error) {
    console.error('\n✗ Error:', error.message);
  }
}

test();
