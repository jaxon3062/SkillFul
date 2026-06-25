use serde_json::{Value, json};

pub fn list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "skilltrace.record_event",
                "description": "Record an arbitrary tracing event.",
                "inputSchema": {
                    "type": "object",
                    "required": ["event_type", "session_id"],
                    "properties": {
                        "event_type": { "type": "string" },
                        "session_id": { "type": "string" },
                        "task_id": { "type": "string" },
                        "skill": { "type": "string" },
                        "agent": { "type": "string" },
                        "adapter": { "type": "string" },
                        "success": { "type": "boolean" }
                    }
                }
            },
            {
                "name": "skilltrace.record_skill_start",
                "description": "Record the start of a skill invocation.",
                "inputSchema": {
                    "type": "object",
                    "required": ["skill"],
                    "properties": {
                        "skill": { "type": "string" },
                        "session_id": { "type": "string" },
                        "task_id": { "type": "string" },
                        "planner_reason": { "type": "string" },
                        "confidence": { "type": "number" },
                        "alternatives": { "type": "array", "items": { "type": "string" } }
                    }
                }
            },
            {
                "name": "skilltrace.record_skill_end",
                "description": "Complete a previously started skill event.",
                "inputSchema": {
                    "type": "object",
                    "required": ["event_id", "success"],
                    "properties": {
                        "event_id": { "type": "string" },
                        "success": { "type": "boolean" },
                        "output_summary": { "type": "string" },
                        "error": { "type": "string" }
                    }
                }
            },
            {
                "name": "skilltrace.record_decision",
                "description": "Record a planner or routing decision.",
                "inputSchema": {
                    "type": "object",
                    "required": ["session_id"],
                    "properties": {
                        "session_id": { "type": "string" },
                        "task_id": { "type": "string" },
                        "skill": { "type": "string" },
                        "planner_reason": { "type": "string" },
                        "confidence": { "type": "number" },
                        "alternatives": { "type": "array", "items": { "type": "string" } }
                    }
                }
            },
            {
                "name": "skilltrace.get_stats",
                "description": "Return aggregated skill usage statistics.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "since": { "type": "string" },
                        "agent": { "type": "string" },
                        "skill": { "type": "string" }
                    }
                }
            },
            {
                "name": "skilltrace.get_failures",
                "description": "Return failed events and retry information.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "since": { "type": "string" },
                        "agent": { "type": "string" },
                        "skill": { "type": "string" }
                    }
                }
            },
            {
                "name": "skilltrace.get_recommendations",
                "description": "Return heuristic recommendations derived from local traces.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "agent": { "type": "string" },
                        "cwd": { "type": "string" }
                    }
                }
            }
        ]
    })
}
