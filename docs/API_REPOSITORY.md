# API Repository Configuration Guide

This document explains how to configure CORINT Decision Engine to load rules and pipelines from a remote HTTP API.

## Overview

The API repository allows you to:
- Load rules and pipelines from a remote server
- Centrally manage decision artifacts across multiple instances
- Enable dynamic updates without redeployment
- Secure access with API key authentication

## Configuration

### Server Configuration

Add the following to your `config/server.yaml`:

```yaml
repository:
  type: api
  base_url: "https://api.example.com/rules"
  api_key: "your-api-key-here"  # Optional
```

### Environment Variables

You can also configure via environment variables:

```bash
export CORINT_REPOSITORY__TYPE=api
export CORINT_REPOSITORY__BASE_URL=https://api.example.com/rules
export CORINT_REPOSITORY__API_KEY=your-api-key-here
```

## API Endpoints

Your API server must implement the following endpoints:

### 1. Manifest Endpoint

**GET** `/manifest`

Returns a JSON manifest listing all available artifacts.

**Response Format:**

```json
{
  "registry": "https://api.example.com/rules/registry.yaml",
  "pipelines": [
    {
      "id": "fraud_detection_pipeline",
      "url": "https://api.example.com/rules/pipelines/fraud_detection.yaml",
      "description": "Production fraud detection pipeline"
    },
    {
      "id": "payment_pipeline",
      "url": "https://api.example.com/rules/pipelines/payment.yaml",
      "description": "Payment processing pipeline"
    }
  ],
  "rulesets": [
    {
      "id": "fraud_detection_core",
      "url": "https://api.example.com/rules/rulesets/fraud_detection_core.yaml",
      "description": "Core fraud detection rules"
    }
  ],
  "rules": [
    {
      "id": "velocity_check",
      "url": "https://api.example.com/rules/rules/velocity_check.yaml",
      "description": "Transaction velocity check"
    }
  ]
}
```

**Authentication:**
- If `api_key` is configured, the request includes: `Authorization: Bearer {api_key}`

### 2. Registry Endpoint

**GET** `/registry.yaml` (or custom path from manifest)

Returns the pipeline registry YAML file.

**Example Response:**

```yaml
version: "0.1"

registry:
  - pipeline: fraud_detection_pipeline
    when:
      event.type: transaction

  - pipeline: payment_pipeline
    when:
      event.type: payment
```

### 3. Artifact Endpoints

**GET** `/pipelines/{id}.yaml`
**GET** `/rulesets/{id}.yaml`
**GET** `/rules/{id}.yaml`

Returns the YAML content for the specified artifact.

**Example Response (Pipeline):**

```yaml
version: "0.1"

imports:
  rulesets:
    - fraud_detection_core

---

pipeline:
  id: fraud_detection_pipeline
  name: Fraud Detection Pipeline
  when:
    event.type: transaction
  steps:
    - include:
        ruleset: fraud_detection_core
```

## Example API Server Implementation

### Node.js/Express Example

```javascript
const express = require('express');
const fs = require('fs').promises;
const path = require('path');

const app = express();
const RULES_DIR = './repository';
const API_KEY = process.env.API_KEY || 'your-secret-key';

// Authentication middleware
function authenticate(req, res, next) {
  const auth = req.headers.authorization;
  if (auth && auth.startsWith('Bearer ')) {
    const token = auth.slice(7);
    if (token === API_KEY) {
      return next();
    }
  }
  res.status(401).json({ error: 'Unauthorized' });
}

// Manifest endpoint
app.get('/manifest', authenticate, async (req, res) => {
  try {
    const manifest = {
      registry: `${req.protocol}://${req.get('host')}/registry.yaml`,
      pipelines: [
        {
          id: 'fraud_detection_pipeline',
          url: `${req.protocol}://${req.get('host')}/pipelines/fraud_detection.yaml`,
          description: 'Fraud detection pipeline'
        }
      ],
      rulesets: [],
      rules: []
    };
    res.json(manifest);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Registry endpoint
app.get('/registry.yaml', authenticate, async (req, res) => {
  try {
    const content = await fs.readFile(
      path.join(RULES_DIR, 'registry.yaml'),
      'utf8'
    );
    res.type('text/yaml').send(content);
  } catch (error) {
    res.status(404).json({ error: 'Registry not found' });
  }
});

