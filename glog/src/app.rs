use std::{path::PathBuf, sync::Arc, time::Duration};

use log::info;
use ratatui::style::Style;
use stopwatch::Stopwatch;
use tokio::{pin, select, sync::mpsc, task::JoinHandle};
use tokio_stream::StreamExt;
use tui_textarea::TextArea;

use crate::{
    stateful_list::{Selectable, StatefulList},
    utils::WarnOnErr,
};

// TODO: confirm most of actions so user knows something happened. Like 'yy'

#[derive(Clone)]
pub struct Entry {
    pub git: git::LogEntry,
    selected: bool,
}

impl Entry {
    pub fn new(git: git::LogEntry) -> Self {
        Self {
            git,
            selected: false,
        }
    }

    pub fn selected(&self) -> bool {
        self.selected
    }
}

impl Selectable for Entry {
    fn selected(&self) -> bool {
        self.selected
    }

    fn toggle_selected(&mut self) {
        self.selected ^= true
    }
}

#[derive(Clone)]
pub enum Mode {
    Normal,
    Command(Option<String>),
}

pub enum LoaderError {
    NoData,
    GitLog(std::io::Error),
}
enum LoaderEvent {
    FirstData {
        data: Vec<Entry>,
        duration: Duration,
        last_sha: Option<String>,
    },
    Data(Vec<Entry>),
    Done(Duration),
    Error(LoaderError),
}

pub struct App<'a> {
    mode: Mode,
    pub should_quit: bool,
    pub log: StatefulList<Entry>,

    repository: PathBuf,
    pub revision_range: Vec<String>,

    pub status: String,
    pub textarea: TextArea<'a>,

    log_receiver: mpsc::UnboundedReceiver<LoaderEvent>,
    log_sender: mpsc::UnboundedSender<LoaderEvent>,

    reload_task: Option<JoinHandle<()>>,
    reload_mutex: Arc<tokio::sync::Mutex<()>>,
}

impl<'a> App<'a> {
    pub fn new(repository: PathBuf, revision_range: Vec<String>) -> App<'a> {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        let (log_sender, log_receiver) = mpsc::unbounded_channel();
        App {
            mode: Mode::Normal,
            should_quit: false,
            log: StatefulList::new(),
            repository,
            revision_range,
            status: String::new(),
            textarea,
            log_receiver,
            log_sender,
            reload_task: None,
            reload_mutex: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    // Run though 'select!' in main loop to get data processing running
    pub async fn process(&mut self) {
        select! {
            Some(loader_event) = self.log_receiver.recv() => {
                match loader_event {
                    LoaderEvent::FirstData { data, duration, last_sha } => {
                        self.log.reset();
                        self.log.push(data);
                        if let Some(last_sha) = last_sha {
                            self.goto(&last_sha);
                        }
                        self.status = format!(
                            "Loaded first {} elements. Took: {}.{}s - loading rest in background..",
                            self.log.len(),
                            duration.as_secs(),
                            (duration.as_millis() % 1000) / 100
                        );
                    },
                    LoaderEvent::Data(data) => {
                        self.log.push(data);
                    },
                    LoaderEvent::Done(duration) => {
                        // TODO: add 'LOADING as last displayed item'
                        // TODO: maybe display element_pos/count (and counter with 123+ when loading)
                        self.status = format!(
                            "Loaded all {} elements. Took: {}.{}s.",
                            self.log.len(),
                            duration.as_secs(),
                            (duration.as_millis() % 1000) / 100
                        );
                    },
                    LoaderEvent::Error(LoaderError::NoData) => {
                        self.status = "No log data!".to_string();
                    },
                    LoaderEvent::Error(LoaderError::GitLog(error)) => {
                        // TODO: this assignement should be a function so we can remove mutlilines,
                        // shorted it etc.
                        self.status = format!("Could not get data: {error}");
                    }
                };
            }
        }
    }

    pub fn title(&self) -> String {
        let mut title = self.repository_path();
        if let Some(item) = self.log.current() {
            title.push_str(" - ");
            title.push_str(&item.git.reached_by);
            title.push(' ');
        }
        title
    }

    // Triggers asynchronous reload of data
    pub fn reload(&mut self, revision_range: Option<Vec<String>>) {
        if let Some(rev) = revision_range {
            self.revision_range = rev;
            info!("New arguments for log: {:?}", self.revision_range);
        }

        let last_sha = self.current_sha();
        self.log.reset();
        self.status = "Reloading data".to_owned();

        let repository = self.repository.clone();
        let revision_range = self.revision_range.clone();
        let sender = self.log_sender.clone();

        let reload_mutex = Arc::clone(&self.reload_mutex);
        let reload_future = async move {
            // tokio::Mutex is taken to ensure that only one future runs at a time
            let _lock = reload_mutex.lock();
            let timer = Stopwatch::start_new();

            let data_in_chunks = git::get_log_data(&repository, &revision_range).await;
            if let Err(error) = data_in_chunks {
                sender
                    .send(LoaderEvent::Error(LoaderError::GitLog(error)))
                    .warn_on_err("Reload: queue error.");
                return;
            }
            let data_in_chunks = data_in_chunks.unwrap().map(Entry::new);

            pin!(data_in_chunks); // so it can be used in async loops

            // TODO: refactor
            {
                let mut data: Vec<Entry> = vec![];
                for _ in 0..100 {
                    if let Some(x) = data_in_chunks.next().await {
                        data.push(x);
                    } else {
                        break;
                    }
                }

                // first chunk is important because it's the first delay to user
                if data.is_empty() {
                    sender
                        .send(LoaderEvent::Error(LoaderError::NoData))
                        .warn_on_err("Reload: queue error.");
                }
                sender
                    .send(LoaderEvent::FirstData {
                        data,
                        duration: timer.elapsed(),
                        last_sha,
                    })
                    .warn_on_err("Reload: queue error.");
            }

            'data_processing: loop {
                let mut data: Vec<Entry> = vec![];
                for _ in 0..100 {
                    if let Some(x) = data_in_chunks.next().await {
                        data.push(x);
                    } else {
                        if data.is_empty() {
                            break 'data_processing;
                        }
                        break;
                    }
                }
                sender
                    .send(LoaderEvent::Data(data))
                    .warn_on_err("Reload: queue error.");
            }

            sender
                .send(LoaderEvent::Done(timer.elapsed()))
                .warn_on_err("Reload: queue error.");
        };

        if let Some(reload_task) = &self.reload_task {
            reload_task.abort();
        }
        self.reload_task = Some(tokio::spawn(reload_future));
    }

