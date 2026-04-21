//! Semantic compression for execution traces.
//!
//! Provides a 4-level progressive zoom model:
//!
//! - **Executive (0)**: One-line summary — total events, top functions, anomalies.
//! - **Hotspot (1)**: Top 10 functions by call count + their cycles (if available).
//! - **Detail (2)**: All functions with full perf counters + call graph adjacency.
//! - **Microscopy (3)**: Full raw event list — no compression.
//!
//! The AI agent starts at Executive and zooms in only on interesting areas,
//! keeping token cost proportional to the analysis depth needed.

use serde::{Deserialize, Serialize};

/// Four-level zoom into trace data.
///
/// Each level is a superset of the level above it.
/// Use `expand()` to move from coarser to finer detail.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum CompressionLevel {
    /// Level 0 — one-line executive summary.
    #[default]
    Executive = 0,
    /// Level 1 — top-10 hotspot functions.
    Hotspot = 1,
    /// Level 2 — all functions with counters + call graph.
    Detail = 2,
    /// Level 3 — raw events, no compression.
    Microscopy = 3,
}

impl CompressionLevel {
    /// Returns the next finer level, or `None` if already at Microscopy.
    pub fn expand(&self) -> Option<Self> {
        match self {
            Self::Executive => Some(Self::Hotspot),
            Self::Hotspot => Some(Self::Detail),
            Self::Detail => Some(Self::Microscopy),
            Self::Microscopy => None,
        }
    }

    /// Returns the next coarser level, or `None` if already at Executive.
    pub fn compress(&self) -> Option<Self> {
        match self {
            Self::Executive => None,
            Self::Hotspot => Some(Self::Executive),
            Self::Detail => Some(Self::Hotspot),
            Self::Microscopy => Some(Self::Detail),
        }
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Executive => "executive",
            Self::Hotspot => "hotspot",
            Self::Detail => "detail",
            Self::Microscopy => "microscopy",
        }
    }
}

impl std::fmt::Display for CompressionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Executive summary (Level 0) — minimal, one-line-per-metric.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutiveSummary {
    /// Total number of events captured.
    pub total_events: u64,
    /// Number of unique functions called.
    pub unique_functions: usize,
    /// Number of distinct threads seen.
    pub thread_count: usize,
    /// Wall-clock duration of the trace in nanoseconds.
    pub duration_ns: u64,
    /// Up to 3 top functions by call count.
    pub top_functions: Vec<String>,
    /// Detected anomalies (crashes, races, timeouts, etc.).
    pub anomalies: Vec<String>,
}

/// One hotspot entry (Level 1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HotspotEntry {
    /// Fully-qualified function name.
    pub function: String,
    /// Number of calls during the trace.
    pub call_count: u64,
    /// Total CPU cycles (if available).
    pub cycles: Option<u64>,
    /// Average cycles per call (derived).
    pub avg_cycles_per_call: Option<u64>,
}

/// Hotspot data (Level 1) — top-10 functions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HotspotData {
    /// Sorted by call count descending.
    pub top_functions: Vec<HotspotEntry>,
    /// Total call count across all functions.
    pub total_calls: u64,
}

/// One function's detailed profile (Level 2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionDetail {
    pub function: String,
    pub call_count: u64,
    pub cycles: Option<u64>,
    pub instructions: Option<u64>,
    pub cache_misses: Option<u64>,
    /// Functions called by this function (callees).
    pub callees: Vec<String>,
}

/// Detail data (Level 2) — all functions with counters + call graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetailData {
    pub functions: Vec<FunctionDetail>,
}

/// Microscopy data (Level 3) — raw event list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MicroscopyData {
    /// Raw event representations.
    pub events: Vec<RawEventEntry>,
}

/// Compact event for Microscopy level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawEventEntry {
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    pub event_type: String,
    pub function: String,
    pub address: u64,
}