// Pipeline endpoint
app.get('/pipelines/:id.yaml', authenticate, async (req, res) => {
  try {
    const content = await fs.readFile(
      path.join(RULES_DIR, 'pipelines', `${req.params.id}.yaml`),
      'utf8'
    );
    res.type('text/yaml').send(content);
  } catch (error) {
    res.status(404).json({ error: 'Pipeline not found' });
  }
});

app.listen(3000, () => {
  console.log('API server running on port 3000');
});
```

### Python/Flask Example

```python
from flask import Flask, jsonify, request, send_file
from pathlib import Path
import os

app = Flask(__name__)
RULES_DIR = Path('./repository')
API_KEY = os.environ.get('API_KEY', 'your-secret-key')

def authenticate():
    auth_header = request.headers.get('Authorization', '')
    if auth_header.startswith('Bearer '):
        token = auth_header[7:]
        if token == API_KEY:
            return True
    return False

@app.route('/manifest')
def manifest():
    if not authenticate():
        return jsonify({'error': 'Unauthorized'}), 401

    base_url = request.url_root.rstrip('/')
    return jsonify({
        'registry': f'{base_url}/registry.yaml',
        'pipelines': [
            {
                'id': 'fraud_detection_pipeline',
                'url': f'{base_url}/pipelines/fraud_detection.yaml',
                'description': 'Fraud detection pipeline'
            }
        ],
        'rulesets': [],
        'rules': []
    })

@app.route('/registry.yaml')
def registry():
    if not authenticate():
        return jsonify({'error': 'Unauthorized'}), 401

    file_path = RULES_DIR / 'registry.yaml'
    if not file_path.exists():
        return jsonify({'error': 'Registry not found'}), 404

    return send_file(file_path, mimetype='text/yaml')

@app.route('/pipelines/<pipeline_id>.yaml')
def pipeline(pipeline_id):
    if not authenticate():
        return jsonify({'error': 'Unauthorized'}), 401

    file_path = RULES_DIR / 'pipelines' / f'{pipeline_id}.yaml'
    if not file_path.exists():
        return jsonify({'error': 'Pipeline not found'}), 404

    return send_file(file_path, mimetype='text/yaml')

if __name__ == '__main__':
    app.run(port=3000)
```

## Security Considerations

### 1. Use HTTPS in Production

```yaml
repository:
  type: api
  base_url: "https://api.example.com/rules"  # Always use HTTPS
```

### 2. Implement Rate Limiting

Protect your API from excessive requests:

```javascript
const rateLimit = require('express-rate-limit');

const limiter = rateLimit({
  windowMs: 15 * 60 * 1000, // 15 minutes
  max: 100 // limit each IP to 100 requests per windowMs
});

app.use('/manifest', limiter);
```

### 3. Rotate API Keys Regularly

Store API keys securely:

```bash
# Use environment variables
export CORINT_REPOSITORY__API_KEY=$(cat /run/secrets/api_key)

# Or use a secret management service
export CORINT_REPOSITORY__API_KEY=$(aws secretsmanager get-secret-value --secret-id corint-api-key --query SecretString --output text)
```

### 4. Validate Artifact Content

Server-side validation:

```javascript
const yaml = require('js-yaml');

