//! Asking GitHub whether a newer release exists.
//!
//! The check is a single HTTPS request on a thread of its own, and its answer
//! comes back as an [`AppEvent::Update`] through the same channel the LSP
//! bridge uses. Nothing here touches `App`: a network call that blocked the
//! event loop would freeze the editor for as long as the network took, and the
//! loop already knows how to drain events that arrive from elsewhere.
//!
//! Installing is handed straight back to `install.sh`, and deliberately so.
//! That script already knows where the binary and the `runtime/` directory go,
//! that macOS quarantines a downloaded executable, and how to leave an existing
//! `config.toml` alone. A second implementation inside the editor would have to
//! learn all of it again and would be the copy that goes stale. What lives here
//! is the decision of *whether* to run it -- see [`install_readiness`] -- and
//! [`run_installer`], which runs it once the terminal has been given back.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::event::AppEvent;

/// Where releases are published.
const REPO: &str = "yyy-studio/termcode";

/// How long a successful check's answer is trusted before another is made.
///
/// A check on every start would be a request per file opened from the shell,
/// which is both rude to the API and pointless: releases do not appear that
/// often.
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Long enough for a slow network, short enough that a hung connection does not
/// leave a thread alive for the session.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// What the user runs to install the new version. The installer already knows
/// how to preserve a config and where the runtime directory goes, so the row
/// hands this over instead of reimplementing any of it.
pub const INSTALL_COMMAND: &str =
    "curl -fsSL https://raw.githubusercontent.com/yyy-studio/termcode/main/install.sh | sh";

/// Why an install cannot be handed to `install.sh` from in here.
///
/// Every one of these is a refusal to touch a binary the editor did not put
/// there. Guessing wrong overwrites somebody else's file: a package manager's,
/// Cargo's, or a build in `target/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallBlocker {
    /// There is no `install.sh` for this platform. Windows is installed from
    /// the release archive by hand, and always has been.
    UnsupportedPlatform,
    /// The running executable is not the one the installer manages.
    NotInstalledByScript(PathBuf),
    /// The path of the running executable, or the home directory, is unknown.
    UnknownLocation,
    /// The installer is a shell script fetched with curl.
    MissingTool(&'static str),
}

impl InstallBlocker {
    /// Short enough for the settings screen's value column.
    pub fn summary(&self) -> String {
        match self {
            InstallBlocker::UnsupportedPlatform => "not on Windows".to_string(),
            InstallBlocker::NotInstalledByScript(_) | InstallBlocker::UnknownLocation => {
                "not managed".to_string()
            }
            InstallBlocker::MissingTool(tool) => format!("no {tool}"),
        }
    }

    /// The whole reason, for the hint line and the status bar.
    pub fn reason(&self) -> String {
        match self {
            InstallBlocker::UnsupportedPlatform => {
                "There is no installer script for Windows -- download the release archive"
                    .to_string()
            }
            InstallBlocker::NotInstalledByScript(exe) => format!(
                "{} was not installed by install.sh; update it the way you installed it",
                exe.display()
            ),
            InstallBlocker::UnknownLocation => {
                "Cannot tell where this binary lives, so it will not be replaced".to_string()
            }
            InstallBlocker::MissingTool(tool) => {
                format!("'{tool}' is not on PATH, and the installer needs it")
            }
        }
    }
}

/// Whether an install can be handed to `install.sh` from inside the editor.
pub fn install_readiness() -> Result<(), InstallBlocker> {
    if cfg!(windows) {
        return Err(InstallBlocker::UnsupportedPlatform);
    }
    let exe = std::env::current_exe().map_err(|_| InstallBlocker::UnknownLocation)?;
    let expected =
        termcode_config::default::installed_binary_path().ok_or(InstallBlocker::UnknownLocation)?;
    if !is_same_file(&exe, &expected) {
        return Err(InstallBlocker::NotInstalledByScript(exe));
    }
    for tool in ["sh", "curl"] {
        if !on_path(tool) {
            return Err(InstallBlocker::MissingTool(tool));
        }
    }
    Ok(())
}

/// Whether two paths name the same file.
///
/// Canonicalised, because `~/.local/bin/termcode` may be a symlink and
/// `current_exe` resolves to what it points at; a plain comparison would then
/// refuse an install that is perfectly fine. A path that cannot be
/// canonicalised -- the expected one, when nothing is installed there yet --
/// falls back to comparing it as written, which simply does not match.
fn is_same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(tool).is_file()))
}

