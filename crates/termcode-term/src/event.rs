use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent};
use termcode_lsp::types::LspResponse;

/// All events the application can process.
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
    Lsp(LspResponse),
}

/// Event handler: polls crossterm events with a tick rate.
pub struct EventHandler {
    tick_rate: Duration,
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
