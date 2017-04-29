
use xz_decom::XZError;

use std::error::Error;
use std;
use byteorder;

/// An error type for parsing errors
pub struct ParsingError {
    pub msg: &'static str,
    pub cause: Option<Box<Error>>
}

impl From<XZError> for ParsingError {
    fn from(e: XZError) -> ParsingError {
        ParsingError {
            msg: "Error decoding compressed data",
            cause: Some(Box::new(e))
        }
    }
}

impl From<byteorder::Error> for ParsingError {
    fn from(e: byteorder::Error) -> ParsingError {
        ParsingError {
            msg: "Error reading bytestream",
            cause: Some(Box::new(e))
        }
    }
}

impl From<std::string::FromUtf8Error> for ParsingError {
    fn from(e: std::string::FromUtf8Error) -> ParsingError {
        ParsingError {
            msg: "Error converting to string",
            cause: Some(Box::new(e))
        }
    }
}

impl From<std::io::Error> for ParsingError {
    fn from(e: std::io::Error) -> ParsingError {
        ParsingError {
            msg: "Error reading bytestream",
            cause: Some(Box::new(e))
        }
    }
}