/// Run the installer, with the terminal already given back to the shell.
///
/// Called from `App::run` **after** `restore_terminal`, so the script's own
/// output and any prompt it makes reach a terminal in its normal state. Running
/// it from inside the event loop would print into the alternate screen with raw
/// mode still on.
///
/// The editor is not restarted afterwards. Replacing the file underneath a
/// running process is fine on Unix, but the process running is still the old
/// one, and re-executing it would run the old code while claiming to be the
/// update.
pub fn run_installer() -> anyhow::Result<()> {
    println!("Updating termcode:");
    println!("  {INSTALL_COMMAND}");
    println!();

    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(INSTALL_COMMAND)
        .status()?;

    if !status.success() {
        anyhow::bail!("the installer exited with {status}");
    }
    println!("\nStart termcode again to run the new version.");
    Ok(())
}

/// The version this binary was built as.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// A release tag, compared numerically rather than as text.
///
/// `0.10.0` is newer than `0.9.0`, which string comparison gets backwards, and
/// that is the whole reason this type exists. Anything after a `-` is a
/// pre-release, which sorts *before* the same numbers without one, as semver
/// says: `1.0.0-rc.1` is not yet `1.0.0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    parts: [u64; 3],
    prerelease: Option<String>,
}

impl Version {
    /// Parse `v0.5.1`, `0.5.1` or `1.0.0-rc.1`. Returns `None` for anything
    /// that is not three dot-separated numbers or fewer, so a tag in some other
    /// shape is reported rather than silently compared as zeroes.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        let (core, prerelease) = match text.split_once('-') {
            Some((core, rest)) => (core, Some(rest.to_string())),
            None => (text, None),
        };

        let mut parts = [0u64; 3];
        let mut seen = 0;
        for (i, field) in core.split('.').enumerate() {
            // A fourth field is not a version this scheme can order, and
            // guessing at one would be worse than saying so.
            let slot = parts.get_mut(i)?;
            *slot = field.parse().ok()?;
            seen += 1;
        }
        (seen > 0).then_some(Self { parts, prerelease })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.parts.cmp(&other.parts).then_with(|| {
            match (&self.prerelease, &other.prerelease) {
                // A release beats its own pre-releases; between two
                // pre-releases of the same version, alphabetical is as good an
                // answer as any and never claims one is newer than a release.
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The published release a check found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    /// The tag with any `v` stripped, so it reads the way `current_version`
    /// does.
    pub version: String,
    /// The release page, for a user who would rather download it by hand.
    pub url: String,
}

/// Where the check has got to. `App` holds one of these and the settings screen
/// reads it; there is no second copy of this state anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateStatus {
    /// No check has been made yet this run.
    #[default]
    Idle,
    /// A check is in flight.
    Checking,
    Latest,
    Available(Release),
    /// The check could not be completed. Offline is the ordinary case, so this
    /// is shown where it was asked for and never raised as an error.
    Failed(String),
}

impl UpdateStatus {
    /// How the status reads in the settings screen's value column.
    ///
    /// That column is fourteen or so cells wide -- every other value in it is a
    /// `[x]`, a number or a key name -- so this stays a version and nothing
    /// else. Whether that version is worth having is [`Self::detail`]'s job,
    /// on the much wider hint line.
    pub fn summary(&self) -> String {
        match self {
            UpdateStatus::Idle => "not checked".to_string(),
            UpdateStatus::Checking => "checking...".to_string(),
            UpdateStatus::Latest => format!("v{}", current_version()),
            UpdateStatus::Available(release) => format!("v{}", release.version),
            UpdateStatus::Failed(_) => "unavailable".to_string(),
        }
    }

    /// The sentence under the row, where there is room to say what the version
    /// in the column means.
    pub fn detail(&self) -> String {
        match self {
            UpdateStatus::Idle => "Not checked yet -- Check Now asks GitHub".to_string(),
            UpdateStatus::Checking => "Asking GitHub...".to_string(),
            UpdateStatus::Latest => "You are running the newest release".to_string(),
            UpdateStatus::Available(release) => format!(
                "v{} is newer than the v{} you are running",
                release.version,
                current_version()
            ),
            UpdateStatus::Failed(reason) => format!("Check failed: {reason}"),
        }
    }
}

/// Where a check gets its answer.
///
/// A trait rather than a bare function so the tests can hand over a release
/// without a network: CI has none, and a check that can only be exercised
/// against the real API is a check that is never exercised.
pub trait ReleaseSource: Send + 'static {
    fn latest(&self) -> anyhow::Result<Release>;
}

