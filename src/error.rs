use std::fmt;

#[derive(Debug)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    #[must_use]
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }

    /// The human-readable error message.
    #[inline]
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}
