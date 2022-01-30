use crossterm::event::EventStream;
use futures::{select, FutureExt, StreamExt};
use log::debug;

pub enum InputEvent {
    Event(crossterm::event::Event),
    Tick,
}
pub struct Input {
    event_stream: EventStream,
}

impl Input {
    pub fn new() -> Input {
        Input {
            event_stream: EventStream::new(),
        }
    }

    pub async fn next(&mut self) -> InputEvent {
        let mut event = self.event_stream.next().fuse();
        select! {
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(x)) => InputEvent::Event(x),
                    Some(Err(e)) => { debug!("Error: {:?}\r", e); InputEvent::Tick }
                    None => InputEvent::Tick,
                }
            }
        }
    }
}
