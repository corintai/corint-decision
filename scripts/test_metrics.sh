#!/bin/bash

# Test OpenTelemetry metrics functionality

echo "🔍 Testing OpenTelemetry Metrics"
echo "================================"
echo ""

# 1. Check if server is running
echo "1. Checking if server is running..."
if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "   ✓ Server is running"
else
    echo "   ✗ Server is not running"
    echo "   Please start the server first: cargo run -p corint-server"
    exit 1
fi
echo ""

# 2. Send test requests (generate metrics data)
echo "2. Sending test requests to generate metrics..."
for i in {1..5}; do
    curl -s -X POST http://localhost:8080/v1/decide \
        -H "Content-Type: application/json" \
        -d '{
            "event_data": {
                "user_id": "user_'$i'",
                "amount": '$((RANDOM % 1000))',
                "transaction_type": "purchase"
            }
        }' > /dev/null
    echo "   Request $i sent"
done
echo "   ✓ Sent 5 test requests"
echo ""

# 3. Fetch metrics
echo "3. Fetching metrics from /metrics endpoint..."
echo "   URL: http://localhost:8080/metrics"
echo ""
curl -s http://localhost:8080/metrics
echo ""

# 4. Next steps
echo ""
echo "📊 Next Steps:"
echo "=============="
echo ""
echo "1. View metrics in browser:"
echo "   http://localhost:8080/metrics"
echo ""
echo "2. Set up Prometheus to scrape metrics:"
echo "   See: docs/QUICK_START_OTEL.md"
echo ""
echo "3. Configure distributed tracing (optional):"
echo "   export OTEL_EXPORTER_OTLP_ENDPOINT='http://localhost:4317'"
echo "   cargo run -p corint-server"
echo ""
echo "📚 Documentation:"
echo "   - Quick Start: docs/QUICK_START_OTEL.md"
echo "   - Full Docs: crates/corint-runtime/README.md (Observability section)"
echo ""
