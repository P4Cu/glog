pub mod internals;
mod log_entry;
mod ref_names;

use std::path::Path;

use async_stream::stream;
use log::warn;
pub use log_entry::LogEntry;
pub use ref_names::RefNames;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_stream::Stream;

/// Produces a stream of LogEntry for given repository and revision_range.
/// This stream may be used in async manner to allow quick and responsive UI for big amount of
/// elements.
#[allow(clippy::single_char_pattern)] // broken compilation after suggested fix
pub async fn get_log_data(
    repository: &Path,
    revision_range: &[String],
) -> Result<impl Stream<Item = LogEntry>, std::io::Error> {
    let mut child = internals::get_log(repository, revision_range).await?;

    let stdout = child
        .stdout
        .take()
        .expect("git log did not output anything");
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    let s = stream! {
        while let Some(line) = lines.next_line().await.unwrap() {
            yield internals::log_entry_from_split(&mut line.split("\x1f"));
        }

        // handle failure?
        let status = child.wait().await;
        warn!("Process exited with: {:?}", status);
    };
    Ok(s)
}
