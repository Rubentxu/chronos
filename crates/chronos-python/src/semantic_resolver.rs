use chronos_domain::{EventData, EventType, Language, TraceEvent};
use chronos_domain::semantic::{ResolveContext, SemanticEvent, SemanticEventKind, SemanticResolver};

/// SemanticResolver for Python that enriches PythonFrame events.
#[derive(Debug)]
pub struct PythonSemanticResolver;

impl PythonSemanticResolver {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PythonSemanticResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticResolver for PythonSemanticResolver {
    fn language(&self) -> Language {
        Language::Python
    }

    fn name(&self) -> &str {
        "python-sysprobe"
    }

    fn resolve(&self, event: &TraceEvent, _ctx: &ResolveContext) -> Option<SemanticEvent> {
        let frame = match &event.data {
            EventData::PythonFrame {
                qualified_name,
                file,
                line,
                locals,
                ..
            } => (qualified_name, file, line, locals),
            _ => return None,
        };

        let (qualified_name, file, line, locals) = frame;

        let description = match event.event_type {
            EventType::FunctionEntry => {
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
                    format!("{}() @ {}:{}", qualified_name, file, line)
                } else {
                    format!("{}({}) @ {}:{}", qualified_name, args.join(", "), file, line)
                }
            }
            EventType::FunctionExit => {
                format!("return {}() @ {}:{}", qualified_name, file, line)
            }
            EventType::ExceptionThrown => {
                format!("EXCEPTION in {} @ {}:{}", qualified_name, file, line)
            }
            _ => return None,
        };

        let kind = match event.event_type {
            EventType::FunctionEntry => {
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
                    function: qualified_name.clone(),
                    module: Some(file.clone()),
                    arguments: args,
                }
            }
            EventType::FunctionExit => SemanticEventKind::FunctionReturned {
                function: qualified_name.clone(),
                return_value: None,
            },
            EventType::ExceptionThrown => {
                let type_name = qualified_name
                    .rsplit('.')
                    .next()
                    .unwrap_or(qualified_name)
                    .to_string();
                SemanticEventKind::Exception {
                    type_name,
                    message: format!("{} at {}:{}", qualified_name, file, line),
                    stack_trace: vec![format!("{}:{}", file, line)],
                }
            }
            _ => return None,
        };

        Some(SemanticEvent {
            source_event_id: event.event_id,
            timestamp_ns: event.timestamp_ns,
            thread_id: event.thread_id,
            language: Language::Python,
            kind,
            description,
        })
    }
}
