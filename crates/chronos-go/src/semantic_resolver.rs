use chronos_domain::{EventData, EventType, GoEventKind, Language, TraceEvent};
use chronos_domain::semantic::{ResolveContext, SemanticEvent, SemanticEventKind, SemanticResolver};

/// SemanticResolver for Go that enriches GoFrame events from Delve debugger.
#[derive(Debug)]
pub struct GoSemanticResolver;

impl GoSemanticResolver {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GoSemanticResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticResolver for GoSemanticResolver {
    fn language(&self) -> Language {
        Language::Go
    }

    fn name(&self) -> &str {
        "go-delve"
    }

    fn resolve(&self, event: &TraceEvent, _ctx: &ResolveContext) -> Option<SemanticEvent> {
        let (goroutine_id, function_name, file, line, locals, event_kind) = match &event.data {
            EventData::GoFrame {
                goroutine_id,
                function_name,
                file,
                line,
                locals,
                event_kind,
            } => (goroutine_id, function_name, file, line, locals, event_kind),
            _ => return None,
        };

        let file_str = file.as_deref().unwrap_or("?");
        let line_str = line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string());

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
                let location = format!("{}:{}", file_str, line_str);
                if args.is_empty() {
                    format!("{}() @ {} [goid={}]", function_name, location, goroutine_id)
                } else {
                    format!(
                        "{}({}) @ {} [goid={}]",
                        function_name,
                        args.join(", "),
                        location,
                        goroutine_id
                    )
                }
            }
            EventType::FunctionExit => {
                format!(
                    "return {}() @ {}:{} [goid={}]",
                    function_name, file_str, line_str, goroutine_id
                )
            }
            _ => return None,
        };

        let kind = match event_kind {
            GoEventKind::Breakpoint | GoEventKind::Step => {
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
                    module: file.clone(),
                    arguments: args,
                }
            }
            GoEventKind::GoroutineStop => SemanticEventKind::Generic {
                summary: format!(
                    "goroutine {} stopped at {}:{}",
                    goroutine_id, file_str, line_str
                ),
            },
            GoEventKind::Exception => SemanticEventKind::Exception {
                type_name: "panic".to_string(),
                message: format!("{} at {}:{}", function_name, file_str, line_str),
                stack_trace: vec![format!(
                    "{} at {}:{}",
                    function_name, file_str, line_str
                )],
            },
        };

        Some(SemanticEvent {
            source_event_id: event.event_id,
            timestamp_ns: event.timestamp_ns,
            thread_id: event.thread_id,
            language: Language::Go,
            kind,
            description,
        })
    }
}
