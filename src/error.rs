use std::{fmt::Display, io::ErrorKind, path::PathBuf};
use thiserror::Error;

use crate::mapper::MapOp;

pub type Result<T = ()> = std::result::Result<T, Errors>;

#[derive(Error, Debug)]
pub enum Errors {
    InvalidRoot(String, PathBuf),
    OutputDirectoryNotFound,
    OutputDirectoryNotEmpty,
    NoVideos,
    ValidationError(OpValidationResult),
    IOError(ErrorKind),
}

#[derive(Debug, Clone)]
pub enum OpValidationResult {
    Valid,
    OverlappingRange(MapOp, MapOp),
    Empty,
}

impl Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Errors::*;
        match self {
            InvalidRoot(root, path) => write!(
                f,
                "{:#?} is not a valid root ({:#?} does not exist).",
                root, path
            ),
            OutputDirectoryNotFound => write!(f, "The output directory does not exist!"),
            OutputDirectoryNotEmpty => write!(f, "The output directory is not empty!"),
            NoVideos => write!(f, "There are no compatible video files in the root folder!"),
            ValidationError(res) => {
                use OpValidationResult::*;
                match res {
                    OverlappingRange(op1, op2) => write!(
                        f,
                        "{} ({}..{}) overlaps {} ({}..{})",
                        op1.name, op1.start, op1.end, op2.name, op2.start, op2.end
                    ),
                    Empty => write!(f, "No operations defined! Exiting."),
                    Valid => panic!(),
                }
            }
            IOError(kind) => write!(f, "Unhandled IO error: {:?}", kind),
        }
    }
}