/// The GitHub releases API.
pub struct GitHubReleases;

impl ReleaseSource for GitHubReleases {
    fn latest(&self) -> anyhow::Result<Release> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .into();
        let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
        let body = agent
            .get(&url)
            // The API rejects a request with no User-Agent outright.
            .header(
                "User-Agent",
                concat!("termcode/", env!("CARGO_PKG_VERSION")),
            )
            .header("Accept", "application/vnd.github+json")
            .call()?
            .body_mut()
            .read_to_string()?;
        parse_release(&body)
    }
}

/// Pull the tag and the page out of a releases API response.
fn parse_release(body: &str) -> anyhow::Result<Release> {
    let value: serde_json::Value = serde_json::from_str(body)?;
    let tag = value
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("no tag_name in the release"))?;
    let url = value
        .get("html_url")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://github.com/{REPO}/releases/latest"));
    Ok(Release {
        version: tag.trim_start_matches('v').to_string(),
        url,
    })
}

/// The last answer, so a start within [`CACHE_TTL`] of the previous one makes
/// no request at all and still knows what it knew.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateCache {
    /// Unix seconds of the last successful check.
    pub last_check: u64,
    pub latest: Option<Release>,
}

fn cache_path() -> PathBuf {
    termcode_config::default::config_dir().join("update.json")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl UpdateCache {
    /// A missing or unreadable cache is simply an empty one: it holds nothing
    /// that cannot be fetched again.
    pub fn load() -> Self {
        std::fs::read_to_string(cache_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = cache_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Whether the stored answer is recent enough to use as-is.
    ///
    /// A `last_check` in the future -- a clock that was wound back -- is not
    /// fresh: `saturating_sub` makes the difference zero, so it is treated as
    /// having just happened, which is the harmless direction. Being one day
    /// late with a version number costs nothing.
    pub fn is_fresh(&self, now: u64) -> bool {
        self.latest.is_some() && now.saturating_sub(self.last_check) < CACHE_TTL.as_secs()
    }
}

/// Compare a release against this binary.
pub fn compare(latest: &Release) -> UpdateStatus {
    let (Some(current), Some(found)) = (
        Version::parse(current_version()),
        Version::parse(&latest.version),
    ) else {
        return UpdateStatus::Failed(format!("cannot read version '{}'", latest.version));
    };
    if found > current {
        UpdateStatus::Available(latest.clone())
    } else {
        UpdateStatus::Latest
    }
}

/// Run a check and report what it found, reading the cache unless `force`.
///
/// Separate from the thread that calls it so a test can run the whole decision
/// -- cache, request, comparison -- without spawning anything.
pub fn check(source: &dyn ReleaseSource, force: bool) -> UpdateStatus {
    if !force {
        let cache = UpdateCache::load();
        if cache.is_fresh(now_secs()) {
            if let Some(release) = &cache.latest {
                return compare(release);
            }
        }
    }
    match source.latest() {
        Ok(release) => {
            let cache = UpdateCache {
                last_check: now_secs(),
                latest: Some(release.clone()),
            };
            if let Err(e) = cache.save() {
                log::warn!("Could not write the update cache: {e}");
            }
            compare(&release)
        }
        Err(e) => UpdateStatus::Failed(e.to_string()),
    }
}

/// Check on a thread and post the answer back through `tx`.
///
/// The thread is detached: a send into a closed channel is what a quit during
/// the request looks like, and it is discarded rather than waited for.
pub fn spawn_check(tx: mpsc::UnboundedSender<AppEvent>, force: bool) {
    spawn_check_with(tx, force, Box::new(GitHubReleases));
}

pub fn spawn_check_with(
    tx: mpsc::UnboundedSender<AppEvent>,
    force: bool,
    source: Box<dyn ReleaseSource>,
) {
    std::thread::spawn(move || {
        let status = check(source.as_ref(), force);
        let _ = tx.send(AppEvent::Update(status));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str) -> Release {
        Release {
            version: version.to_string(),
            url: "https://example.invalid".to_string(),
        }
    }

    #[test]
    fn a_version_is_ordered_by_its_numbers_not_its_text() {
        // The case string comparison gets wrong.
        assert!(Version::parse("0.10.0").unwrap() > Version::parse("0.9.0").unwrap());
        assert!(Version::parse("1.0.0").unwrap() > Version::parse("0.99.99").unwrap());
        assert_eq!(Version::parse("v0.5.0"), Version::parse("0.5.0"));
    }

    #[test]
    fn a_prerelease_is_older_than_the_release_it_leads_to() {
        assert!(Version::parse("1.0.0").unwrap() > Version::parse("1.0.0-rc.1").unwrap());
        assert!(Version::parse("1.0.0-rc.2").unwrap() > Version::parse("1.0.0-rc.1").unwrap());
    }

    #[test]
    fn a_short_version_is_padded_and_a_malformed_one_is_refused() {
        assert_eq!(Version::parse("1.2"), Version::parse("1.2.0"));
        assert!(Version::parse("").is_none());
        assert!(Version::parse("nightly").is_none());
        assert!(Version::parse("1.2.3.4").is_none());
    }

    #[test]
    fn a_release_is_read_out_of_the_api_response() {
        let body = r#"{"tag_name":"v9.9.9","html_url":"https://example.invalid/r"}"#;
        let parsed = parse_release(body).unwrap();
        assert_eq!(parsed.version, "9.9.9");
        assert_eq!(parsed.url, "https://example.invalid/r");
    }

    #[test]
    fn a_response_without_a_tag_is_an_error_not_a_version_of_zero() {
        assert!(parse_release(r#"{"message":"Not Found"}"#).is_err());
    }

    #[test]
    fn comparing_reports_the_newer_release_and_nothing_else() {
        assert_eq!(compare(&release("0.0.1")), UpdateStatus::Latest);
        assert_eq!(compare(&release(current_version())), UpdateStatus::Latest);
        assert_eq!(
            compare(&release("999.0.0")),
            UpdateStatus::Available(release("999.0.0"))
        );
    }

    #[test]
    fn an_unreadable_tag_is_reported_rather_than_compared() {
        assert!(matches!(
            compare(&release("nightly")),
            UpdateStatus::Failed(_)
        ));
    }

    #[test]
    fn a_cache_is_stale_once_its_ttl_has_passed_and_never_fresh_when_empty() {
        let cache = UpdateCache {
            last_check: 1_000_000,
            latest: Some(release("1.0.0")),
        };
        assert!(cache.is_fresh(1_000_000));
        assert!(cache.is_fresh(1_000_000 + CACHE_TTL.as_secs() - 1));
        assert!(!cache.is_fresh(1_000_000 + CACHE_TTL.as_secs()));
        // A clock wound back must not read as fresh-forever in the other
        // direction either; it simply looks like the check just happened.
        assert!(cache.is_fresh(0));

        let empty = UpdateCache {
            last_check: 1_000_000,
            latest: None,
        };
        assert!(!empty.is_fresh(1_000_000));
    }

    struct Fixed(anyhow::Result<Release>);

    impl ReleaseSource for Fixed {
        fn latest(&self) -> anyhow::Result<Release> {
            match &self.0 {
                Ok(release) => Ok(release.clone()),
                Err(e) => Err(anyhow::anyhow!("{e}")),
            }
        }
    }

    /// The real API, on demand only: CI has no network, and a test that fails
    /// when GitHub is slow is a test that gets deleted.
    ///
    /// `cargo test -p termcode-term -- --ignored reaches_the_real_api`
    #[test]
    #[ignore = "hits the network"]
    fn reaches_the_real_api() {
        let release = GitHubReleases.latest().expect("the releases API answered");
        assert!(
            Version::parse(&release.version).is_some(),
            "unreadable tag: {release:?}"
        );
    }

    #[test]
    fn a_failed_request_is_a_status_rather_than_an_error() {
        let source = Fixed(Err(anyhow::anyhow!("no route to host")));
        assert!(matches!(
            check(&source, true),
            UpdateStatus::Failed(reason) if reason.contains("no route to host")
        ));
    }
}
