package com.corint;

import com.google.gson.Gson;
import com.google.gson.JsonObject;
import com.sun.jna.Pointer;

import java.util.HashMap;
import java.util.Map;

/**
 * CORINT Decision Engine for Java
 */
public class DecisionEngine implements AutoCloseable {
    private static final Gson gson = new Gson();
    private Pointer handle;

    /**
     * Create a new decision engine from a file system repository
     *
     * @param repositoryPath Path to the repository
     */
    public DecisionEngine(String repositoryPath) {
        this.handle = CorintNative.INSTANCE.corint_engine_new(repositoryPath);
        if (this.handle == null) {
            throw new RuntimeException("Failed to create decision engine");
        }
    }

    /**
     * Create a new decision engine from a database
     *
     * @param databaseUrl PostgreSQL database URL
     * @param fromDatabase Must be true to use database constructor
     */
    public DecisionEngine(String databaseUrl, boolean fromDatabase) {
        if (!fromDatabase) {
            throw new IllegalArgumentException("Use DecisionEngine(String) for file system repository");
        }
        this.handle = CorintNative.INSTANCE.corint_engine_new_from_database(databaseUrl);
        if (this.handle == null) {
            throw new RuntimeException("Failed to create decision engine from database");
        }
    }

    /**
     * Execute a decision
     *
     * @param request The decision request
     * @return The decision response
     */
    public DecisionResponse decide(DecisionRequest request) {
        if (this.handle == null) {
            throw new IllegalStateException("Engine has been closed");
        }

        // Convert request to JSON
        String requestJson = gson.toJson(request);

        // Call FFI function
        Pointer resultPtr = CorintNative.INSTANCE.corint_engine_decide(this.handle, requestJson);

        if (resultPtr == null) {
            throw new RuntimeException("Decision execution failed");
        }

        String resultJson = resultPtr.getString(0);
        CorintNative.INSTANCE.corint_string_free(resultPtr);

        // Parse response
        JsonObject result = gson.fromJson(resultJson, JsonObject.class);

        // Check for errors
        if (result.has("error")) {
            throw new RuntimeException("Decision error: " + result.get("error").getAsString());
        }

        return gson.fromJson(resultJson, DecisionResponse.class);
    }

    /**
     * Close the engine and free resources
     */
    @Override
    public void close() {
        if (this.handle != null) {
            CorintNative.INSTANCE.corint_engine_free(this.handle);
            this.handle = null;
        }
    }

    /**
     * Get the CORINT version
     *
     * @return Version string
     */
    public static String version() {
        Pointer versionPtr = CorintNative.INSTANCE.corint_version();
        if (versionPtr == null) {
            return "unknown";
        }
        String version = versionPtr.getString(0);
        CorintNative.INSTANCE.corint_string_free(versionPtr);
        return version;
    }

    /**
     * Initialize the logging system
     */
    public static void initLogging() {
        CorintNative.INSTANCE.corint_init_logging();
    }
}