/// A lazily-expanded compressed trace view.
///
/// Starts at `Executive` level. Call `expand_to()` to populate
/// finer-grained data on demand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedTrace {
    /// The current compression level.
    pub level: CompressionLevel,
    /// Always present — the executive summary.
    pub executive: ExecutiveSummary,
    /// Present when level >= Hotspot.
    pub hotspot: Option<HotspotData>,
    /// Present when level >= Detail.
    pub detail: Option<DetailData>,
    /// Present when level >= Microscopy.
    pub microscopy: Option<MicroscopyData>,
}

impl CompressedTrace {
    /// Create a new compressed trace at the Executive level.
    pub fn new(executive: ExecutiveSummary) -> Self {
        Self {
            level: CompressionLevel::Executive,
            executive,
            hotspot: None,
            detail: None,
            microscopy: None,
        }
    }

    /// Expand to the Hotspot level, adding hotspot data.
    pub fn with_hotspot(mut self, hotspot: HotspotData) -> Self {
        self.hotspot = Some(hotspot);
        if self.level < CompressionLevel::Hotspot {
            self.level = CompressionLevel::Hotspot;
        }
        self
    }

    /// Expand to the Detail level, adding function detail data.
    pub fn with_detail(mut self, detail: DetailData) -> Self {
        self.detail = Some(detail);
        if self.level < CompressionLevel::Detail {
            self.level = CompressionLevel::Detail;
        }
        self
    }

    /// Expand to the Microscopy level, adding raw events.
    pub fn with_microscopy(mut self, microscopy: MicroscopyData) -> Self {
        self.microscopy = Some(microscopy);
        self.level = CompressionLevel::Microscopy;
        self
    }

