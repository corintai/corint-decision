package com.corint;

import java.util.HashMap;
import java.util.Map;

/**
 * Decision request
 */
public class DecisionRequest {
    private Map<String, Object> event_data;
    private Map<String, Object> features;
    private Map<String, Object> api;
    private Map<String, Object> service;
    private Map<String, Object> llm;
    private Map<String, Object> vars;
    private Map<String, String> metadata;
    private DecisionOptions options;

    public DecisionRequest(Map<String, Object> eventData) {
        this.event_data = eventData;
        this.metadata = new HashMap<>();
        this.options = new DecisionOptions();
    }

    public Map<String, Object> getEventData() {
        return event_data;
    }

    public void setEventData(Map<String, Object> eventData) {
        this.event_data = eventData;
    }

    public Map<String, Object> getFeatures() {
        return features;
    }

    public void setFeatures(Map<String, Object> features) {
        this.features = features;
    }

    public Map<String, Object> getApi() {
        return api;
    }

    public void setApi(Map<String, Object> api) {
        this.api = api;
    }

    public Map<String, Object> getService() {
        return service;
    }

    public void setService(Map<String, Object> service) {
        this.service = service;
    }

    public Map<String, Object> getLlm() {
        return llm;
    }

    public void setLlm(Map<String, Object> llm) {
        this.llm = llm;
    }

    public Map<String, Object> getVars() {
        return vars;
    }

    public void setVars(Map<String, Object> vars) {
        this.vars = vars;
    }

    public Map<String, String> getMetadata() {
        return metadata;
    }

    public void setMetadata(Map<String, String> metadata) {
        this.metadata = metadata;
    }

    public DecisionOptions getOptions() {
        return options;
    }

    public void setOptions(DecisionOptions options) {
        this.options = options;
    }

    public static class DecisionOptions {
        private boolean enable_trace = false;

        public boolean isEnableTrace() {
            return enable_trace;
        }

        public void setEnableTrace(boolean enableTrace) {
            this.enable_trace = enableTrace;
        }
    }
}
