package main

import (
	"fmt"
	"log"

	"github.com/corint/corint-go"
)

func main() {
	// Print version
	fmt.Printf("CORINT Version: %s\n", corint.Version())

	// Initialize logging
	corint.InitLogging()

	// Create engine with file system repository
	// Assumes 'repository' directory exists in current working directory
	engine, err := corint.NewEngine("repository")
	if err != nil {
		log.Fatalf("Failed to create engine: %v", err)
	}
	defer engine.Close()

	// Create a decision request
	request := &corint.DecisionRequest{
		EventData: map[string]interface{}{
			"user_id": "user123",
			"email":   "test@example.com",
			"amount":  1000.0,
			"ip":      "192.168.1.1",
		},
		Options: corint.DecisionOptions{
			EnableTrace: true,
		},
	}

	// Execute decision
	response, err := engine.Decide(request)
	if err != nil {
		log.Fatalf("Decision failed: %v", err)
	}

	// Print results
	fmt.Printf("Decision: %s\n", response.Decision)
	fmt.Printf("Actions: %v\n", response.Actions)

	if response.Trace != nil {
		fmt.Println("Execution trace available")
	}

	fmt.Println("Done!")
}
