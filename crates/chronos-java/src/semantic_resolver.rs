use chronos_domain::{EventData, EventType, JavaEventKind, Language, TraceEvent};
use chronos_domain::semantic::{ResolveContext, SemanticEvent, SemanticEventKind, SemanticResolver};

/// SemanticResolver for Java that enriches JavaFrame events.
#[derive(Debug)]
pub struct JavaSemanticResolver;

impl JavaSemanticResolver {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JavaSemanticResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticResolver for JavaSemanticResolver {
    fn language(&self) -> Language {
        Language::Java
    }

    fn name(&self) -> &str {
        "java-jdwp"
    }

    fn resolve(&self, event: &TraceEvent, _ctx: &ResolveContext) -> Option<SemanticEvent> {
        let (class_name, method_name, signature, file, line, locals, event_kind) = match &event.data {
            EventData::JavaFrame {
                class_name,
                method_name,
                signature,
                file,
                line,
                locals,
                event_kind,
            } => (class_name, method_name, signature, file, line, locals, event_kind),
            _ => return None,
        };

        let qualified_name = format!("{}.{}", class_name, method_name);

        let file_str = file.as_deref().unwrap_or("?");
        let line_str = line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string());

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
                let sig = signature.as_deref().unwrap_or("");
                if args.is_empty() {
                    format!("{}({}) @ {}:{}", qualified_name, sig, file_str, line_str)
                } else {
                    format!(
                        "{}({}) with {} @ {}:{}",
                        qualified_name,
                        sig,
                        args.join(", "),
                        file_str,
                        line_str
                    )
                }
            }
            EventType::FunctionExit => {
                format!("return {}.{} @ {}:{}", class_name, method_name, file_str, line_str)
            }
            EventType::ExceptionThrown => {
                format!(
                    "EXCEPTION in {}.{} @ {}:{}",
                    class_name, method_name, file_str, line_str
                )
            }
            _ => return None,
        };

        let kind = match event_kind {
            JavaEventKind::MethodEntry => {
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
                    module: file.clone(),
                    arguments: args,
                }
            }
            JavaEventKind::MethodExit => SemanticEventKind::FunctionReturned {
                function: qualified_name.clone(),
                return_value: None,
            },
            JavaEventKind::Exception => SemanticEventKind::Exception {
                type_name: class_name.clone(),
                message: format!("{} at {}:{}", qualified_name, file_str, line_str),
                stack_trace: vec![format!(
                    "{}.{} at {}:{}",
                    class_name, method_name, file_str, line_str
                )],
            },
        };

        Some(SemanticEvent {
            source_event_id: event.event_id,
            timestamp_ns: event.timestamp_ns,
            thread_id: event.thread_id,
            language: Language::Java,
            kind,
            description,
        })
    }
}
