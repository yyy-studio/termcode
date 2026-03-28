/// Trait for clipboard access, allowing dependency injection and testing.
pub trait ClipboardProvider: Send {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str) -> anyhow::Result<()>;
}
