//! Sandbox session management.
//!
//! Satisfies Requirement: sandbox-session

/// Session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session has not started.
    Idle,
    /// Session is running and collecting traces.
    Running,
    /// Session has paused collection.
    Paused,
    /// Session has stopped and data can be extracted.
    Stopped,
}

/// A sandbox trace session.
#[derive(Debug, Clone)]
pub struct SandboxSession {
    /// Session ID.
    pub id: String,
    /// Session state.
    pub state: SessionState,
    /// Start time.
    start_time: Option<std::time::Instant>,
    /// Stop time.
    stop_time: Option<std::time::Instant>,
    /// Duration in seconds.
    pub duration_sec: Option<f64>,
}

impl SandboxSession {
    /// Creates a new sandbox session.
    pub fn new(id: String) -> Self {
        Self {
            id,
            state: SessionState::Idle,
            start_time: None,
            stop_time: None,
            duration_sec: None,
        }
    }

    /// Starts the trace session.
    pub fn start(&mut self) {
        self.state = SessionState::Running;
        self.start_time = Some(std::time::Instant::now());
        self.stop_time = None;
        self.duration_sec = None;
    }

    /// Pauses the trace session.
    pub fn pause(&mut self) {
        if self.state == SessionState::Running {
            self.state = SessionState::Paused;
        }
    }

    /// Resumes a paused session.
    pub fn resume(&mut self) {
        if self.state == SessionState::Paused {
            self.state = SessionState::Running;
        }
    }

    /// Stops the trace session.
    pub fn stop(&mut self) {
        self.state = SessionState::Stopped;
        self.stop_time = Some(std::time::Instant::now());

        if let Some(start) = self.start_time {
            if let Some(stop) = self.stop_time {
                self.duration_sec = Some((stop - start).as_secs_f64());
            }
        }
    }

    /// Returns the session duration in seconds.
    pub fn duration(&self) -> Option<f64> {
        self.duration_sec
    }

    /// Returns the current status.
    pub fn status(&self) -> SessionState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle() {
        let mut session = SandboxSession::new("test-1".to_string());
        assert_eq!(session.state, SessionState::Idle);

        session.start();
        assert_eq!(session.state, SessionState::Running);
        assert!(session.start_time.is_some());

        session.stop();
        assert_eq!(session.state, SessionState::Stopped);
        assert!(session.duration().is_some());
    }

    #[test]
    fn test_session_pause_resume() {
        let mut session = SandboxSession::new("test-2".to_string());
        session.start();
        assert_eq!(session.state, SessionState::Running);

        session.pause();
        assert_eq!(session.state, SessionState::Paused);

        session.resume();
        assert_eq!(session.state, SessionState::Running);
    }
}