    pub fn next(&mut self, count: usize) -> Option<()> {
        self.log.scroll_next(count);
        Some(())
    }
    pub fn prev(&mut self, count: usize) -> Option<()> {
        self.log.scroll_prev(count);
        Some(())
    }

    pub fn current_sha(&self) -> Option<String> {
        let item = self.log.current()?;
        if item.git.hash.is_empty() {
            None
        } else {
            Some(item.git.hash.clone())
        }
    }
    pub fn repository_path(&self) -> String {
        // TODO: resolve the repo path
        self.repository
            .clone()
            .into_os_string()
            .into_string()
            .unwrap_or_else(|_| "<unknown>".to_string())
    }

    pub fn top(&mut self) {
        self.log.scroll_start()
    }

    pub fn bottom(&mut self) {
        self.log.scroll_end()
    }

    pub fn next_node(&mut self) -> Option<()> {
        let selected = self.log.current_position();
        let reached_by = &self.log.current()?.git.reached_by;
        let next = self
            .log
            .iter_all()
            .map(|v| &v.git.reached_by)
            .enumerate()
            .skip(selected + 1)
            .find(|v| !v.1.is_empty() && v.1.ne(reached_by))
            .map(|v| v.0)?;
        self.log.scroll_to_position(next);
        Some(())
    }

    pub fn prev_node(&mut self) -> Option<()> {
        let selected = self.log.current_position();
        // if previous is the same as current find it's top
        // else previous is different but still we need to find it's top
        // so either way just find a top of previous
        // NOTE: we need to skip empty nodes
        let (selected, reached_by) = self
            .log
            .iter_all()
            .map(|v| &v.git.reached_by)
            .enumerate()
            .take(selected)
            .rfind(|v| !v.1.is_empty())?;
        let prev = self
            .log
            .iter_all()
            .map(|v| &v.git.reached_by)
            .enumerate()
            .take(selected)
            .rfind(|v| v.1.ne(reached_by))
            .map(|v| v.0)?;
        self.log.scroll_to_position(prev + 1);
        Some(())
    }

    pub fn center_node(&mut self) -> Option<()> {
        self.log.center();
        Some(())
    }

    // TODO: change return type to something that can be used to display status etc?
    pub fn select(&mut self) -> Option<()> {
        self.log.toggle_select_for_current()
    }

    pub fn mode_set(&mut self, mode: Mode) {
        match &mode {
            Mode::Normal => {}
            Mode::Command(cmd) => {
                // Remove input for next search. Do not recreate `self.textarea` instance to keep undo history so that users can
                // restore previous input easily.
                self.textarea.move_cursor(tui_textarea::CursorMove::End);
                self.textarea.delete_line_by_head();
                self.textarea.insert_char(':');
                if let Some(cmd) = cmd {
                    self.textarea.insert_str(cmd);
                }
            }
        }
        self.mode = mode;
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn goto(&mut self, hash: &str) -> Option<()> {
        let pos = self
            .log
            .iter_all()
            // TODO: fix this
            .position(|e| e.git.hash.starts_with(hash))?;
        self.log.scroll_to_position(pos);
        Some(())
    }
}
