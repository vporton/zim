#[derive(Debug, PartialEq)]
pub enum MimeType {
    /// A special "MimeType" that represents a redirection
    Redirect,
    LinkTarget,
    DeletedEntry,
    Type(String),
}
