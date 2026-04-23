//! Semantic Resolver — translates raw kernel events into high-level LLM-friendly events.
//!
//! Raw events from eBPF/ptrace contain low-level data: instruction pointers, syscall
//! numbers, register values. A `SemanticResolver` translates these into human-readable
//! events: "UserService::login called with user_id=42" instead of "uprobe hit at 0x40a2b0".
//!
//! Each language adapter provides its own resolver that understands the runtime's
//! object layout, calling conventions, and debug info format.

use crate::{Language, TraceEvent};

// ---------------------------------------------------------------------------
// Semantic Event
// ---------------------------------------------------------------------------

/// A high-level, LLM-friendly representation of a trace event.
///
/// Unlike `TraceEvent` which contains raw addresses and register values,
/// a `SemanticEvent` contains resolved names, typed arguments, and
/// human-readable descriptions.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SemanticEvent {
    /// The original trace event ID.
    pub source_event_id: u64,
    /// The original timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// The thread that produced this event.
    pub thread_id: u64,
    /// The language runtime that was traced.
    pub language: Language,
    /// High-level event kind (language-specific).
    pub kind: SemanticEventKind,
    /// Human-readable description of what happened.
    pub description: String,
}

impl std::fmt::Debug for SemanticEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SemanticEvent")
            .field("source_event_id", &self.source_event_id)
            .field("timestamp_ns", &self.timestamp_ns)
            .field("thread_id", &self.thread_id)
            .field("language", &self.language)
            .field("kind", &self.kind)
            .field("description", &self.description)
            .finish()
    }
}

/// Classification of a semantic event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SemanticEventKind {
    /// A function was called.
    FunctionCalled {
        /// Resolved function name (demangled).
        function: String,
        /// Module or class the function belongs to.
        module: Option<String>,
        /// Resolved arguments (name → value).
        arguments: Vec<(String, String)>,
    },
    /// A function returned.
    FunctionReturned {
        /// Resolved function name.
        function: String,
        /// Return value (if captured).
        return_value: Option<String>,
    },
    /// An exception was thrown/caught.
    Exception {
        /// Exception type name.
        type_name: String,
        /// Exception message.
        message: String,
        /// Stack trace at the throw point.
        stack_trace: Vec<String>,
    },
    /// A syscall was made.
    Syscall {
        /// Syscall name (e.g., "open", "read", "write").
        name: String,
        /// Resolved arguments.
        arguments: Vec<(String, String)>,
        /// Return value.
        return_value: Option<String>,
    },
    /// A memory allocation/deallocation.
    MemoryOperation {
        /// Type of operation.
        operation: MemoryOp,
        /// Number of bytes.
        size: Option<usize>,
        /// Address.
        address: u64,
    },
    /// A thread was created or exited.
    ThreadLifecycle {
        /// What happened.
        event: ThreadEvent,
        /// Thread ID.
        tid: u64,
    },
    /// A signal was delivered.
    Signal {
        /// Signal name (e.g., "SIGSEGV", "SIGKILL").
        name: String,
        /// Signal number.
        number: i32,
    },
    /// A generic event that didn't fit other categories.
    Generic {
        /// What happened.
        summary: String,
    },
    /// The raw event couldn't be resolved (no matching resolver).
    Unresolved,
}

/// Memory operation type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MemoryOp {
    Alloc,
    Free,
    Read,
    Write,
}

/// Thread lifecycle event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ThreadEvent {
    Created,
    Exited,
    Started,
}

// ---------------------------------------------------------------------------
// SemanticResolver trait
// ---------------------------------------------------------------------------

/// Translates raw `TraceEvent`s into `SemanticEvent`s.
///
/// Each language runtime (Python, Java, Go, etc.) implements this trait
/// to provide language-specific semantic enrichment. For example:
///
/// - **Python resolver**: reads `PyFrameObject` from `/proc/pid/mem` to get
///   function names and local variables from a uprobe hit.
/// - **Java resolver**: uses JVMTI/JDWP to resolve BCI → source line,
///   object ID → class name.
/// - **Native resolver**: uses DWARF debug info to resolve address → function
///   name, register values → local variables.
pub trait SemanticResolver: Send + Sync {
    /// The language this resolver handles.
    fn language(&self) -> Language;

    /// Attempt to resolve a raw trace event into a semantic event.
    ///
    /// Returns `Some(SemanticEvent)` if this resolver can handle the event,
    /// or `None` if the event is not relevant to this language.
    fn resolve(&self, event: &TraceEvent, context: &ResolveContext) -> Option<SemanticEvent>;

    /// Human-readable name of this resolver.
    fn name(&self) -> &str;
}

/// Context provided to resolvers for enrichment.
///
/// Contains information about the target process that resolvers
/// may need to translate raw events.
#[derive(Debug, Clone)]
pub struct ResolveContext {
    /// PID of the target process.
    pub pid: u32,
    /// Path to the target binary.
    pub binary_path: Option<String>,
}

// ---------------------------------------------------------------------------
// ResolverPipeline
// ---------------------------------------------------------------------------

/// A pipeline of semantic resolvers that tries each one in order.
///
/// Events are dispatched to the first resolver that claims them.
/// If no resolver handles an event, it gets `SemanticEventKind::Unresolved`.
#[derive(Clone)]
pub struct ResolverPipeline {
    resolvers: Vec<std::sync::Arc<dyn SemanticResolver>>,
}

