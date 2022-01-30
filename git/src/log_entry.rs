use crate::ref_names::RefNames;

#[derive(Debug, Clone)] // TODO: remove clone!
pub struct LogEntry {
    pub graph: String,
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
    pub refs: Option<RefNames>,
    /// Command line ref via which this commit was reached
    pub reached_by: String,
}

impl LogEntry {
    pub fn author_and_date(&self) -> String {
        if self.author.is_empty() {
            "".to_string()
        } else {
            format!("({}, {})", self.author, self.date)
        }
    }
}