    /// Saliency score for a function: cycles/total_cycles * call_count weighting.
    ///
    /// Returns a `[0.0, 1.0]` score for the given function name if hotspot data
    /// is available. Returns `None` otherwise.
    pub fn saliency_score(&self, function: &str) -> Option<f64> {
        let hotspot = self.hotspot.as_ref()?;
        let total_cycles: u64 = hotspot.top_functions.iter().filter_map(|e| e.cycles).sum();
        let entry = hotspot
            .top_functions
            .iter()
            .find(|e| e.function == function)?;

        if total_cycles == 0 {
            // Fall back to call-count ratio
            if hotspot.total_calls == 0 {
                return Some(0.0);
            }
            return Some(entry.call_count as f64 / hotspot.total_calls as f64);
        }

        let fn_cycles = entry.cycles.unwrap_or(0);
        Some(fn_cycles as f64 / total_cycles as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn executive() -> ExecutiveSummary {
        ExecutiveSummary {
            total_events: 1000,
            unique_functions: 20,
            thread_count: 2,
            duration_ns: 5_000_000,
            top_functions: vec!["main".into(), "compute".into(), "io_loop".into()],
            anomalies: vec![],
        }
    }

    #[test]
    fn test_compression_level_expand_executive_to_hotspot() {
        let level = CompressionLevel::Executive;
        let next = level.expand().unwrap();
        assert_eq!(next, CompressionLevel::Hotspot);
    }

    #[test]
    fn test_compression_level_expand_hotspot_to_detail() {
        let level = CompressionLevel::Hotspot;
        let next = level.expand().unwrap();
        assert_eq!(next, CompressionLevel::Detail);
    }

    #[test]
    fn test_compression_level_expand_detail_to_microscopy() {
        let level = CompressionLevel::Detail;
        let next = level.expand().unwrap();
        assert_eq!(next, CompressionLevel::Microscopy);
    }

    #[test]
    fn test_compression_level_expand_microscopy_returns_none() {
        let level = CompressionLevel::Microscopy;
        assert!(level.expand().is_none());
    }

    #[test]
    fn test_compression_level_compress_hotspot_to_executive() {
        let level = CompressionLevel::Hotspot;
        assert_eq!(level.compress(), Some(CompressionLevel::Executive));
    }

    #[test]
    fn test_compression_level_ordering() {
        assert!(CompressionLevel::Executive < CompressionLevel::Hotspot);
        assert!(CompressionLevel::Hotspot < CompressionLevel::Detail);
        assert!(CompressionLevel::Detail < CompressionLevel::Microscopy);
    }

    #[test]
    fn test_compressed_trace_starts_at_executive() {
        let ct = CompressedTrace::new(executive());
        assert_eq!(ct.level, CompressionLevel::Executive);
        assert!(ct.hotspot.is_none());
        assert!(ct.detail.is_none());
        assert!(ct.microscopy.is_none());
    }

    #[test]
    fn test_compressed_trace_with_hotspot_sets_level() {
        let hotspot = HotspotData {
            top_functions: vec![HotspotEntry {
                function: "compute".into(),
                call_count: 500,
                cycles: Some(10_000),
                avg_cycles_per_call: Some(20),
            }],
            total_calls: 500,
        };
        let ct = CompressedTrace::new(executive()).with_hotspot(hotspot);
        assert_eq!(ct.level, CompressionLevel::Hotspot);
        assert!(ct.hotspot.is_some());
        assert!(ct.detail.is_none());
    }

    #[test]
    fn test_compressed_trace_with_detail_sets_level() {
        let detail = DetailData {
            functions: vec![FunctionDetail {
                function: "compute".into(),
                call_count: 5,
                cycles: Some(500),
                instructions: Some(200),
                cache_misses: None,
                callees: vec!["helper".into()],
            }],
        };
        let hotspot = HotspotData {
            top_functions: vec![],
            total_calls: 5,
        };
        let ct = CompressedTrace::new(executive())
            .with_hotspot(hotspot)
            .with_detail(detail);
        assert_eq!(ct.level, CompressionLevel::Detail);
        assert!(ct.detail.is_some());
    }

    #[test]
    fn test_saliency_score_by_cycles() {
        let hotspot = HotspotData {
            top_functions: vec![
                HotspotEntry {
                    function: "heavy".into(),
                    call_count: 10,
                    cycles: Some(8_000),
                    avg_cycles_per_call: Some(800),
                },
                HotspotEntry {
                    function: "light".into(),
                    call_count: 90,
                    cycles: Some(2_000),
                    avg_cycles_per_call: Some(22),
                },
            ],
            total_calls: 100,
        };
        let ct = CompressedTrace::new(executive()).with_hotspot(hotspot);

        let heavy_score = ct.saliency_score("heavy").unwrap();
        let light_score = ct.saliency_score("light").unwrap();

        // heavy used 80% of cycles
        assert!((heavy_score - 0.8).abs() < 1e-9);
        // light used 20% of cycles
        assert!((light_score - 0.2).abs() < 1e-9);
    }

    #[test]
    fn test_saliency_score_fallback_to_calls() {
        // No cycles data
        let hotspot = HotspotData {
            top_functions: vec![
                HotspotEntry {
                    function: "a".into(),
                    call_count: 3,
                    cycles: None,
                    avg_cycles_per_call: None,
                },
                HotspotEntry {
                    function: "b".into(),
                    call_count: 7,
                    cycles: None,
                    avg_cycles_per_call: None,
                },
            ],
            total_calls: 10,
        };
        let ct = CompressedTrace::new(executive()).with_hotspot(hotspot);
        let score_a = ct.saliency_score("a").unwrap();
        let score_b = ct.saliency_score("b").unwrap();
        assert!((score_a - 0.3).abs() < 1e-9);
        assert!((score_b - 0.7).abs() < 1e-9);
    }

    #[test]
    fn test_saliency_score_unknown_function_returns_none() {
        let hotspot = HotspotData {
            top_functions: vec![],
            total_calls: 0,
        };
        let ct = CompressedTrace::new(executive()).with_hotspot(hotspot);
        assert!(ct.saliency_score("nonexistent").is_none());
    }

    #[test]
    fn test_saliency_score_without_hotspot_returns_none() {
        let ct = CompressedTrace::new(executive());
        assert!(ct.saliency_score("main").is_none());
    }
}
