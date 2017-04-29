#[derive(Debug, PartialEq)]
pub enum Target {
    /// Redirect specified as a URL index
    Redirect(u32),
    /// Cluster index and blob index
    Cluster(u32, u32),
}
