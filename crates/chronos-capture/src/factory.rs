//! Adapter registry — maps languages to trace adapters.

use crate::adapter::TraceAdapter;
use chronos_domain::{Language, TraceError};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry that maps languages to their trace adapters.
pub struct AdapterRegistry {
    adapters: HashMap<Language, Arc<dyn TraceAdapter>>,
}

impl AdapterRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Create a new registry with the given adapters pre-registered.
    pub fn with_adapters(adapters: Vec<Arc<dyn TraceAdapter>>) -> Self {
        let mut registry = Self::new();
        for adapter in adapters {
            registry.register(adapter);
        }
        registry
    }

    /// Register an adapter for a language.
    pub fn register(&mut self, adapter: Arc<dyn TraceAdapter>) {
        self.adapters.insert(adapter.get_language(), adapter);
    }

    /// Get the adapter for a specific language.
    pub fn get(&self, language: Language) -> Option<Arc<dyn TraceAdapter>> {
        self.adapters.get(&language).cloned()
    }

    /// Get an adapter, returning an error if not found.
    pub fn get_or_error(&self, language: Language) -> Result<Arc<dyn TraceAdapter>, TraceError> {
        self.get(language).ok_or_else(|| {
            TraceError::UnsupportedLanguage(format!(
                "No adapter registered for language: {}",
                language
            ))
        })
    }

    /// Get an adapter for a native language (C, C++, or Rust).
    /// All three share the same adapter (ptrace-based).
    pub fn get_native(&self) -> Option<Arc<dyn TraceAdapter>> {
        self.adapters.get(&Language::C).cloned()
    }

    /// List all registered languages.
    pub fn registered_languages(&self) -> Vec<Language> {
        self.adapters.keys().copied().collect()
    }

    /// Check if an adapter is registered for a language.
    pub fn has_adapter(&self, language: Language) -> bool {
        self.adapters.contains_key(&language)
    }

    /// Check if the registry supports a given language string (e.g., "python", "javascript").
    #[allow(dead_code)]
    pub fn supports_language_string(&self, lang: &str) -> bool {
        match lang.to_lowercase().as_str() {
            "python" => self.has_adapter(Language::Python),
            "javascript" | "nodejs" | "js" | "node" => self.has_adapter(Language::JavaScript),
            "rust" | "c" | "cpp" | "c++" | "native" => {
                self.has_adapter(Language::Rust)
                    || self.has_adapter(Language::C)
                    || self.has_adapter(Language::Cpp)
            }
            "ebpf" => self.has_adapter(Language::Ebpf),
            _ => false,
        }
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::TraceAdapter;
    use chronos_domain::{CaptureConfig, CaptureSession, Language, TraceError};
    use std::sync::Arc;

    struct TestAdapter {
        language: Language,
    }

    impl TraceAdapter for TestAdapter {
        fn start_capture(&self, _config: CaptureConfig) -> Result<CaptureSession, TraceError> {
            Ok(CaptureSession::new(1, self.language, _config))
        }
        fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
            Ok(())
        }
        fn attach_to_process(
            &self,
            _pid: u32,
            config: CaptureConfig,
        ) -> Result<CaptureSession, TraceError> {
            Ok(CaptureSession::new(_pid, self.language, config))
        }
        fn get_language(&self) -> Language {
            self.language
        }
        fn name(&self) -> &str {
            "test"
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = AdapterRegistry::new();
        let rust_adapter = Arc::new(TestAdapter {
            language: Language::Rust,
        });
        let c_adapter = Arc::new(TestAdapter {
            language: Language::C,
        });

        registry.register(rust_adapter);
        registry.register(c_adapter);

        assert!(registry.has_adapter(Language::Rust));
        assert!(registry.has_adapter(Language::C));
        assert!(!registry.has_adapter(Language::Python));

        let langs = registry.registered_languages();
        assert_eq!(langs.len(), 2);
    }

    #[test]
    fn test_registry_get_or_error() {
        let registry = AdapterRegistry::new();
        let result = registry.get_or_error(Language::Python);
        match result {
            Err(e) => assert!(e.to_string().contains("python"), "Error was: {}", e),
            Ok(_) => panic!("Expected error for unregistered language"),
        }
    }

    #[test]
    fn test_registry_default() {
        let registry = AdapterRegistry::default();
        assert!(registry.registered_languages().is_empty());
    }
}
