#!/usr/bin/env python3
"""
Example usage of CORINT Decision Engine Python bindings

This example demonstrates the basic API usage. To run it with an actual
repository, you need to either:
1. Have a 'repository' directory in your current working directory, or
2. Set up a PostgreSQL database and use DecisionEngine.fromDatabase()
"""

from corint import DecisionEngine, DecisionRequest
import os

def main():
    # Print version and library path
    print(f"CORINT Version: {DecisionEngine.version()}")

    # Show which library is being used
    import sys
    sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))
    from corint.engine import _lib_path
    print(f"Using library: {_lib_path}")
    print()

    # Initialize logging (optional)
    # DecisionEngine.init_logging()

    # Example 1: Check if repository exists in current directory
    # Use the repository in the current working directory
    repo_path = os.path.join(os.getcwd(), "repository")

    if not os.path.exists(repo_path):
        print("NOTE: This is a demonstration of the Python API.")
        print(f"Repository not found at: {repo_path}")
        print()
        print("To run this example with a real repository:")
        print("1. Create a repository directory with your rules, or")
        print("2. Modify this script to use a database URL")
        print()
        print("Example API usage:")
        print("=" * 50)
        print("""
# Create engine with file system repository
engine = DecisionEngine(repository_path="path/to/repository")

# Or create engine with database
engine = DecisionEngine(database_url="postgresql://...")

# Create a decision request
request = DecisionRequest(
    event_data={
        "type": "test1",  # Required: must match a pipeline in registry.yaml
        "user_id": "user123",
        "email": "test@example.com",
        "amount": 1000.0,
        "ip": "192.168.1.1"
    },
    enable_trace=True
)

# Execute decision
response = engine.decide(request)

# Print results
print(f"Decision: {response.decision}")
print(f"Actions: {response.actions}")

# Clean up
engine.close()
        """)
        return

    # If repository exists, run the actual example
    print(f"Using repository: {repo_path}")
    print()

    try:
        with DecisionEngine(repository_path=repo_path) as engine:
            # Create a decision request
            # The event_data must include 'type' to match a pipeline in registry.yaml
            request = DecisionRequest(
                event_data={
                    "type": "test1",  # Matches comprehensive_risk_assessment pipeline
                    "user_id": "user123",
                    "email": "test@example.com",
                    "amount": 1000.0,
                    "ip": "192.168.1.1"
                },
                enable_trace=True
            )

            # Execute decision
            response = engine.decide(request)

            # Print raw response
            print("=" * 60)
            print("RAW RESPONSE:")
            print("=" * 60)
            import json
            print(json.dumps(response.to_dict(), indent=2, ensure_ascii=False))
            print("=" * 60)
            print()

            # Print results
            print(f"Decision: {response.decision or '(no decision - rules not triggered)'}")
            print(f"Actions: {response.actions}")
            print(f"Metadata: {response.metadata}")
            print()
            print("Note: Empty decision is normal - the test data doesn't trigger any rules.")
            print("Rules were evaluated successfully, as shown in the trace below.")

            if response.trace:
                print("\nPipeline executed:")
                pipeline_id = response.trace.get('pipeline', {}).get('pipeline_id')
                print(f"  - {pipeline_id}")

                steps = response.trace.get('pipeline', {}).get('steps', [])
                print(f"  - {len(steps)} step(s) executed")

                rulesets = response.trace.get('pipeline', {}).get('rulesets', [])
                for ruleset in rulesets:
                    rules = ruleset.get('rules', [])
                    triggered = sum(1 for r in rules if r.get('triggered'))
                    print(f"  - Ruleset '{ruleset.get('ruleset_id')}': {len(rules)} rules evaluated, {triggered} triggered")

            print()
            print("Done!")
    except Exception as e:
        print(f"Error: {e}")
        print()
        print("This might happen if the repository is empty or misconfigured.")


if __name__ == "__main__":
    main()