app.get('/pipelines/:id.yaml', authenticate, async (req, res) => {
  try {
    const content = await fs.readFile(filePath, 'utf8');

    // Validate YAML syntax
    const parsed = yaml.load(content);

    // Validate required fields
    if (!parsed.pipeline || !parsed.pipeline.id) {
      return res.status(400).json({ error: 'Invalid pipeline format' });
    }

    res.type('text/yaml').send(content);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});
```

### 5. Audit Logging

Log all API access:

```javascript
app.use((req, res, next) => {
  console.log(`${new Date().toISOString()} - ${req.method} ${req.path} - ${req.ip}`);
  next();
});
```

## Testing

### 1. Test with curl

```bash
# Test manifest endpoint
curl -H "Authorization: Bearer your-api-key" \
  https://api.example.com/rules/manifest

# Test pipeline endpoint
curl -H "Authorization: Bearer your-api-key" \
  https://api.example.com/rules/pipelines/fraud_detection.yaml
```

### 2. Test with CORINT Server

```bash
# Configure API repository
cat > config/server.yaml <<EOF
repository:
  type: api
  base_url: "https://api.example.com/rules"
  api_key: "your-api-key"
EOF

# Start server and check logs
cargo run --package corint-server
```

Expected log output:

```
INFO corint_server: Loading rules from API repository: https://api.example.com/rules
INFO corint_server: Fetching repository manifest from API
INFO corint_server:   Using API key authentication
INFO corint_server:   Found 2 pipelines in manifest
INFO corint_server:   Loading registry from: https://api.example.com/rules/registry.yaml
INFO corint_server:   Loading pipeline: fraud_detection_pipeline
INFO corint_server: ✓ Successfully loaded 2 pipelines from API
```

## Troubleshooting

### Connection Refused

**Error:** `Failed to fetch manifest: connection refused`

**Solution:**
- Check API server is running
- Verify `base_url` is correct
- Ensure network connectivity

### Authentication Failed

**Error:** `API returned error status: 401`

**Solution:**
- Verify API key is correct
- Check Authorization header format
- Ensure API key hasn't expired

### Timeout

**Error:** `Failed to fetch manifest: timeout`

**Solution:**
- Increase timeout (default: 30s)
- Check API server performance
- Verify network latency

### Invalid YAML

**Error:** `Failed to parse manifest: invalid JSON`

**Solution:**
- Validate manifest JSON format
- Check Content-Type headers
- Verify API response format

## Advanced Usage

### Custom Manifest Structure

You can extend the manifest with custom metadata:

```json
{
  "version": "1.0",
  "registry": "https://api.example.com/rules/registry.yaml",
  "pipelines": [
    {
      "id": "fraud_detection_pipeline",
      "url": "https://api.example.com/rules/pipelines/fraud_detection.yaml",
      "description": "Fraud detection pipeline",
      "version": "2.1.0",
      "tags": ["fraud", "production"],
      "updated_at": "2025-12-11T10:00:00Z"
    }
  ]
}
```

### Versioning Support

Implement version-specific endpoints:

```
GET /v1/manifest
GET /v2/manifest
GET /pipelines/fraud_detection/v2.1.0.yaml
```

### Caching Strategy

Client-side caching with ETag:

```javascript
app.get('/pipelines/:id.yaml', authenticate, async (req, res) => {
  const content = await fs.readFile(filePath, 'utf8');
  const etag = crypto.createHash('md5').update(content).digest('hex');

  res.set('ETag', etag);
  res.set('Cache-Control', 'max-age=300'); // 5 minutes

  if (req.headers['if-none-match'] === etag) {
    return res.status(304).end();
  }

  res.type('text/yaml').send(content);
});
```

## Comparison with Other Repository Types

| Feature | FileSystem | Database | API |
|---------|-----------|----------|-----|
| Setup Complexity | Low | Medium | Medium |
| Runtime Updates | ❌ No | ✅ Yes | ✅ Yes |
| Network Required | ❌ No | ✅ Yes | ✅ Yes |
| Centralized Management | ❌ No | ✅ Yes | ✅ Yes |
| Version Control (Git) | ✅ Yes | ❌ No | ⚠️ Optional |
| Multi-Instance Sync | ❌ No | ✅ Yes | ✅ Yes |
| Offline Support | ✅ Yes | ❌ No | ❌ No |
| Latency | <1ms | 10-30ms | 50-200ms |
| Best For | Development | Production | Multi-Region |

## Next Steps

1. ✅ Implement API server following the spec above
2. ✅ Configure CORINT server to use API repository
3. ✅ Test manifest and artifact endpoints
4. ✅ Deploy to production with HTTPS and authentication
5. ⚠️ Monitor API performance and caching

## Related Documentation

- [Server Configuration](../config/server.yaml)
- [Repository Types](../docs/REPOSITORY_TYPES.md)
- [Database Repository](../crates/corint-repository/README.md)
- [Security Best Practices](../docs/SECURITY.md)
