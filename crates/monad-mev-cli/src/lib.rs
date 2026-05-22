//! Command-line interface for `monad-mev-rs`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use monad_mev_core::{Address, Error, EventKind, ReplayClock, B256};
use monad_mev_events::{
    decode_basic_defi_log, fixture_root, fixture_stream_items, load_fixture,
    load_workspace_fixture, sdk_metadata, ChainEvent, FixtureDocument, FixtureReport, ReplayConfig,
    ReplayFilter, ReplayRunner,
};
use serde::Serialize;
use serde_json::{json, Value};

/// CLI binary name.
pub const BIN_NAME: &str = "monad-mev";

/// CLI crate version, exposed for diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Successful command exit code.
pub const OK: i32 = 0;

/// Exit code used for runtime failures.
pub const GENERAL_ERROR: i32 = 1;

/// Exit code used for invalid CLI usage.
pub const USAGE_ERROR: i32 = 2;

/// Result of executing a CLI command without directly touching process I/O.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CliOutcome {
    /// Process exit code.
    pub exit_code: i32,
    /// Text to write to stdout.
    pub stdout: String,
    /// Text to write to stderr.
    pub stderr: String,
}

impl CliOutcome {
    fn ok(stdout: impl Into<String>) -> Self {
        Self {
            exit_code: OK,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    fn error(exit_code: i32, stderr: impl Into<String>) -> Self {
        Self {
            exit_code,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }
}

/// Returns the V1 help text.
#[must_use]
pub const fn help_text() -> &'static str {
    "monad-mev 0.0.0\n\nUsage: monad-mev [GLOBAL] <command> [OPTIONS]\n       monad-mev --help\n       monad-mev --version\n\nGlobal options:\n  --json                 Emit structured JSON when the command supports it\n  --no-color             Disable colored output\n  --log-level <level>    Set diagnostic verbosity\n\nCommands:\n  doctor                 Print environment and SDK diagnostics\n  inspect                Summarize a fixture or snapshot-like source\n  decode                 Decode fixture events to JSONL\n  replay                 Replay fixture events with deterministic filters\n  strategy new <path>    Create a compiling strategy scaffold\n"
}

/// Returns the version text.
#[must_use]
pub fn version_text() -> String {
    format!("{BIN_NAME} {VERSION}")
}

/// Runs the CLI with argument strings excluding the binary name.
#[must_use]
pub fn run_cli(args: impl IntoIterator<Item = impl Into<String>>) -> CliOutcome {
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    let wants_json_errors = args.iter().any(|arg| arg == "--json");
    match run_cli_inner(&args) {
        Ok(outcome) => outcome,
        Err(CliError::Usage(message)) => error_outcome(
            wants_json_errors,
            USAGE_ERROR,
            "usage",
            &format!("{message}\ntry `{BIN_NAME} --help`"),
        ),
        Err(CliError::Runtime(message)) => {
            error_outcome(wants_json_errors, GENERAL_ERROR, "runtime", &message)
        }
    }
}

fn run_cli_inner(args: &[String]) -> std::result::Result<CliOutcome, CliError> {
    if args.is_empty() {
        return Ok(CliOutcome::ok(help_text()));
    }
    if matches!(args[0].as_str(), "-h" | "--help" | "help") {
        return Ok(CliOutcome::ok(help_text()));
    }
    if args[0] == "--version" {
        return Ok(CliOutcome::ok(format!("{}\n", version_text())));
    }

    let (global, command_args) = parse_global_options(args)?;
    let Some((command, command_args)) = command_args.split_first() else {
        return Ok(CliOutcome::ok(help_text()));
    };
    if matches!(
        command_args.first().map(String::as_str),
        Some("-h" | "--help")
    ) {
        return command_help(command)
            .ok_or_else(|| CliError::Usage(format!("unknown command `{command}`")));
    }

    match command.as_str() {
        "doctor" => command_doctor(&global, command_args),
        "inspect" => command_inspect(&global, command_args),
        "decode" => command_decode(&global, command_args),
        "replay" => command_replay(&global, command_args),
        "strategy" => command_strategy(command_args),
        "-h" | "--help" => Ok(CliOutcome::ok(help_text())),
        "--version" => Ok(CliOutcome::ok(format!("{}\n", version_text()))),
        other => Err(CliError::Usage(format!("unknown command `{other}`"))),
    }
}

fn command_help(command: &str) -> Option<CliOutcome> {
    let text = match command {
        "doctor" => {
            "Usage: monad-mev [GLOBAL] doctor\n\nPrint host, SDK, fixture, and live-ring diagnostics.\n"
        }
        "inspect" => {
            "Usage: monad-mev [GLOBAL] inspect --fixture <name-or-path>\n\nSummarize a deterministic fixture or snapshot-like fixture source.\n"
        }
        "decode" => {
            "Usage: monad-mev [GLOBAL] decode --fixture <name-or-path> [--defi]\n\nDecode fixture events to JSONL, or DeFi events with --defi.\n"
        }
        "replay" => {
            "Usage: monad-mev [GLOBAL] replay --fixture <name-or-path> [FILTERS] [--report <path>] [--events-jsonl <path>]\n\nRun deterministic fixture replay with optional filters.\n"
        }
        "strategy" => {
            "Usage: monad-mev strategy new <destination>\n\nCreate a compiling strategy scaffold with a fixture-backed test.\n"
        }
        _ => return None,
    };
    Some(CliOutcome::ok(text))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GlobalOptions {
    json: bool,
    no_color: bool,
    log_level: Option<String>,
}

fn parse_global_options(
    args: &[String],
) -> std::result::Result<(GlobalOptions, &[String]), CliError> {
    let mut options = GlobalOptions::default();
    let mut index = 0;

    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--json" => options.json = true,
            "--no-color" => options.no_color = true,
            "--log-level" => {
                index += 1;
                let Some(level) = args.get(index) else {
                    return Err(CliError::Usage("--log-level requires a value".to_owned()));
                };
                options.log_level = Some(level.clone());
            }
            value if value.starts_with("--log-level=") => {
                let level = value.trim_start_matches("--log-level=");
                if level.is_empty() {
                    return Err(CliError::Usage("--log-level requires a value".to_owned()));
                }
                options.log_level = Some(level.to_owned());
            }
            value if value.starts_with('-') => {
                return Err(CliError::Usage(format!("unknown global option `{value}`")));
            }
            _ => break,
        }
        index += 1;
    }

    Ok((options, &args[index..]))
}

fn command_doctor(
    global: &GlobalOptions,
    args: &[String],
) -> std::result::Result<CliOutcome, CliError> {
    reject_extra_args("doctor", args)?;
    let live_supported = cfg!(target_os = "linux");
    let live_reason = if live_supported {
        "live event-ring support is enabled for Linux hosts"
    } else {
        "live event-ring support requires a Linux host in V1"
    };
    let sdk = sdk_metadata();
    let value = json!({
        "command": "doctor",
        "status": "ok",
        "version": VERSION,
        "core_version": monad_mev_core::VERSION,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "fixture_root": fixture_root(),
        "live": {
            "supported": live_supported,
            "reason": live_reason,
        },
        "sdk": sdk,
        "no_color": global.no_color,
        "log_level": global.log_level,
    });

    if global.json {
        return json_outcome(&value);
    }

    Ok(CliOutcome::ok(format!(
        "status: ok\nversion: {VERSION}\ncore: {}\nhost: {} {}\nfixtures: {}\nlive: {} ({live_reason})\nsdk: {} {} {}\n",
        monad_mev_core::VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
        fixture_root().display(),
        if live_supported { "supported" } else { "unavailable" },
        sdk.repository,
        sdk.tag,
        sdk.commit,
    )))
}

fn command_inspect(
    global: &GlobalOptions,
    args: &[String],
) -> std::result::Result<CliOutcome, CliError> {
    let options = FixtureCommandOptions::parse("inspect", args)?;
    let source = load_fixture_input(options.fixture.as_deref())?;
    let items = fixture_stream_items(&source.fixture).map_err(CliError::from)?;
    let run = ReplayRunner::new(ReplayConfig::default())
        .run(items)
        .map_err(CliError::from)?;
    let observed_report = FixtureReport::from(&run.report);
    let kinds = fixture_kind_counts(&source.fixture);

    if global.json {
        return json_outcome(&json!({
            "command": "inspect",
            "source": source.label,
            "name": source.fixture.name,
            "description": source.fixture.description,
            "event_count": source.fixture.events.len(),
            "kinds": kinds,
            "expected_report": source.fixture.expected_report,
            "observed_report": observed_report,
        }));
    }

    let mut stdout = format!(
        "source: {}\nname: {}\ndescription: {}\nevents: {}\ngaps: {}\nlogs: {}\n",
        source.label,
        source.fixture.name,
        source.fixture.description,
        source.fixture.events.len(),
        observed_report.gaps,
        observed_report.logs_seen,
    );
    if !kinds.is_empty() {
        stdout.push_str("kinds:\n");
        for (kind, count) in kinds {
            let _ = writeln!(stdout, "  {kind}: {count}");
        }
    }
    Ok(CliOutcome::ok(stdout))
}

fn command_decode(
    global: &GlobalOptions,
    args: &[String],
) -> std::result::Result<CliOutcome, CliError> {
    let options = DecodeOptions::parse(args)?;
    let source = load_fixture_input(options.fixture.as_deref())?;
    let items = fixture_stream_items(&source.fixture).map_err(CliError::from)?;
    let mut decoded = Vec::new();

    for item in items {
        let monad_mev_core::StreamItem::Event(event) = item else {
            continue;
        };
        if options.defi {
            let ChainEvent::Log(log) = event.payload else {
                continue;
            };
            decoded.push(
                serde_json::to_value(decode_basic_defi_log(log)).map_err(|err| json_error(&err))?,
            );
        } else {
            decoded.push(serde_json::to_value(event).map_err(|err| json_error(&err))?);
        }
    }

    if global.json {
        return json_outcome(&json!({
            "command": "decode",
            "source": source.label,
            "event_count": decoded.len(),
            "events": decoded,
        }));
    }

    jsonl_outcome(decoded)
}

fn command_replay(
    global: &GlobalOptions,
    args: &[String],
) -> std::result::Result<CliOutcome, CliError> {
    let options = ReplayOptions::parse(args)?;
    let source = load_fixture_input(options.fixture.as_deref())?;
    let mut config = ReplayConfig {
        clock: options.clock,
        filter: options.filter,
        collect_events_jsonl: options.events_jsonl.is_some(),
    };
    if global.json {
        config.collect_events_jsonl = options.events_jsonl.is_some();
    }

    let items = fixture_stream_items(&source.fixture).map_err(CliError::from)?;
    let run = ReplayRunner::new(config)
        .run(items)
        .map_err(CliError::from)?;

    if let Some(report_path) = options.report {
        std::fs::write(&report_path, run.json_report().map_err(CliError::from)?).map_err(
            |err| {
                CliError::Runtime(format!(
                    "failed to write report {}: {err}",
                    report_path.display()
                ))
            },
        )?;
    }
    if let Some(events_path) = options.events_jsonl {
        std::fs::write(&events_path, &run.events_jsonl).map_err(|err| {
            CliError::Runtime(format!(
                "failed to write events JSONL {}: {err}",
                events_path.display()
            ))
        })?;
    }

    if global.json {
        return json_outcome(&json!({
            "command": "replay",
            "source": source.label,
            "report": run.report,
        }));
    }

    Ok(CliOutcome::ok(format!("{}\n", run.human_summary())))
}

fn command_strategy(args: &[String]) -> std::result::Result<CliOutcome, CliError> {
    match args {
        [subcommand, destination] if subcommand == "new" => {
            scaffold_strategy_project(destination).map_err(|err| {
                CliError::Runtime(format!("strategy scaffold failed at {destination}: {err}"))
            })?;
            Ok(CliOutcome::ok(format!(
                "created strategy scaffold at {destination}\n"
            )))
        }
        _ => Err(CliError::Usage(
            "usage: monad-mev strategy new <destination>".to_owned(),
        )),
    }
}

fn reject_extra_args(command: &str, args: &[String]) -> std::result::Result<(), CliError> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(CliError::Usage(format!(
            "`{command}` does not accept `{}`",
            args[0]
        )))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct FixtureCommandOptions {
    fixture: Option<String>,
}

impl FixtureCommandOptions {
    fn parse(command: &str, args: &[String]) -> std::result::Result<Self, CliError> {
        let mut options = Self::default();
        let mut index = 0;

        while let Some(arg) = args.get(index) {
            match arg.as_str() {
                "--fixture" => {
                    index += 1;
                    options.fixture = Some(required_arg(args, index, "--fixture")?.to_owned());
                }
                value if value.starts_with("--fixture=") => {
                    options.fixture = Some(value.trim_start_matches("--fixture=").to_owned());
                }
                "--summary" => {}
                value if value.starts_with('-') => {
                    return Err(CliError::Usage(format!(
                        "unknown {command} option `{value}`"
                    )));
                }
                value if options.fixture.is_none() => options.fixture = Some(value.to_owned()),
                value => {
                    return Err(CliError::Usage(format!(
                        "`{command}` received unexpected argument `{value}`"
                    )));
                }
            }
            index += 1;
        }

        if options.fixture.is_none() {
            return Err(CliError::Usage(format!(
                "`{command}` requires --fixture <name-or-path>"
            )));
        }
        Ok(options)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DecodeOptions {
    fixture: Option<String>,
    defi: bool,
}

impl DecodeOptions {
    fn parse(args: &[String]) -> std::result::Result<Self, CliError> {
        let mut options = Self::default();
        let mut index = 0;

        while let Some(arg) = args.get(index) {
            match arg.as_str() {
                "--fixture" => {
                    index += 1;
                    options.fixture = Some(required_arg(args, index, "--fixture")?.to_owned());
                }
                value if value.starts_with("--fixture=") => {
                    options.fixture = Some(value.trim_start_matches("--fixture=").to_owned());
                }
                "--defi" => options.defi = true,
                value if value.starts_with('-') => {
                    return Err(CliError::Usage(format!("unknown decode option `{value}`")));
                }
                value if options.fixture.is_none() => options.fixture = Some(value.to_owned()),
                value => {
                    return Err(CliError::Usage(format!(
                        "`decode` received unexpected argument `{value}`"
                    )));
                }
            }
            index += 1;
        }

        if options.fixture.is_none() {
            return Err(CliError::Usage(
                "`decode` requires --fixture <name-or-path>".to_owned(),
            ));
        }
        Ok(options)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ReplayOptions {
    fixture: Option<String>,
    filter: ReplayFilter,
    clock: ReplayClock,
    report: Option<PathBuf>,
    events_jsonl: Option<PathBuf>,
}

impl ReplayOptions {
    fn parse(args: &[String]) -> std::result::Result<Self, CliError> {
        let mut fixture: Option<String> = None;
        let mut filter = ReplayFilter::default();
        let mut clock = ReplayClock::AsFastAsPossible;
        let mut report = None;
        let mut events_jsonl = None;
        let mut index = 0;

        while let Some(arg) = args.get(index) {
            match arg.as_str() {
                "--fixture" => {
                    index += 1;
                    fixture = Some(required_arg(args, index, "--fixture")?.to_owned());
                }
                value if value.starts_with("--fixture=") => {
                    fixture = Some(value.trim_start_matches("--fixture=").to_owned());
                }
                "--from-seqno" => {
                    index += 1;
                    filter.from_seqno = Some(parse_u64_arg(args, index, "--from-seqno")?);
                }
                "--to-seqno" => {
                    index += 1;
                    filter.to_seqno = Some(parse_u64_arg(args, index, "--to-seqno")?);
                }
                "--from-block" => {
                    index += 1;
                    filter.from_block = Some(parse_u64_arg(args, index, "--from-block")?);
                }
                "--to-block" => {
                    index += 1;
                    filter.to_block = Some(parse_u64_arg(args, index, "--to-block")?);
                }
                "--kind" => {
                    index += 1;
                    filter
                        .event_kinds
                        .push(parse_event_kind(required_arg(args, index, "--kind")?)?);
                }
                "--address" => {
                    index += 1;
                    filter.address = Some(parse_address(required_arg(args, index, "--address")?)?);
                }
                "--topic0" => {
                    index += 1;
                    filter.topic0 = Some(parse_b256(required_arg(args, index, "--topic0")?)?);
                }
                "--txn" => {
                    index += 1;
                    filter.txn_idx = Some(parse_u64_arg(args, index, "--txn")?);
                }
                "--speed" => {
                    index += 1;
                    clock =
                        ReplayClock::parse_speed_multiplier(required_arg(args, index, "--speed")?)
                            .map_err(CliError::from)?;
                }
                "--report" => {
                    index += 1;
                    report = Some(PathBuf::from(required_arg(args, index, "--report")?));
                }
                "--events-jsonl" => {
                    index += 1;
                    events_jsonl =
                        Some(PathBuf::from(required_arg(args, index, "--events-jsonl")?));
                }
                value if value.starts_with('-') => {
                    return Err(CliError::Usage(format!("unknown replay option `{value}`")));
                }
                value if fixture.is_none() => fixture = Some(value.to_owned()),
                value => {
                    return Err(CliError::Usage(format!(
                        "`replay` received unexpected argument `{value}`"
                    )));
                }
            }
            index += 1;
        }

        if fixture.is_none() {
            return Err(CliError::Usage(
                "`replay` requires --fixture <name-or-path>".to_owned(),
            ));
        }

        Ok(Self {
            fixture,
            filter,
            clock,
            report,
            events_jsonl,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FixtureInput {
    label: String,
    fixture: FixtureDocument,
}

fn load_fixture_input(value: Option<&str>) -> std::result::Result<FixtureInput, CliError> {
    let value = value.ok_or_else(|| CliError::Usage("fixture is required".to_owned()))?;
    let path = Path::new(value);

    if path.exists() {
        return Ok(FixtureInput {
            label: value.to_owned(),
            fixture: load_fixture(path).map_err(CliError::from)?,
        });
    }

    let workspace_name = if Path::new(value)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        value.to_owned()
    } else {
        format!("{value}.json")
    };
    Ok(FixtureInput {
        label: workspace_name.clone(),
        fixture: load_workspace_fixture(&workspace_name).map_err(CliError::from)?,
    })
}

fn fixture_kind_counts(fixture: &FixtureDocument) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for event in &fixture.events {
        if let Some(kind) = event.get("kind").and_then(Value::as_str) {
            *counts.entry(kind.to_owned()).or_default() += 1;
        }
    }
    counts
}

fn json_outcome(value: &impl Serialize) -> std::result::Result<CliOutcome, CliError> {
    let json = serde_json::to_string_pretty(value).map_err(|err| json_error(&err))?;
    Ok(CliOutcome::ok(format!("{json}\n")))
}

fn jsonl_outcome(values: Vec<Value>) -> std::result::Result<CliOutcome, CliError> {
    let mut stdout = String::new();
    for value in values {
        let line = serde_json::to_string(&value).map_err(|err| json_error(&err))?;
        stdout.push_str(&line);
        stdout.push('\n');
    }
    Ok(CliOutcome::ok(stdout))
}

fn error_outcome(wants_json: bool, exit_code: i32, kind: &str, message: &str) -> CliOutcome {
    if wants_json {
        let value = json!({
            "status": "error",
            "error": {
                "kind": kind,
                "message": message,
                "exit_code": exit_code,
            }
        });
        let stderr = serde_json::to_string_pretty(&value)
            .unwrap_or_else(|_| format!("{kind} error: {message}"));
        return CliOutcome::error(exit_code, format!("{stderr}\n"));
    }

    CliOutcome::error(exit_code, format!("{message}\n"))
}

fn required_arg<'a>(
    args: &'a [String],
    index: usize,
    option: &str,
) -> std::result::Result<&'a str, CliError> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| CliError::Usage(format!("{option} requires a value")))
}

fn parse_u64_arg(
    args: &[String],
    index: usize,
    option: &str,
) -> std::result::Result<u64, CliError> {
    let value = required_arg(args, index, option)?;
    value
        .parse()
        .map_err(|err| CliError::Usage(format!("{option} expects u64, got `{value}`: {err}")))
}

fn parse_event_kind(value: &str) -> std::result::Result<EventKind, CliError> {
    serde_json::from_value(Value::String(value.to_owned()))
        .map_err(|err| CliError::Usage(format!("invalid event kind `{value}`: {err}")))
}

fn parse_address(value: &str) -> std::result::Result<Address, CliError> {
    value
        .parse()
        .map_err(|err| CliError::Usage(format!("invalid address `{value}`: {err}")))
}

fn parse_b256(value: &str) -> std::result::Result<B256, CliError> {
    value
        .parse()
        .map_err(|err| CliError::Usage(format!("invalid B256 `{value}`: {err}")))
}

fn json_error(error: &serde_json::Error) -> CliError {
    CliError::Runtime(format!("failed to serialize JSON: {error}"))
}

fn core_crate_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../monad-mev-core")
}

/// Creates a minimal strategy project that compiles immediately.
///
/// # Errors
///
/// Returns an error when the destination exists or files cannot be written.
pub fn scaffold_strategy_project(destination: impl AsRef<Path>) -> std::io::Result<()> {
    let destination = destination.as_ref();
    let core_path = core_crate_path();
    std::fs::create_dir(destination)?;
    std::fs::create_dir(destination.join("src"))?;
    std::fs::write(
        destination.join("Cargo.toml"),
        format!(
            r#"[package]
name = "generated-monad-mev-strategy"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
monad-mev-core = {{ path = "{}" }}
serde_json = "1.0"
"#,
            core_path.display()
        ),
    )?;
    std::fs::write(
        destination.join("src/lib.rs"),
        r#"use monad_mev_core::{Action, EventEnvelope, RecordAction, Result, Strategy, StrategyContext};

#[derive(Default)]
pub struct GeneratedStrategy;

impl Strategy<String> for GeneratedStrategy {
    fn on_event(
        &mut self,
        _context: &mut StrategyContext,
        event: &EventEnvelope<String>,
    ) -> Result<Vec<Action>> {
        Ok(vec![Action::Record(RecordAction {
            topic: "generated.event".to_owned(),
            payload: serde_json::json!({ "seqno": event.seqno() }),
        })])
    }
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{
        run_strategy, CommitState, EventKind, EventMeta, EventSourceKind, FlowTags,
        RecordingExecutor, StreamItem, B256,
    };

    use super::*;

    #[test]
    fn generated_strategy_records_fixture_event() {
        let event = EventEnvelope::new(
            "fixture".to_owned(),
            EventMeta {
                seqno: 1,
                record_epoch_nanos: 1,
                event_kind: EventKind::TxnLog,
                source: EventSourceKind::Fixture,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: CommitState::Unknown,
                schema_hash: Some(B256::from([1_u8; 32])),
            },
        );
        let mut strategy = GeneratedStrategy;
        let mut executor = RecordingExecutor::default();
        let mut context = StrategyContext::new("generated");

        run_strategy(
            [StreamItem::Event(event)],
            &mut strategy,
            &mut executor,
            &mut context,
        )
        .expect("strategy should run");
        assert_eq!(executor.receipts().len(), 1);
    }
}
"#,
    )?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CliError {
    Usage(String),
    Runtime(String),
}

impl From<Error> for CliError {
    fn from(error: Error) -> Self {
        Self::Runtime(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_mentions_binary_name() {
        assert!(help_text().contains(BIN_NAME));
        assert!(help_text().contains("doctor"));
    }

    #[test]
    fn version_mentions_binary_name() {
        assert!(version_text().starts_with(BIN_NAME));
    }

    #[test]
    fn doctor_supports_json_output() {
        let outcome = run_cli(["--json", "doctor"]);

        assert_eq!(outcome.exit_code, OK);
        let value: Value = serde_json::from_str(&outcome.stdout).expect("doctor JSON");
        assert_eq!(value["command"], "doctor");
        assert_eq!(value["status"], "ok");
        assert!(value["live"].get("supported").is_some());
    }

    #[test]
    fn every_command_has_help() {
        for command in ["doctor", "inspect", "decode", "replay", "strategy"] {
            let outcome = run_cli([command, "--help"]);

            assert_eq!(outcome.exit_code, OK, "{command} help should succeed");
            assert!(outcome.stdout.contains("Usage:"), "{command} help");
        }
    }

    #[test]
    fn inspect_fixture_summarizes_events() {
        let outcome = run_cli(["inspect", "--fixture", "raw-events"]);

        assert_eq!(outcome.exit_code, OK);
        assert!(outcome.stdout.contains("events: 2"));
        assert!(outcome.stdout.contains("txn_log: 1"));
    }

    #[test]
    fn decode_defi_fixture_outputs_jsonl() {
        let outcome = run_cli(["decode", "--fixture", "defi-decoded", "--defi"]);

        assert_eq!(outcome.exit_code, OK);
        let line: Value = serde_json::from_str(outcome.stdout.lines().next().expect("jsonl line"))
            .expect("decode JSONL");
        assert_eq!(line["type"], "erc20_transfer");
    }

    #[test]
    fn replay_fixture_writes_report_and_events() {
        let destination =
            std::env::temp_dir().join(format!("monad-mev-replay-command-{}", std::process::id()));
        std::fs::create_dir_all(&destination).expect("temp dir");
        let report = destination.join("report.json");
        let events = destination.join("events.jsonl");

        let outcome = run_cli([
            "replay",
            "--fixture",
            "raw-events",
            "--report",
            report.to_str().expect("report path"),
            "--events-jsonl",
            events.to_str().expect("events path"),
        ]);

        assert_eq!(outcome.exit_code, OK);
        assert!(outcome.stdout.contains("events=2"));
        assert!(report.is_file());
        assert!(events.is_file());

        std::fs::remove_dir_all(destination).ok();
    }

    #[test]
    fn invalid_usage_returns_usage_exit_code() {
        let outcome = run_cli(["replay"]);

        assert_eq!(outcome.exit_code, USAGE_ERROR);
        assert!(outcome.stderr.contains("requires --fixture"));
    }

    #[test]
    fn invalid_command_returns_usage_exit_code() {
        let outcome = run_cli(["wat"]);

        assert_eq!(outcome.exit_code, USAGE_ERROR);
        assert!(outcome.stderr.contains("unknown command"));
    }

    #[test]
    fn missing_fixture_returns_runtime_exit_code() {
        let outcome = run_cli(["inspect", "--fixture", "does-not-exist"]);

        assert_eq!(outcome.exit_code, GENERAL_ERROR);
        assert!(outcome.stderr.contains("failed to read fixture"));
    }

    #[test]
    fn json_errors_are_structured() {
        let outcome = run_cli(["--json", "inspect", "--fixture", "does-not-exist"]);

        assert_eq!(outcome.exit_code, GENERAL_ERROR);
        let value: Value = serde_json::from_str(&outcome.stderr).expect("error JSON");
        assert_eq!(value["status"], "error");
        assert_eq!(value["error"]["kind"], "runtime");
    }

    #[test]
    fn strategy_new_scaffold_creates_expected_files() {
        let destination = std::env::temp_dir().join(format!(
            "monad-mev-generated-strategy-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&destination).ok();

        scaffold_strategy_project(&destination).expect("scaffold should be created");

        assert!(destination.join("Cargo.toml").is_file());
        assert!(destination.join("src/lib.rs").is_file());
        assert!(scaffold_strategy_project(&destination).is_err());

        std::fs::remove_dir_all(destination).ok();
    }

    #[test]
    fn fixture_path_still_points_at_workspace_fixtures() {
        assert!(monad_mev_events::fixture_path("raw-events.json").is_file());
    }
}
