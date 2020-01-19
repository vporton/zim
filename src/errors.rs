use bitreader;
use std;
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    UnknownCompression,
    UnknownMimeType,
    InvalidMagicNumber,
    InvalidVersion,
    InvalidHeader,
    InvalidClusterExtension,
    MissingBlobList,
    OutOfBounds,
    ParsingError(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(std::error::Error::description(self))
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::UnknownCompression => "unknown compression",
            Error::UnknownMimeType => "unknown mimetype",
            Error::InvalidMagicNumber => "invalid magic number",
            Error::InvalidVersion => "invalid major version, must be 5 or 6",
            Error::InvalidHeader => "invalid header",
            Error::InvalidClusterExtension => "cluster extension requires major version 6",
            Error::MissingBlobList => "cluster is missing a blob list",
            Error::OutOfBounds => "out of bounds access",
            Error::ParsingError(_) => "failed to parse",
        }
    }

    #[inline]
    fn cause(&self) -> Option<&dyn std::error::Error> {
        match *self {
            Error::ParsingError(ref err) => Some(&**err),
            _ => None,
        }
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Error {
        Error::ParsingError(err.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::ParsingError(err.into())
    }
}

impl From<bitreader::BitReaderError> for Error {
    fn from(err: bitreader::BitReaderError) -> Error {
        Error::ParsingError(err.into())
    }
}
