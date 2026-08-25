use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent};
use termcode_lsp::types::LspResponse;

use crate::update::UpdateStatus;

/// All events the application can process.
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
    Lsp(LspResponse),
    /// The answer from a background update check.
    Update(UpdateStatus),
}

/// Where `App`'s loop gets its events.
///
/// A trait rather than the concrete handler because the loop is worth testing
/// and crossterm's queue is not something a test can fill: `run()` reads the
/// process's own terminal, so nothing that only *reproduces* the loop's order
/// can prove the loop still performs it. A scripted source lets the real
/// `event_loop` run against a `TestBackend`.
pub trait EventSource {
    /// Block until an event arrives, or until the tick rate expires.
    fn next(&mut self) -> anyhow::Result<AppEvent>;

    /// The next event if one is already waiting, without blocking.
    fn try_next(&mut self) -> anyhow::Result<Option<AppEvent>>;
}

/// Event handler: polls crossterm events with a tick rate.
pub struct EventHandler {
    tick_rate: Duration,
}

impl EventSource for EventHandler {
    fn next(&mut self) -> anyhow::Result<AppEvent> {
        EventHandler::next(self)
    }

    fn try_next(&mut self) -> anyhow::Result<Option<AppEvent>> {
        EventHandler::try_next(self)
    }
}

impl EventHandler {
    pub fn new(tick_rate_ms: u64) -> Self {
        Self {
            tick_rate: Duration::from_millis(tick_rate_ms),
        }
    }

    /// Poll for the next event. Returns Tick if no event within tick_rate.
    pub fn next(&self) -> anyhow::Result<AppEvent> {
        if event::poll(self.tick_rate)? {
            Ok(read_event()?)
        } else {
            Ok(AppEvent::Tick)
        }
    }

    /// The next event if one is already waiting, without blocking.
    ///
    /// Lets the caller take everything the terminal has queued before drawing:
    /// a wheel flick arrives as a long burst, and only the last frame of it was
    /// ever going to be seen.
    pub fn try_next(&self) -> anyhow::Result<Option<AppEvent>> {
        if event::poll(Duration::ZERO)? {
            Ok(Some(read_event()?))
        } else {
            Ok(None)
        }
    }
}

fn read_event() -> anyhow::Result<AppEvent> {
    Ok(match event::read()? {
        CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => AppEvent::Key(key),
        CrosstermEvent::Key(_) => AppEvent::Tick,
        CrosstermEvent::Mouse(mouse) => AppEvent::Mouse(mouse),
        CrosstermEvent::Resize(w, h) => AppEvent::Resize(w, h),
        _ => AppEvent::Tick,
    })
}
