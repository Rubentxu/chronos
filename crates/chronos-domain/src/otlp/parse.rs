//! W3C Trace Context parsers (`traceparent`, `tracestate`).
//!
//! These match the spike contract (`/home/rubentxu/m6-spikes/m6.1-skeleton/src/parse.rs`)
//! so any ingestor that consumed the spike continues to work unchanged.

use super::{
    new_external_context, new_span_id, new_trace_flags, new_trace_id, ExternalTraceContext,
    Tracestate,
};

/// Failure modes for [`parse_traceparent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceparentParseError {
    /// Header had a different number of dash-separated segments than the
    /// required 4 (version, trace-id, parent-id, flags).
    WrongSegmentCount {
        /// Actual number of segments observed.
        actual: usize,
    },
    /// A segment did not have the required hex length.
    WrongLength {
        /// Which segment (`"trace-id"`, `"parent-id"`, `"trace-flags"`).
        segment: &'static str,
        /// The length that was actually observed.
        actual: usize,
    },
    /// A segment contained non-hex characters.
    InvalidHex {
        /// Which segment.
        segment: &'static str,
        /// The raw value that was not hex.
        value: String,
    },
    /// Version byte was not `"00"` (the only version currently supported).
    InvalidVersion {
        /// The version string observed.
        version: String,
    },
    /// A segment was all-zero (invalid per W3C §3.2.2.3/§3.2.2.4).
    InvalidAllZero {
        /// Which segment.
        segment: &'static str,
    },
}

impl std::fmt::Display for TraceparentParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongSegmentCount { actual } => {
                write!(f, "expected 4 segments, got {}", actual)
            }
            Self::WrongLength { segment, actual } => {
                write!(f, "segment '{}' has wrong length (got {})", segment, actual)
            }
            Self::InvalidHex { segment, value } => {
                write!(f, "segment '{}' is not valid hex: '{}'", segment, value)
            }
            Self::InvalidVersion { version } => write!(
                f,
                "version '{}' not supported (only 00)",
                version
            ),
            Self::InvalidAllZero { segment } => {
                write!(f, "segment '{}' is all-zero (invalid)", segment)
            }
        }
    }
}

impl std::error::Error for TraceparentParseError {}

/// Failure modes for [`parse_tracestate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TracestateParseError {
    /// More than 32 vendor entries were supplied.
    TooManyEntries {
        /// Number of entries that triggered the failure.
        count: usize,
    },
    /// A single entry exceeded the 256-char budget.
    EntryTooLong {
        /// Length of the offending entry.
        len: usize,
        /// The raw entry text.
        raw: String,
    },
    /// An entry did not contain the `vendor=value` separator.
    MissingEquals {
        /// The raw entry text.
        raw: String,
    },
    /// An entry had an empty vendor string.
    EmptyVendor,
    /// An entry had a vendor string with characters outside the allowed
    /// charset (lowercase alphanum + `-_./*`).
    InvalidVendor {
        /// The raw vendor string that was rejected.
        raw: String,
    },
}

impl std::fmt::Display for TracestateParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyEntries { count } => write!(f, "tracestate has {} entries (>32)", count),
            Self::EntryTooLong { len, raw } => {
                write!(f, "entry too long ({} chars): '{}'", len, raw)
            }
            Self::MissingEquals { raw } => {
                write!(f, "tracestate entry missing '=': '{}'", raw)
            }
            Self::EmptyVendor => write!(f, "tracestate entry has empty vendor"),
            Self::InvalidVendor { raw } => {
                write!(f, "tracestate entry has invalid vendor: '{}'", raw)
            }
        }
    }
}

impl std::error::Error for TracestateParseError {}

