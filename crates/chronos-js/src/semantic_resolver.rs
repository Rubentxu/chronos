use chronos_domain::{EventData, EventType, JsEventKind, Language, TraceEvent};
use chronos_domain::semantic::{ResolveContext, SemanticEvent, SemanticEventKind, SemanticResolver};

/// SemanticResolver for JavaScript that enriches JsFrame events from V8 inspector.
#[derive(Debug)]
pub struct JsSemanticResolver;

impl JsSemanticResolver {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsSemanticResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticResolver for JsSemanticResolver {
    fn language(&self) -> Language {
        Language::JavaScript
    }

    fn name(&self) -> &str {
        "js-v8"
    }

    fn resolve(&self, event: &TraceEvent, _ctx: &ResolveContext) -> Option<SemanticEvent> {
        let frame = match &event.data {
            EventData::JsFrame {
                function_name,
                script_url,
                line_number,
                column_number,
                locals,
                scope_chain,
                event_kind,
            } => (function_name, script_url, line_number, column_number, locals, scope_chain, event_kind),
            _ => return None,
        };

        let (function_name, script_url, line_number, _column_number, locals, _scope_chain, event_kind) = frame;

        let description = match event.event_type {
            EventType::FunctionEntry | EventType::BreakpointHit => {
                let args: Vec<String> = locals
                    .as_ref()
                    .map(|l| {
                        l.iter()
                            .filter(|v| !v.name.starts_with("__"))
                            .map(|v| format!("{}={}", v.name, v.value))
                            .collect()
                    })
                    .unwrap_or_default();
                if args.is_empty() {
                    format!("{}() @ {}:{}", function_name, script_url, line_number)
                } else {
                    format!(
                        "{}({}) @ {}:{}",
                        function_name,
                        args.join(", "),
                        script_url,
                        line_number
                    )
                }
            }
            _ => return None,
        };

        let kind = match event_kind {
            JsEventKind::Breakpoint | JsEventKind::Step => {
                let args: Vec<(String, String)> = locals
                    .as_ref()
                    .map(|l| {
                        l.iter()
                            .filter(|v| !v.name.starts_with("__"))
                            .take(5)
                            .map(|v| (v.name.clone(), v.value.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                SemanticEventKind::FunctionCalled {
                    function: function_name.clone(),
                    module: Some(script_url.clone()),
                    arguments: args,
                }
            }
            JsEventKind::Exception => SemanticEventKind::Exception {
                type_name: "Error".to_string(),
                message: format!("{} @ {}:{}", function_name, script_url, line_number),
                stack_trace: vec![format!(
                    "{} @ {}:{}",
                    function_name, script_url, line_number
                )],
            },
            JsEventKind::Other(tag) => SemanticEventKind::Generic {
                summary: format!(
                    "JS {} at {}:{} in {}",
                    tag, script_url, line_number, function_name
                ),
            },
        };

        Some(SemanticEvent {
            source_event_id: event.event_id,
            timestamp_ns: event.timestamp_ns,
            thread_id: event.thread_id,
            language: Language::JavaScript,
            kind,
            description,
        })
    }
}
