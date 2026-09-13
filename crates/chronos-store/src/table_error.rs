//! Classification of `redb` table-open failures on **read** paths.
//!
//! `redb` reports a database that exists but was never written to as
//! `TableError::TableDoesNotExist` when a table is opened in a read
//! transaction. For a read that is not a failure: it means "nothing has been
//! stored yet", and every read path wants to answer accordingly (`Ok(None)`,
//! `Ok(false)`, `SessionNotFound`).
//!
//! Collapsing that case with every *other* table error is worse than it looks.
//! `ContentStore::get` is called in a loop by `SessionStore::load_session`
//! (`if let Some(evt) = self.cas.get(h)? { events.push(evt) }`), so answering a
//! storage-level fault with `Ok(None)` silently *drops events* from the loaded
//! session instead of reporting it.
//!
//! m9-72 closes `FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE` by giving the
//! four read paths in `storage.rs` and `cas.rs` one classifier to share.

use crate::error::StoreError;

/// Outcome of classifying a read-path `open_table` failure.
pub(crate) enum TableOpenFailure {
    /// The table does not exist yet (virgin database): "nothing stored".
    Absent,
    /// Any other storage-level failure, which must propagate.
    Fault(redb::TableError),
}

/// Classify a failed `open_table` on a read path.
///
/// `redb::TableError` is `#[non_exhaustive]`, so the wildcard arm is not
/// hypothetical: every variant other than `TableDoesNotExist` — including ones
/// redb may add later, and ones already reachable such as `TableTypeMismatch`
/// from a database written by a different binary — is a fault, never "absent".
pub(crate) fn classify_read_table_error(e: redb::TableError) -> TableOpenFailure {
    match e {
        redb::TableError::TableDoesNotExist(_) => TableOpenFailure::Absent,
        other => TableOpenFailure::Fault(other),
    }
}

impl TableOpenFailure {
    /// Answer an absent table with `not_found`; propagate a fault.
    // `StoreError` carries `redb::Error`, which is large; every `Result` in this
    // crate that returns it carries the same allow (see `cas.rs`, `storage.rs`).
    #[allow(clippy::result_large_err)]
    pub(crate) fn or_not_found<T>(self, not_found: T) -> Result<T, StoreError> {
        match self {
            TableOpenFailure::Absent => Ok(not_found),
            TableOpenFailure::Fault(e) => Err(StoreError::Database(e.into())),
        }
    }

    /// Answer an absent table with `SessionNotFound(session_id)`; propagate a fault.
    pub(crate) fn session_not_found(self, session_id: &str) -> StoreError {
        match self {
            TableOpenFailure::Absent => StoreError::SessionNotFound(session_id.to_string()),
            TableOpenFailure::Fault(e) => StoreError::Database(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absent table is the virgin-database case: it must read as "nothing
    /// stored", never as an error.
    #[test]
    fn test_absent_table_is_not_a_fault() {
        let classified =
            classify_read_table_error(redb::TableError::TableDoesNotExist("sessions".to_string()));
        assert!(matches!(classified, TableOpenFailure::Absent));
    }

    /// Any other table error is a storage fault. Before m9-72 the four read
    /// paths matched `Err(_)` and answered "not found", so a fault was
    /// indistinguishable from an empty store.
    #[test]
    fn test_non_absent_table_error_is_a_fault() {
        let classified =
            classify_read_table_error(redb::TableError::TableIsNotMultimap("sessions".to_string()));
        match classified {
            TableOpenFailure::Fault(_) => {}
            TableOpenFailure::Absent => {
                panic!("a storage-level table error must not classify as Absent")
            }
        }
    }

    /// `or_not_found` is the shape `cas.rs` needs: absent table -> the given
    /// "not found" value, fault -> propagated error.
    #[test]
    fn test_or_not_found_maps_absent_and_propagates_fault() {
        let absent =
            classify_read_table_error(redb::TableError::TableDoesNotExist("cas".to_string()));
        assert_eq!(absent.or_not_found(None::<u8>).unwrap(), None);

        let fault =
            classify_read_table_error(redb::TableError::TableIsNotMultimap("cas".to_string()));
        assert!(matches!(
            fault.or_not_found(None::<u8>),
            Err(StoreError::Database(_))
        ));
    }

    /// `session_not_found` is the shape `storage.rs` needs: absent table ->
    /// `SessionNotFound`, fault -> propagated error (not a missing session).
    #[test]
    fn test_session_not_found_maps_absent_and_propagates_fault() {
        let absent =
            classify_read_table_error(redb::TableError::TableDoesNotExist("sessions".to_string()));
        assert!(matches!(
            absent.session_not_found("s1"),
            StoreError::SessionNotFound(ref id) if id == "s1"
        ));

        let fault =
            classify_read_table_error(redb::TableError::TableIsNotMultimap("sessions".to_string()));
        assert!(matches!(
            fault.session_not_found("s1"),
            StoreError::Database(_)
        ));
    }
}
