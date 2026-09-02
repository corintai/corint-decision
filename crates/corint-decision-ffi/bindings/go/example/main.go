package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"runtime"
	"time"

	"github.com/corint/corint-go"
)

func repositoryPath() (string, error) {
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		return "", fmt.Errorf("unable to resolve example path")
	}

	baseDir := filepath.Dir(filename)
	repoPath := filepath.Join(baseDir, "../../../../..", "repository")
	return filepath.Abs(repoPath)
}

func main() {
	// Print version
	fmt.Printf("CORINT Version: %s\n", corint.Version())

	// Initialize logging
	corint.InitLogging()

	// Create engine with file system repository
	repoPath, err := repositoryPath()
	if err != nil {
		log.Fatalf("Failed to resolve repository path: %v", err)
	}
	if _, statErr := os.Stat(repoPath); statErr != nil {
		log.Fatalf("Repository not found at %s: %v", repoPath, statErr)
	}
	repoRoot := filepath.Dir(repoPath)
	if chdirErr := os.Chdir(repoRoot); chdirErr != nil {
		log.Fatalf("Failed to change directory to %s: %v", repoRoot, chdirErr)
	}

	engine, err := corint.NewEngine(repoPath)
	if err != nil {
		log.Fatalf("Failed to create engine: %v", err)
	}
	defer engine.Close()

	// Build request based on docs/API_REQUEST.md and docs/dsl/overall.md.
	// The SDK expects event fields under event_data; user profile is nested under event_data.user.
	request := &corint.DecisionRequest{
		EventData: map[string]interface{}{
			"type":       "login",
			"timestamp":  time.Now().UTC().Format(time.RFC3339),
			"user_id":    "user123",
			"ip_address": "192.168.1.1",
			"device_id":  "device_abc",
			"device": map[string]interface{}{
				"is_emulator":            false,
				"is_rooted":              false,
				"fingerprint_confidence": 1.0,
			},
			"user": map[string]interface{}{
				"account_age_days": 365,
				"email_verified":   true,
				"phone_verified":   true,
				"timezone":         "America/Los_Angeles",
			},
		},
		Features: map[string]interface{}{
			"failed_login_count_1h": 0,
			"unique_devices_7d":     1,
			"user_risk_score":       0.1,
			"login_count_1h":        0,
			"login_count_24h":       0,
			"is_new_device":         false,
			"country_changed":       false,
			"is_off_hours":          false,
		},
		Metadata: map[string]string{
			"request_id": "req_go_001",
			"source":     "go_example",
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
