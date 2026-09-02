package com.corint;

import java.util.HashMap;
import java.util.Map;

/**
 * Example usage of CORINT Decision Engine Java bindings
 */
public class Example {
    public static void main(String[] args) {
        // Print version
        System.out.println("CORINT Version: " + DecisionEngine.version());

        // Initialize logging
        DecisionEngine.initLogging();

        // Create engine with file system repository
        // Assumes 'repository' directory exists in current working directory
        try (DecisionEngine engine = new DecisionEngine("repository")) {
            // Create a decision request
            Map<String, Object> eventData = new HashMap<>();
            eventData.put("user_id", "user123");
            eventData.put("email", "test@example.com");
            eventData.put("amount", 1000.0);
            eventData.put("ip", "192.168.1.1");

            DecisionRequest request = new DecisionRequest(eventData);
            request.getOptions().setEnableTrace(true);

            // Execute decision
            DecisionResponse response = engine.decide(request);

            // Print results
            System.out.println("Decision: " + response.getDecision());
            System.out.println("Actions: " + response.getActions());

            if (response.getTrace() != null) {
                System.out.println("Execution trace available");
            }

            System.out.println("Done!");
        } catch (Exception e) {
            System.err.println("Error: " + e.getMessage());
            e.printStackTrace();
        }
    }
}
