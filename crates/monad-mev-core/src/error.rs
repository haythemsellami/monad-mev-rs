use std::fmt::{Display, Formatter};

use crate::{GapEvent, PayloadExpired, SchemaMismatch};

/// Project-wide error type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// A descriptor sequence gap was detected.
    Gap(GapEvent),
    /// An event descriptor was available but its payload had expired.
    PayloadExpired(PayloadExpired),
    /// A source schema hash does not match the compiled decoder schema.
    SchemaMismatch(SchemaMismatch),
    /// A catch-all message for errors that do not yet have a stable variant.
    Message(String),
}

/// Project-wide result type.
pub type Result<T> = std::result::Result<T, Error>;

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gap(gap) => Display::fmt(gap, formatter),
            Self::PayloadExpired(expired) => Display::fmt(expired, formatter),
            Self::SchemaMismatch(mismatch) => Display::fmt(mismatch, formatter),
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

impl From<GapEvent> for Error {
    fn from(value: GapEvent) -> Self {
        Self::Gap(value)
    }
}

impl From<PayloadExpired> for Error {
    fn from(value: PayloadExpired) -> Self {
        Self::PayloadExpired(value)
    }
}

impl From<SchemaMismatch> for Error {
    fn from(value: SchemaMismatch) -> Self {
        Self::SchemaMismatch(value)
    }
}
