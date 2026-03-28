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
            match event::read()? {
                CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                    Ok(AppEvent::Key(key))
                }
                CrosstermEvent::Key(_) => Ok(AppEvent::Tick),
                CrosstermEvent::Mouse(mouse) => Ok(AppEvent::Mouse(mouse)),
                CrosstermEvent::Resize(w, h) => Ok(AppEvent::Resize(w, h)),
                _ => Ok(AppEvent::Tick),
            }
        } else {
            Ok(AppEvent::Tick)
        }
    }
}
