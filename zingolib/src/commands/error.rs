use std::fmt;

#[derive(Debug)]
pub(crate) enum CommandError {
    FailedJsonParsing(json::Error),
    FailedIntParsing(std::num::ParseIntError),
    UnexpectedType,
    MissingKey(String),
    InvalidArguments,
    IncompatibleMemo,
    InvalidMemo(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use CommandError::*;

        match self {
            FailedJsonParsing(e) => write!(f, "failed to parse argument. {}", e),
            FailedIntParsing(e) => write!(f, "failed to parse argument. {}", e),
            UnexpectedType => write!(f, "arguments cannot be parsed to expected type."),
            MissingKey(key) => write!(f, "json array is missing \"{}\" key.", key),
            InvalidArguments => write!(f, "arguments given are invalid."),
            IncompatibleMemo => {
                write!(f, "memo's cannot be sent to transparent addresses.")
            }
            InvalidMemo(e) => write!(f, "failed to interpret memo. {}", e),
        }
    }
}