impl Default for ResolverPipeline {
    fn default() -> Self {
        Self { resolvers: Vec::new() }
    }
}

impl std::fmt::Debug for ResolverPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolverPipeline")
            .field("resolver_count", &self.resolvers.len())
            .finish()
    }
}

impl ResolverPipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a resolver to the pipeline.
    pub fn add_resolver(&mut self, resolver: Box<dyn SemanticResolver>) {
        self.resolvers.push(std::sync::Arc::from(resolver));
    }

    /// Resolve an event through the pipeline.
    ///
    /// Tries each resolver in order. Returns the first match,
    /// or an `Unresolved` event if none match.
    pub fn resolve(&self, event: &TraceEvent, context: &ResolveContext) -> SemanticEvent {
        for resolver in &self.resolvers {
            if let Some(se) = resolver.resolve(event, context) {
                return se;
            }
        }

        // No resolver handled this event
        SemanticEvent {
            source_event_id: event.event_id,
            timestamp_ns: event.timestamp_ns,
            thread_id: event.thread_id,
            language: Language::Unknown,
            kind: SemanticEventKind::Unresolved,
            description: format!("{:?}", event.event_type),
        }
    }

    /// Resolve a batch of events.
    pub fn resolve_batch(
        &self,
        events: &[TraceEvent],
        context: &ResolveContext,
    ) -> Vec<SemanticEvent> {
        events.iter().map(|e| self.resolve(e, context)).collect()
    }

    /// Number of registered resolvers.
    pub fn resolver_count(&self) -> usize {
        self.resolvers.len()
    }

    /// List the names of all registered resolvers.
    pub fn resolver_names(&self) -> Vec<&str> {
        self.resolvers.iter().map(|r| r.name()).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventData, EventType, SourceLocation};

    /// A minimal test resolver that handles FunctionEntry events.
    struct TestResolver;

    impl SemanticResolver for TestResolver {
        fn language(&self) -> Language {
            Language::Rust
        }

        fn resolve(&self, event: &TraceEvent, _ctx: &ResolveContext) -> Option<SemanticEvent> {
            match event.event_type {
                EventType::FunctionEntry => Some(SemanticEvent {
                    source_event_id: event.event_id,
                    timestamp_ns: event.timestamp_ns,
                    thread_id: event.thread_id,
                    language: Language::Rust,
                    kind: SemanticEventKind::FunctionCalled {
                        function: event.location.function.clone().unwrap_or_default(),
                        module: event.location.file.clone(),
                        arguments: vec![],
                    },
                    description: format!("Called {}", event.location.function.as_deref().unwrap_or("??")),
                }),
                _ => None,
            }
        }

        fn name(&self) -> &str {
            "test-resolver"
        }
    }

    fn make_event(id: u64, event_type: EventType, func: &str) -> TraceEvent {
        let mut loc = SourceLocation::from_address(0x1000);
        loc.function = Some(func.to_string());
        TraceEvent::new(id, id * 1000, 1, event_type, loc, EventData::Empty)
    }

    #[test]
    fn test_pipeline_with_matching_resolver() {
        let mut pipeline = ResolverPipeline::new();
        pipeline.add_resolver(Box::new(TestResolver));

        let event = make_event(1, EventType::FunctionEntry, "main");
        let result = pipeline.resolve(&event, &ResolveContext {
            pid: 1234,
            binary_path: None,
        });

        assert!(matches!(result.kind, SemanticEventKind::FunctionCalled { .. }));
        assert_eq!(result.source_event_id, 1);
        assert_eq!(result.language, Language::Rust);
    }

    #[test]
    fn test_pipeline_unresolved_event() {
        let mut pipeline = ResolverPipeline::new();
        pipeline.add_resolver(Box::new(TestResolver));

        let event = make_event(2, EventType::SyscallEnter, "");
        let result = pipeline.resolve(&event, &ResolveContext {
            pid: 1234,
            binary_path: None,
        });

        assert!(matches!(result.kind, SemanticEventKind::Unresolved));
    }

    #[test]
    fn test_pipeline_empty() {
        let pipeline = ResolverPipeline::new();

        let event = make_event(1, EventType::FunctionEntry, "main");
        let result = pipeline.resolve(&event, &ResolveContext {
            pid: 1234,
            binary_path: None,
        });

        assert!(matches!(result.kind, SemanticEventKind::Unresolved));
        assert_eq!(pipeline.resolver_count(), 0);
    }

    #[test]
    fn test_resolve_batch() {
        let mut pipeline = ResolverPipeline::new();
        pipeline.add_resolver(Box::new(TestResolver));

        let events = vec![
            make_event(1, EventType::FunctionEntry, "alpha"),
            make_event(2, EventType::SyscallEnter, ""),
            make_event(3, EventType::FunctionEntry, "beta"),
        ];

        let results = pipeline.resolve_batch(&events, &ResolveContext {
            pid: 1234,
            binary_path: None,
        });

        assert_eq!(results.len(), 3);
        assert!(matches!(results[0].kind, SemanticEventKind::FunctionCalled { .. }));
        assert!(matches!(results[1].kind, SemanticEventKind::Unresolved));
        assert!(matches!(results[2].kind, SemanticEventKind::FunctionCalled { .. }));
    }

    #[test]
    fn test_resolver_names() {
        let mut pipeline = ResolverPipeline::new();
        pipeline.add_resolver(Box::new(TestResolver));

        assert_eq!(pipeline.resolver_names(), vec!["test-resolver"]);
    }
}
