//! The `<config-dir>/config.toml` file format, and rendering of the generated file.

use komga_db::pool::{DatabaseConfig, JournalMode};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{OAuth2ClientRegistration, ServerConfig};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileConfig {
    pub server: Option<FileServer>,
    pub cors: Option<FileCors>,
    pub database: Option<FileDatabase>,
    pub tasks_db: Option<FileDatabase>,
    pub search: Option<FileSearch>,
    pub fonts: Option<FileFonts>,
    pub books: Option<FileBooks>,
    pub libraries: Option<FileLibraries>,
    pub kobo: Option<FileKobo>,
    pub oauth2: Option<FileOAuth2>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileServer {
    pub port: Option<u16>,
    pub context_path: Option<String>,
    pub session_timeout: Option<ConfigDuration>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileCors {
    pub allowed_origins: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileDatabase {
    pub file: Option<PathBuf>,
    /// read pool size; None = min(CPU cores, max-pool-size)
    pub pool_size: Option<u32>,
    pub max_pool_size: Option<u32>,
    pub journal_mode: Option<String>,
    pub busy_timeout: Option<ConfigDuration>,
    pub pragmas: Option<HashMap<String, String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileSearch {
    pub data_directory: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileFonts {
    pub data_directory: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileBooks {
    pub page_hashing: Option<u32>,
    pub epub_divina_letter_count_threshold: Option<usize>,
}

/// Only consulted when database migrations run (fresh or upgraded data directory).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileLibraries {
    pub file_hashing: Option<bool>,
    pub scan_on_startup: Option<bool>,
    pub delete_empty_collections: Option<bool>,
    pub delete_empty_read_lists: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileKobo {
    pub sync_item_limit: Option<u32>,
    pub kepubify_path: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileOAuth2 {
    pub account_creation: Option<bool>,
    pub oidc_email_verification: Option<bool>,
    pub registrations: Option<HashMap<String, FileOAuth2Registration>>,
}

/// One table per provider; setting `issuer-uri` switches it to OIDC discovery mode.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileOAuth2Registration {
    pub client_name: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub authorization_grant_type: Option<String>,
    pub redirect_uri: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub issuer_uri: Option<String>,
    pub authorization_uri: Option<String>,
    pub token_uri: Option<String>,
    pub user_info_uri: Option<String>,
    pub user_name_attribute: Option<String>,
}

/// Duration accepting Spring-style strings ("500ms", "10s", "30m", "1h", "7d");
/// a bare TOML integer means seconds.
#[derive(Debug, Clone, Copy)]
pub struct ConfigDuration(pub Duration);

impl<'de> Deserialize<'de> for ConfigDuration {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = ConfigDuration;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a duration string like \"7d\"/\"1h\"/\"30m\"/\"10s\"/\"500ms\", or seconds as an integer")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                parse_duration(v).map(ConfigDuration).map_err(E::custom)
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(ConfigDuration(Duration::from_secs(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                u64::try_from(v)
                    .map(|s| ConfigDuration(Duration::from_secs(s)))
                    .map_err(|_| E::custom(format!("negative duration: {v}")))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    // "ms" must be tried before "s"
    for (suffix, millis) in [
        ("ms", 1u64),
        ("s", 1_000),
        ("m", 60_000),
        ("h", 3_600_000),
        ("d", 86_400_000),
    ] {
        if let Some(num) = s.strip_suffix(suffix) {
            let n: u64 = num
                .trim()
                .parse()
                .map_err(|_| format!("invalid duration {s:?}"))?;
            return Ok(Duration::from_millis(n * millis));
        }
    }
    Err(format!(
        "invalid duration {s:?}: expected a number followed by ms/s/m/h/d"
    ))
}

pub fn format_duration(d: Duration) -> String {
    let ms = d.as_millis() as u64;
    for (suffix, unit) in [
        ("d", 86_400_000u64),
        ("h", 3_600_000),
        ("m", 60_000),
        ("s", 1_000),
    ] {
        if ms >= unit && ms.is_multiple_of(unit) {
            return format!("{}{}", ms / unit, suffix);
        }
    }
    format!("{ms}ms")
}

// ---- rendering of the generated config.toml ----

/// Renders the file written on first start: every value is shown explicitly, so the
/// result reflects the built-in defaults merged with the migrated configuration.
pub fn render(config: &ServerConfig, source: Option<&Path>) -> String {
    let mut out = String::new();
    out.push_str("# kmrs configuration file.\n#\n");
    match source {
        Some(p) => out.push_str(&format!(
            "# Generated on first start; values migrated from {}.\n",
            p.display()
        )),
        None => out.push_str("# Generated on first start with built-in defaults.\n"),
    }
    out.push_str(
        "# Delete a key to fall back to its default; delete the file to regenerate it.\n\
         # Precedence (lowest to highest): built-in defaults < this file < env vars < CLI flags.\n\
         # Env vars keep the komga/Spring names (e.g. `database.file` -> KOMGA_DATABASE_FILE),\n\
         # so an existing komga deployment's environment keeps working.\n\n",
    );

    out.push_str("[server]\n");
    out.push_str(&format!(
        "port = {} # env: SERVER_PORT; CLI: --port\n",
        config.port
    ));
    out.push_str(&format!(
        "session-timeout = {} # env: SERVER_SERVLET_SESSION_TIMEOUT; \"500ms\"/\"10s\"/\"30m\"/\"1h\"/\"7d\", bare integer = seconds\n",
        q(&format_duration(config.session_timeout))
    ));
    match &config.server_context_path {
        Some(p) => out.push_str(&format!(
            "context-path = {} # env: SERVER_SERVLET_CONTEXT_PATH; URL prefix, empty = root\n",
            q(p)
        )),
        None => out.push_str(
            "# context-path = \"/\" # env: SERVER_SERVLET_CONTEXT_PATH; URL prefix, empty = root\n",
        ),
    }
    out.push('\n');

    out.push_str("[cors]\n");
    out.push_str(&format!(
        "allowed-origins = {} # env: KOMGA_CORS_ALLOWEDORIGINS (comma-separated)\n\n",
        str_list(&config.cors_allowed_origins)
    ));

    render_database(&mut out, "database", &config.database, "KOMGA_DATABASE");
    render_database(&mut out, "tasks-db", &config.tasks_db, "KOMGA_TASKSDB");

    out.push_str("[search]\n");
    out.push_str(&format!(
        "data-directory = {} # tantivy index; env: KOMGA_LUCENE_DATA_DIRECTORY\n\n",
        q(&config.lucene_dir.display().to_string())
    ));

    out.push_str("[fonts]\n");
    out.push_str(&format!(
        "data-directory = {} # env: KOMGA_FONTS_DATA_DIRECTORY\n\n",
        q(&config.fonts_dir.display().to_string())
    ));

    out.push_str("[books]\n");
    out.push_str(&format!(
        "page-hashing = {} # env: KOMGA_PAGEHASHING\n",
        config.page_hashing
    ));
    out.push_str(&format!(
        "epub-divina-letter-count-threshold = {} # env: KOMGA_EPUBDIVINALETTERCOUNTTHRESHOLD\n\n",
        config.epub_divina_letter_count_threshold
    ));

    out.push_str("[libraries]\n");
    out.push_str(
        "# only consulted when database migrations run (fresh or upgraded data directory)\n",
    );
    out.push_str(&format!(
        "file-hashing = {} # env: KOMGA_FILEHASHING\n",
        config.migration_placeholders.library_file_hashing
    ));
    out.push_str(&format!(
        "scan-on-startup = {} # env: KOMGA_LIBRARIESSCANSTARTUP\n",
        config.migration_placeholders.library_scan_startup
    ));
    out.push_str(&format!(
        "delete-empty-collections = {} # env: KOMGA_DELETEEMPTYCOLLECTIONS\n",
        config.migration_placeholders.delete_empty_collections
    ));
    out.push_str(&format!(
        "delete-empty-read-lists = {} # env: KOMGA_DELETEEMPTYREADLISTS\n\n",
        config.migration_placeholders.delete_empty_read_lists
    ));

    out.push_str("[kobo]\n");
    out.push_str(&format!(
        "sync-item-limit = {} # env: KOMGA_KOBO_SYNCITEMLIMIT\n",
        config.kobo_sync_item_limit
    ));
    match &config.kepubify_path {
        Some(p) => out.push_str(&format!(
            "kepubify-path = {} # env: KOMGA_KOBO_KEPUBIFY_PATH\n\n",
            q(&p.display().to_string())
        )),
        None => out.push_str(
            "# kepubify-path = \"/usr/local/bin/kepubify\" # env: KOMGA_KOBO_KEPUBIFY_PATH\n\n",
        ),
    }

    out.push_str("[oauth2]\n");
    out.push_str(&format!(
        "account-creation = {} # env: KOMGA_OAUTH2ACCOUNTCREATION\n",
        config.oauth2.account_creation
    ));
    out.push_str(&format!(
        "oidc-email-verification = {} # env: KOMGA_OIDCMAILVERIFICATION\n",
        config.oauth2.oidc_email_verification
    ));
    out.push_str(
        "# One table per provider; setting `issuer-uri` switches it to OIDC discovery mode.\n\
         # env vars SPRING_SECURITY_OAUTH2_CLIENT_REGISTRATION_<ID>_*/PROVIDER_<ID>_* override single fields.\n",
    );
    if config.oauth2.registrations.is_empty() {
        out.push_str(
            "#\n\
             # [oauth2.registrations.github]\n\
             # client-id = \"...\"\n\
             # client-secret = \"...\"\n\
             # client-name = \"GitHub\" # defaults to the registration id\n\
             # scopes = [\"read:user\", \"user:email\"]\n\
             # issuer-uri = \"https://accounts.google.com\"\n",
        );
    } else {
        out.push('\n');
        for reg in &config.oauth2.registrations {
            render_registration(&mut out, reg);
        }
    }
    out
}

fn render_database(out: &mut String, section: &str, db: &DatabaseConfig, env_prefix: &str) {
    out.push_str(&format!("[{section}]\n"));
    out.push_str(&format!(
        "file = {} # env: {env_prefix}_FILE\n",
        q(&db.file.display().to_string())
    ));
    match db.pool_size {
        Some(n) => out.push_str(&format!(
            "pool-size = {n} # read pool size; default min(CPU cores, max-pool-size). env: {env_prefix}_POOLSIZE\n"
        )),
        None => out.push_str(&format!(
            "# pool-size = 4 # read pool size; default min(CPU cores, max-pool-size). env: {env_prefix}_POOLSIZE\n"
        )),
    }
    out.push_str(&format!(
        "max-pool-size = {} # env: {env_prefix}_MAXPOOLSIZE\n",
        db.max_pool_size
    ));
    let mode = match db.journal_mode {
        JournalMode::Wal => "WAL",
        JournalMode::Delete => "DELETE",
        JournalMode::Truncate => "TRUNCATE",
        JournalMode::Persist => "PERSIST",
        JournalMode::Memory => "MEMORY",
        JournalMode::Off => "OFF",
    };
    out.push_str(&format!(
        "journal-mode = {} # WAL/DELETE/TRUNCATE/PERSIST/MEMORY/OFF; env: {env_prefix}_JOURNALMODE\n",
        q(mode)
    ));
    match db.busy_timeout {
        Some(d) => out.push_str(&format!(
            "busy-timeout = {} # env: {env_prefix}_BUSYTIMEOUT\n",
            q(&format_duration(d))
        )),
        None => out.push_str(&format!(
            "# busy-timeout = \"30s\" # env: {env_prefix}_BUSYTIMEOUT\n"
        )),
    }
    if db.pragmas.is_empty() {
        out.push_str(&format!(
            "# [{section}.pragmas] # extra SQLite pragmas, TOML only\n# synchronous = \"NORMAL\"\n"
        ));
    } else {
        out.push_str(&format!(
            "[{section}.pragmas] # extra SQLite pragmas, TOML only\n"
        ));
        for (k, v) in &db.pragmas {
            out.push_str(&format!("{k} = {}\n", q(v)));
        }
    }
    out.push('\n');
}

fn render_registration(out: &mut String, reg: &OAuth2ClientRegistration) {
    out.push_str(&format!(
        "[oauth2.registrations.{}]\n",
        toml_key(&reg.registration_id)
    ));
    out.push_str(&format!("client-id = {}\n", q(&reg.client_id)));
    out.push_str(&format!("client-secret = {}\n", q(&reg.client_secret)));
    if let Some(v) = &reg.client_name {
        out.push_str(&format!(
            "client-name = {} # defaults to the registration id\n",
            q(v)
        ));
    }
    out.push_str(&format!(
        "authorization-grant-type = {}\n",
        q(&reg.authorization_grant_type)
    ));
    if let Some(v) = &reg.redirect_uri {
        out.push_str(&format!("redirect-uri = {}\n", q(v)));
    }
    if !reg.scopes.is_empty() {
        out.push_str(&format!("scopes = {}\n", str_list(&reg.scopes)));
    }
    if let Some(v) = &reg.issuer_uri {
        out.push_str(&format!("issuer-uri = {}\n", q(v)));
    }
    if let Some(v) = &reg.authorization_uri {
        out.push_str(&format!("authorization-uri = {}\n", q(v)));
    }
    if let Some(v) = &reg.token_uri {
        out.push_str(&format!("token-uri = {}\n", q(v)));
    }
    if let Some(v) = &reg.user_info_uri {
        out.push_str(&format!("user-info-uri = {}\n", q(v)));
    }
    if let Some(v) = &reg.user_name_attribute {
        out.push_str(&format!("user-name-attribute = {}\n", q(v)));
    }
    out.push('\n');
}

fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn str_list(v: &[String]) -> String {
    format!(
        "[{}]",
        v.iter().map(|s| q(s)).collect::<Vec<_>>().join(", ")
    )
}

/// Bare TOML keys allow only ASCII alphanumerics, `_` and `-`.
fn toml_key(id: &str) -> String {
    if !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        id.to_string()
    } else {
        q(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_parsing() {
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("10s").unwrap(), Duration::from_secs(10));
        assert_eq!(parse_duration("30m").unwrap(), Duration::from_secs(1800));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(
            parse_duration("7d").unwrap(),
            Duration::from_secs(7 * 86400)
        );
        assert!(parse_duration("7").is_err());
        assert!(parse_duration("abc").is_err());
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(1800)), "30m");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
        assert_eq!(format_duration(Duration::from_secs(7 * 86400)), "7d");
        assert_eq!(format_duration(Duration::from_secs(90)), "90s");
        assert_eq!(format_duration(Duration::from_secs(120)), "2m");
    }
}
