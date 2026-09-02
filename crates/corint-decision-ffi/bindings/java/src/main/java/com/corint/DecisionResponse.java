package com.corint;

import java.util.List;
import java.util.Map;

/**
 * Decision response
 */
public class DecisionResponse {
    private String request_id;
    private String pipeline_id;
    private DecisionResult result;
    private long processing_time_ms;
    private Map<String, String> metadata;
    private Map<String, Object> trace;

    public String getRequestId() {
        return request_id;
    }

    public void setRequestId(String requestId) {
        this.request_id = requestId;
    }

    public String getPipelineId() {
        return pipeline_id;
    }

    public void setPipelineId(String pipelineId) {
        this.pipeline_id = pipelineId;
    }

    public DecisionResult getResult() {
        return result;
    }

    public void setResult(DecisionResult result) {
        this.result = result;
    }

    public Map<String, Object> getTrace() {
        return trace;
    }

    public void setTrace(Map<String, Object> trace) {
        this.trace = trace;
    }

    public Map<String, String> getMetadata() {
        return metadata;
    }

    public void setMetadata(Map<String, String> metadata) {
        this.metadata = metadata;
    }

    public long getProcessingTimeMs() {
        return processing_time_ms;
    }

    public void setProcessingTimeMs(long processingTimeMs) {
        this.processing_time_ms = processingTimeMs;
    }

    public String getDecision() {
        if (result == null || result.getSignal() == null) {
            return null;
        }
        return result.getSignal().getType();
    }

    public List<String> getActions() {
        if (result == null) {
            return null;
        }
        return result.getActions();
    }

    public static class DecisionResult {
        private DecisionSignal signal;
        private List<String> actions;
        private int score;
        private List<String> triggered_rules;
        private String explanation;
        private Map<String, Object> context;

        public DecisionSignal getSignal() {
            return signal;
        }

        public void setSignal(DecisionSignal signal) {
            this.signal = signal;
        }

        public List<String> getActions() {
            return actions;
        }

        public void setActions(List<String> actions) {
            this.actions = actions;
        }

        public int getScore() {
            return score;
        }

        public void setScore(int score) {
            this.score = score;
        }

        public List<String> getTriggeredRules() {
            return triggered_rules;
        }

        public void setTriggeredRules(List<String> triggeredRules) {
            this.triggered_rules = triggeredRules;
        }

        public String getExplanation() {
            return explanation;
        }

        public void setExplanation(String explanation) {
            this.explanation = explanation;
        }

        public Map<String, Object> getContext() {
            return context;
        }

        public void setContext(Map<String, Object> context) {
            this.context = context;
        }
    }

    public static class DecisionSignal {
        private String type;

        public String getType() {
            return type;
        }

        public void setType(String type) {
            this.type = type;
        }
    }
}
