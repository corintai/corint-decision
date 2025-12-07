#!/usr/bin/env python3
"""
CORINT Decision Engine Server API Test Script (Python)
"""

import requests
import json
import os
import sys

# Server configuration
SERVER_URL = os.getenv("CORINT_SERVER_URL", "http://localhost:8080")

def print_header(text):
    print("\n" + "=" * 60)
    print(f"  {text}")
    print("=" * 60)

def print_result(title, response):
    print(f"\n{title}")
    print(f"Status Code: {response.status_code}")
    try:
        print(json.dumps(response.json(), indent=2))
    except:
        print(response.text)

def test_health():
    """Test health check endpoint"""
    print_header("Test 1: Health Check")
    response = requests.get(f"{SERVER_URL}/health")
    print_result("Health Check Result:", response)
    return response.status_code == 200

def test_decide_user_001():
    """Test decision endpoint with user_001"""
    print_header("Test 2: Make Decision (user_001)")
    
    payload = {
        "event_data": {
            "user_id": "user_001",
            "device_id": "device_001",
            "ip_address": "203.0.113.1",
            "event.type": "transaction",
            "event.user_id": "user_001",
            "event.device_id": "device_001",
            "event.ip_address": "203.0.113.1",
            "event.event_type": "transaction"
        }
    }
    
    response = requests.post(f"{SERVER_URL}/v1/decide", json=payload)
    print_result("Decision Result:", response)
    
    if response.status_code == 200:
        result = response.json()
        print(f"\n✓ Decision: {result.get('action')}")
        print(f"  Score: {result.get('score')}")
        print(f"  Triggered Rules: {result.get('triggered_rules')}")
        print(f"  Processing Time: {result.get('processing_time_ms')}ms")
        return True
    return False

def test_decide_user_003():
    """Test decision endpoint with user_003 (higher risk)"""
    print_header("Test 3: Make Decision (user_003 - Higher Risk)")
    
    payload = {
        "event_data": {
            "user_id": "user_003",
            "device_id": "device_003",
            "ip_address": "203.0.113.3",
            "event.type": "transaction",
            "event.user_id": "user_003",
            "event.device_id": "device_003",
            "event.ip_address": "203.0.113.3",
            "event.event_type": "transaction"
        }
    }
    
    response = requests.post(f"{SERVER_URL}/v1/decide", json=payload)
    print_result("Decision Result:", response)
    
    if response.status_code == 200:
        result = response.json()
        print(f"\n✓ Decision: {result.get('action')}")
        print(f"  Score: {result.get('score')}")
        print(f"  Triggered Rules: {result.get('triggered_rules')}")
        print(f"  Processing Time: {result.get('processing_time_ms')}ms")
        return True
    return False


def main():
    print(f"\nCORINT Decision Engine API Tests")
    print(f"Server: {SERVER_URL}")
    
    # Check if server is reachable
    try:
        response = requests.get(f"{SERVER_URL}/health", timeout=5)
        print(f"✓ Server is reachable")
    except requests.exceptions.RequestException as e:
        print(f"✗ Cannot reach server: {e}")
        print(f"\nPlease make sure the server is running:")
        print(f"  cargo run --bin corint-server")
        sys.exit(1)
    
    # Run tests
    results = []
    results.append(("Health Check", test_health()))
    results.append(("Decision (user_001)", test_decide_user_001()))
    results.append(("Decision (user_003)", test_decide_user_003()))
    
    # Print summary
    print_header("Test Summary")
    for name, passed in results:
        status = "✓ PASSED" if passed else "✗ FAILED"
        print(f"  {status}: {name}")
    
    total = len(results)
    passed = sum(1 for _, p in results if p)
    print(f"\n  Total: {passed}/{total} tests passed")
    
    sys.exit(0 if passed == total else 1)

if __name__ == "__main__":
    main()

