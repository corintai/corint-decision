# OpenTelemetry Quick Start Guide

## Method 1: Quick Test (Recommended)

### 1. Start the Server
```bash
# Start server (OpenTelemetry is already integrated)
cargo run -p corint-server
```

### 2. Run the Test Script
```bash
# Send test requests and view metrics
./scripts/test_metrics.sh
```

### 3. View in Browser
Open your browser and visit:
- Health check: http://localhost:8080/health
- Metrics: http://localhost:8080/metrics

---

## Method 2: Manual Testing

### 1. Start the Server
```bash
cargo run -p corint-server
```

### 2. Send a Decision Request
```bash
curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "user_id": "test_user",
      "amount": 1500,
      "transaction_type": "purchase"
    }
  }'
```

### 3. View Metrics
```bash
curl http://localhost:8080/metrics
```

You should see something like:
```
# Metrics available through PrometheusExporter
```

---

## Method 3: Run Example Code

```bash
# Run the OpenTelemetry metrics example
cargo run --example opentelemetry_metrics --features otel --package corint-runtime
```

Example output:
```
🚀 OpenTelemetry Metrics Example
================================

✓ OpenTelemetry initialized with Prometheus exporter

📊 Recording metrics...

  Request 1: latency=0.100s
  Request 2: latency=0.150s
  ...

📈 Prometheus Metrics Output:
============================
# Metrics available through PrometheusExporter
```

---

## Advanced Configuration

### 1. Enable Distributed Tracing

```bash
# Set OTLP endpoint (requires Jaeger running first)
export OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4317"

# Start server
cargo run -p corint-server
```

### 2. Configure Prometheus Scraping

Create `prometheus.yml`:
```yaml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'corint-decision'
    static_configs:
      - targets: ['localhost:8080']
```

Start Prometheus:
```bash
docker run -d \
  -p 9090:9090 \
  -v $(pwd)/prometheus.yml:/etc/prometheus/prometheus.yml \
  prom/prometheus
```

Visit http://localhost:9090 to view Prometheus UI

### 3. Start Jaeger (for Tracing)

```bash
docker run -d \
  -p 16686:16686 \
  -p 4317:4317 \
  jaegertracing/all-in-one:latest
```

Visit http://localhost:16686 to view Jaeger UI

---

## Using in Your Code

### Recording Custom Metrics

```rust
use corint_runtime::observability::otel::meter;
use opentelemetry::KeyValue;

// Get a meter
let meter = meter("my-app");

// Create a counter
let counter = meter.u64_counter("my_requests_total").build();
counter.add(1, &[KeyValue::new("status", "success")]);

// Create a histogram
let histogram = meter.f64_histogram("my_duration_seconds").build();
histogram.record(0.123, &[]);
```

### Creating Spans (Tracing)

```rust
use corint_runtime::observability::otel::tracer;
use opentelemetry::trace::Tracer;

let tracer = tracer("my-tracer");
let span = tracer.start("my_operation");

// Your code here...

span.end();
```

---

## Verification

### Check Server Logs
After starting the server, you should see:
```
✓ OpenTelemetry initialized
service_name="corint-server" metrics_enabled=true tracing_enabled=true
Prometheus metrics exporter initialized
```

### Check Metrics Endpoint
```bash
curl http://localhost:8080/metrics
```

Should return:
```
# Metrics available through PrometheusExporter
```

Or actual metrics data (if custom metrics are present)

---

## Troubleshooting

### Issue 1: Server starts but no /metrics endpoint

**Solution:**
Ensure you're using the latest code:
```bash
cargo clean
cargo build --release
cargo run -p corint-server
```

### Issue 2: Metrics display is empty

**Reason:** The default PrometheusExporter doesn't automatically expose internal metrics

**Solutions:**
1. Send some decision requests to generate business metrics
2. Or add custom metrics in your code (see examples above)

### Issue 3: OTLP tracing not working

**Check:**
1. Is Jaeger running? `docker ps | grep jaeger`
2. Is the endpoint correct? Should be `http://localhost:4317`
3. Check server logs for errors

---

## Next Steps

1. **Integrate Prometheus** - Configure Prometheus to scrape metrics
2. **Create Dashboards** - Build visualization dashboards in Grafana
3. **Set Up Alerts** - Configure alert rules in Prometheus
4. **Add Business Metrics** - Add custom metrics in your decision logic
5. **Distributed Tracing** - Integrate Jaeger to trace request chains

---

## Reference Documentation

- Complete documentation: `crates/corint-runtime/README.md` (Observability section)
- OpenTelemetry official: https://opentelemetry.io/docs/
- Prometheus documentation: https://prometheus.io/docs/
- Jaeger documentation: https://www.jaegertracing.io/docs/