/// Parse a W3C `traceparent` header value into an [`ExternalTraceContext`].
///
/// Successful round-trip is `parse_traceparent(s)?.to_traceparent() == s`
/// (validated by the integration tests).
pub fn parse_traceparent(s: &str) -> Result<ExternalTraceContext, TraceparentParseError> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 4 {
        return Err(TraceparentParseError::WrongSegmentCount {
            actual: parts.len(),
        });
    }
    let version = parts[0];
    let trace_id_hex = parts[1];
    let parent_id_hex = parts[2];
    let flags_hex = parts[3];
    if version != "00" {
        return Err(TraceparentParseError::InvalidVersion {
            version: version.to_string(),
        });
    }
    if trace_id_hex.len() != 32 {
        return Err(TraceparentParseError::WrongLength {
            segment: "trace-id",
            actual: trace_id_hex.len(),
        });
    }
    if parent_id_hex.len() != 16 {
        return Err(TraceparentParseError::WrongLength {
            segment: "parent-id",
            actual: parent_id_hex.len(),
        });
    }
    if flags_hex.len() != 2 {
        return Err(TraceparentParseError::WrongLength {
            segment: "trace-flags",
            actual: flags_hex.len(),
        });
    }
    let trace_id_bytes = parse_hex_16(trace_id_hex).ok_or_else(|| {
        TraceparentParseError::InvalidHex {
            segment: "trace-id",
            value: trace_id_hex.to_string(),
        }
    })?;
    let span_id_bytes = parse_hex_8(parent_id_hex).ok_or_else(|| {
        TraceparentParseError::InvalidHex {
            segment: "parent-id",
            value: parent_id_hex.to_string(),
        }
    })?;
    let flags_byte =
        parse_hex_byte(flags_hex).ok_or_else(|| TraceparentParseError::InvalidHex {
            segment: "trace-flags",
            value: flags_hex.to_string(),
        })?;
    if trace_id_bytes == [0u8; 16] {
        return Err(TraceparentParseError::InvalidAllZero {
            segment: "trace-id",
        });
    }
    if span_id_bytes == [0u8; 8] {
        return Err(TraceparentParseError::InvalidAllZero {
            segment: "parent-id",
        });
    }
    Ok(new_external_context(
        new_trace_id(trace_id_bytes),
        new_span_id(span_id_bytes),
        new_trace_flags(flags_byte),
        None,
    ))
}

/// Parse a W3C `tracestate` header value into a [`Tracestate`].
pub fn parse_tracestate(s: &str) -> Result<Tracestate, TracestateParseError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Tracestate::default());
    }
    let entries: Vec<&str> = s.split(',').collect();
    if entries.len() > 32 {
        return Err(TracestateParseError::TooManyEntries {
            count: entries.len(),
        });
    }
    let mut parsed = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.len() > 256 {
            return Err(TracestateParseError::EntryTooLong {
                len: entry.len(),
                raw: entry.to_string(),
            });
        }
        let eq_pos = entry
            .find('=')
            .ok_or_else(|| TracestateParseError::MissingEquals {
                raw: entry.to_string(),
            })?;
        let vendor = &entry[..eq_pos];
        let value = &entry[eq_pos + 1..];
        if vendor.is_empty() {
            return Err(TracestateParseError::EmptyVendor);
        }
        if !vendor
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-_.*".contains(c))
        {
            return Err(TracestateParseError::InvalidVendor {
                raw: vendor.to_string(),
            });
        }
        parsed.push((vendor.to_string(), value.to_string()));
    }
    Ok(Tracestate { entries: parsed })
}

fn parse_hex_16(s: &str) -> Option<[u8; 16]> {
    let bytes = parse_hex_bytes(s)?;
    if bytes.len() != 16 {
        return None;
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&bytes);
    Some(out)
}

fn parse_hex_8(s: &str) -> Option<[u8; 8]> {
    let bytes = parse_hex_bytes(s)?;
    if bytes.len() != 8 {
        return None;
    }
    let mut out = [0u8; 8];
    out.copy_from_slice(&bytes);
    Some(out)
}

fn parse_hex_byte(s: &str) -> Option<u8> {
    if s.len() != 2 {
        return None;
    }
    u8::from_str_radix(s, 16).ok()
}

fn parse_hex_bytes(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let h = hex_value(chars[i])?;
        let l = hex_value(chars[i + 1])?;
        bytes.push((h << 4) | l);
        i += 2;
    }
    Some(bytes)
}

fn hex_value(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some(c as u8 - b'0'),
        'a'..='f' => Some(c as u8 - b'a' + 10),
        'A'..='F' => Some(c as u8 - b'A' + 10),
        _ => None,
    }
}
