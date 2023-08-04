extern crate core;


use std::{env, fs};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::str::FromStr;

use anyhow::Context;
use chrono::{DateTime, NaiveDateTime, Utc};
use clap::{Arg, arg, ArgAction, ArgMatches, Args, command, Command, FromArgMatches, value_parser, ValueEnum};
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use either::Either;
use if_chain::if_chain;
use lazy_static::lazy_static;
use log::info;
use prettytable::{Cell, row, Table};
use serde::{Deserialize, Serialize};
use simplelog::{ColorChoice, CombinedLogger, LevelFilter, TerminalMode, TermLogger, WriteLogger};
use spinners::{Spinner, Spinners};
use thiserror::Error;

use stine_rs::{EventType, LazyLevel, SemesterResult, SemesterType, Stine};
use stine_rs::Semester as SemesterStine;

mod notify;

// reusing the config as env file ( ͠° ͟ʖ ͡°), don't know if good or bad ( ͡ʘ ͜ʖ ͡ʘ)
lazy_static! {
    static ref CONFIG_PATH: PathBuf = env::current_exe().unwrap().parent().unwrap().join(".stine-env");
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Config {
    username: String,
    password: String,
    session: String,
    cnsc_cookie: String,
    /// Last usage of session cookies as unix timestamp
    last_used: Option<i64>,
}


fn load_cfg(config_path: &Path) -> anyhow::Result<Config> {
    if config_path.exists() {
        let config: Config = toml::from_str(&fs::read_to_string(config_path)?)?;

        return Ok(config);
    }
    Ok(Config::default())
}

fn save_cfg(config_path: &Path, cfg: &mut Config) -> anyhow::Result<()> {
    if !config_path.exists() {
        fs::create_dir_all(config_path.parent().unwrap_or_else(|| Path::new("")))?;
    }

    cfg.last_used = Some(Utc::now().timestamp());
    let mut buffer = File::create(config_path)?;
    buffer.write_all(toml::to_string_pretty(&cfg)?.as_bytes())?;
    Ok(())
}

fn unwrap_option_or_generic<T, F>(option: Option<T>, fallback: F) -> Either<T, F> {
    if let Some(value) = option {
        either::Left(value)
    } else {
        either::Right(fallback)
    }
}

/// Loads credentials for authentication
/// If credentials are passed as cli arg, these are prioritized and updated in the returned Config
fn get_credentials(matches: &ArgMatches) -> Config {
    let mut auth_cfg = Config::default();

    let username = matches.get_one::<String>("username");
    let password = matches.get_one::<String>("password");

    if_chain! {
        if let Some(username) = username;
        if let Some(password) = password;
        then {

            // if &auth_cfg.username != username || &auth_cfg.password != password {
            //     println!("Passed credentials {}:{}, differ from config credentials {}:{}",
            //                      username, password,
            //                      auth_cfg.username, auth_cfg.password);
            // }

            if matches.get_flag("save_config") {
                save_cfg(&CONFIG_PATH, &mut auth_cfg)
                .with_context(|| format!("Failed saving config to {}", &CONFIG_PATH.display())).unwrap();
                println!("{} [{}]",
                 "> Saved credentials to config file".bright_green(),
                 &CONFIG_PATH.display().to_string().underline());
            }

            auth_cfg.username = username.to_string();
            auth_cfg.password = password.to_string();

        } else {
            // if no auth arg passed, try to load them from config file

            let use_auth_arg_msg = format!("Can't load credentials from config-file at: {} \
            (Look at the README.md for slightly more info). \
            Please use --username <USERNAME> and --password <PASSWORD> for authentication.",
                &CONFIG_PATH.display());

            let cfg: Config = load_cfg(&CONFIG_PATH)
                .with_context(|| format!("Failed loading config file from: {}", &CONFIG_PATH.display())).unwrap();

            // config is empty
            if cfg.username.is_empty() || cfg.password.is_empty() {
                eprintln!("{}", use_auth_arg_msg.red());
                exit(-1);
            } else {
                println!("Loading username and password from config file: [{}]", &CONFIG_PATH.display());
                auth_cfg = cfg;
            }

        }
    }

    auth_cfg
}

fn check_network_connection() -> bool {
    reqwest::blocking::get("https://google.com").is_ok()
}


fn authenticate(auth_cfg: &Config, check_network: bool) -> Stine {
    let last_used_dt = DateTime::from_utc(
        NaiveDateTime::from_timestamp_opt(
            auth_cfg.last_used.unwrap_or(Utc::now().timestamp()), 0,
        )
            .unwrap_or(Utc::now().naive_utc()), Utc,
    );

    let max_timeout = 30;
    let no_timeout = (Utc::now() - last_used_dt).num_minutes() < max_timeout;

    if !auth_cfg.session.is_empty() && !auth_cfg.cnsc_cookie.is_empty()
        && no_timeout {
        println!("> Authenticating using session cookies");
        if let Ok(stine_session) = Stine::new_session(&auth_cfg.cnsc_cookie, &auth_cfg.session) {
            return stine_session;
        } else {
            println!("{}", "Failed authenticating using session cookies.".red());
            println!("> Using credentials");
        }
    }

    match Stine::new(auth_cfg.username.as_str(), auth_cfg.password.as_str()) {
        Ok(stine) => stine,
        Err(error) => {
            if check_network && !check_network_connection() {
                panic!("{}", "Can't reach network. Is your internet working? O-o".bright_red());
            } else {
                eprintln!("{}. Error: {}",
                          "Failed authenticating with Stine".bright_red(),
                          error);
                exit(-1);
            }
        }
    }
}

#[derive(ValueEnum, Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
enum Language {
    German,
    English,
}

impl From<Language> for stine_rs::Language {
    fn from(lang: Language) -> Self {
        match lang {
            Language::German => stine_rs::Language::German,
            Language::English => stine_rs::Language::English,
        }
    }
}

impl From<stine_rs::Language> for Language {
    fn from(lang: stine_rs::Language) -> Self {
        match lang {
            stine_rs::Language::German => Language::German,
            stine_rs::Language::English => Language::English,
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Semester {
    pub season: SemesterType,
    pub year: Either<i32, (i32, i32)>,
}

impl From<SemesterStine> for Semester {
    fn from(semester: SemesterStine) -> Self {
        Semester {
            season: semester.season,
            year: semester.year,
        }
    }
}

impl From<Semester> for SemesterStine {
    fn from(semester: Semester) -> Self {
        SemesterStine {
            season: semester.season,
            year: semester.year,
        }
    }
}

impl clap::builder::ValueParserFactory for Semester {
    type Parser = SemesterValueParser;

    fn value_parser() -> Self::Parser {
        SemesterValueParser
    }
}

#[derive(Clone, Debug)]
pub struct SemesterValueParser;

impl clap::builder::TypedValueParser for SemesterValueParser {
    type Value = Semester;

    fn parse_ref(
        &self, _cmd: &Command, _arg: Option<&Arg>, value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let semester_result: Result<SemesterStine, SemesterParseError> = convert_error(
            SemesterStine::from_str(value.to_str().unwrap()));
        Ok(Semester::from(semester_result?))
    }
}

fn convert_error(input: Result<SemesterStine, anyhow::Error>)
                 -> Result<SemesterStine, SemesterParseError> {
    Ok(input?)
}

#[derive(Error, Debug)]
enum SemesterParseError {
    #[error("Failed parsing Semester. Expected semester string in format: \
    [(wi)|(su)se \\d\\d(/\\d\\d)?] (case-insensitive). \
     e.g.: SuSe22, \"wise 21/22\". Error: {0}")]
    ParseError(#[from] anyhow::Error)
}

impl From<SemesterParseError> for clap::Error {
    fn from(error: SemesterParseError) -> Self {
        clap::Error::raw(clap::error::ErrorKind::ValueValidation, error)
    }
}

// https://docs.rs/clap/latest/clap/_derive/index.html#mixing-builder-and-derive-apis
#[derive(Args, Debug)]
struct DerivedArgs {
    #[command(flatten)]
    verbose: Verbosity,
}

fn get_command() -> Command {
    let command = command!().disable_colored_help(false)
        .arg(
            arg!(--username <USERNAME> "Username for the stine login. Alternatively use .env file")
                .required(false)
                .value_parser(value_parser!(String))
        )
        .arg(
            arg!(--password <PASSWORD> "Password for the stine login. Alternatively use .env file")
                .required(false)
                .value_parser(value_parser!(String))
        )
        .arg(
            arg!(--save_config "Save username and password to .env file")
                .required(false)
                .action(ArgAction::SetTrue)
        )
        .arg(arg!(-l --language <LANGUAGE>)
            .required(false)
            .value_parser(value_parser!(Language))
            .help("Set Stine language. Changes output and language for your account")
        )
        .arg_required_else_help(true)
        .subcommand_required(true)
        .subcommands(
            [
                Command::new("semester-results")
                    .about("Print exam results of semesters")
                    .arg(
                        arg!(-s --semesters <SEMESTERS>)
                            .required(false)
                            .num_args(0..)
                            .value_parser(value_parser!(Semester))
                    )
                    .arg(
                        Arg::new("grade-avg").long("grade-avg")
                            .required(false)
                            .action(ArgAction::SetTrue)
                            .help("Show grade avg of the course. Potentially doubles requests to STINE.")
                    ),
                // TODO: impl courses subcommand
                // Command::new("courses")
                //     .about("Print all available courses")
                //     .arg(Arg::new("force-refresh").short('f').long("force-refresh")),
                Command::new("registration-status")
                    .about("Print registration status of all applied (sub)-modules")
                    .arg(arg!(-r --reduce).required(false).action(ArgAction::SetTrue)
                        .help("Reduce requests made to STINE. \
                        Removes colorized output and may result in slightly wrong lecture names")),
                Command::new("notify")
                    .about("Send email about various events")
                    .arg(arg!(-e --events <EVENTS>)
                        .required(false)
                        .num_args(0..)
                        .value_parser(value_parser!(notify::NotifyEvent))
                        .help("Specify events to be notified about. If not specified, you will be notified about all events"))
                    .arg(arg!(--email_address <EMAIL_ADDRESS>)
                        .required(true)
                        .value_parser(value_parser!(String))
                        .help("Email-address used for the notifications"))
                    .arg(arg!(--email_password <EMAIL_PASSWORD>)
                        .required(true)
                        .value_parser(value_parser!(String))
                        .help("Password for the email address"))
                    .arg(arg!(--smtp_server <SMTP_SERVER>)
                        .required(false)
                        .value_parser(value_parser!(String))
                        .help("SMTP Server Address. E.g.: smtp.gmail.com. Required if can't be determined"))
                    .arg(arg!(--smtp_port <SMTP_PORT>)
                        .required(false)
                        .value_parser(value_parser!(u16))
                        .help("SMTP Server PORT. E.g.: 587. Required if can't be determined"))
                    .arg(arg!(-l --language <LANGUAGE>)
                        .required(false)
                        .value_parser(value_parser!(Language))
                        .help("STINE Language setting. Necessary for data comparison. \
                        If not specified, default of your stine account(if nothing saved) or the language saved for data comparison is used. \
                        Errors if either default or specified language is different from the saved language of the data"))
                    .arg(Arg::new("force_language").long("force-language")
                        .required(false)
                        .action(ArgAction::SetTrue)
                        .help("Overwrites the saved language, \
                        will delete old data and replace it with new data in the specified <LANGUAGE> using --language"))
                    .arg(Arg::new("dry").long("dry-run")
                        .required(false)
                        .action(ArgAction::SetTrue).help("Only output to stdout."))
                    .arg(Arg::new("send-test-email").long("send-test-email")
                        .required(false)
                        .action(ArgAction::SetTrue).help("Send a test Email to see if your email credentials work.")),
                Command::new("check")
                    .about("Check your credentials and connection to Stine")
            ],
        );

    DerivedArgs::augment_args(command)
}

fn colorize_event_type(str: String, event_type: Option<EventType>) -> colored::ColoredString {
    match event_type {
        Some(EventType::Lecture) => str.blue(),
        Some(EventType::Exercise) => str.green(),
        Some(EventType::Project) => str.red(),
        Some(EventType::Internship) => str.red(),
        Some(EventType::Seminar) => str.magenta(),
        Some(EventType::Proseminar) | Some(EventType::GPSCourse) => str.magenta(),
        Some(EventType::Tutorial) => str.cyan(),
        _ => str.white(),
    }
}

fn main() {
    let matches = get_command().get_matches();

    let derived_matches = DerivedArgs::from_arg_matches(&matches)
        .map_err(|err| err.exit())
        .unwrap();
    let log_level: LevelFilter = derived_matches.verbose.log_level_filter();

    let log_config = simplelog::ConfigBuilder::new()
        .add_filter_allow_str("stine")
        .build();

    let log_path = dirs::home_dir().unwrap().join("stine-cli.log");

    let log_file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(&log_path)
        .with_context(|| format!("Failed writing to log file: {}", log_path.display())).unwrap();


    CombinedLogger::init(
        vec![
            TermLogger::new(log_level, log_config.clone(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Trace, log_config, log_file),
        ]
    ).unwrap();

    info!("LogLevel: [{log_level}]");
    info!("Using log file at: [{}]", log_path.display());

    let mut auth_cfg = get_credentials(&matches);

    let mut stine: Stine = authenticate(
        &auth_cfg,
        matches!(matches.subcommand_name(), Some("check")));

    println!("{}", "Successfully authenticated with Stine".bold().green());


    match matches.subcommand() {
        Some(("semester-results", sub_matches)) => {
            let grade_avg = sub_matches.get_flag("grade-avg");
            let semesters: Vec<Semester> = sub_matches.get_many("semesters")
                .unwrap_or_default().cloned().collect();
            let semesters: Vec<SemesterStine> = semesters.iter().cloned().map(SemesterStine::from).collect();

            let mut spinner = Spinner::new(Spinners::Dots,
                                           "Fetching semester results".into());

            // fetch semester results using NotLazy to directly use `GradeStats`
            let lazy_level = if grade_avg { LazyLevel::NotLazy } else { LazyLevel::FullLazy };
            let semester_results: Vec<SemesterResult> = if semesters.is_empty() {
                stine.get_all_semester_results(lazy_level)
                    .unwrap_or_else(|_| { panic!("{}", "Request Error while trying to fetch all semester results".bright_red()) })
            } else {
                println!("Selected Semesters: {semesters:?}");
                stine.get_semester_results(semesters, lazy_level)
                    .unwrap_or_else(|_| { panic!("{}", "Request Error while trying to fetch semester results".bright_red()) })
            };
            spinner.stop();

            let mut table = Table::new();
            let mut header_row = row!["ID", "Name", "Final grade", "Credits", "Status"];
            if grade_avg {
                header_row.add_cell(Cell::new("Grade Avg"))
            }
            table.add_row(header_row);
            for semester_result in semester_results {
                for mut course_result in semester_result.courses {
                    let mut row = row![
                            course_result.number,
                            course_result.name,
                            unwrap_option_or_generic(course_result.final_grade, "-"),
                            course_result.credits.as_ref().unwrap_or(&"-".to_string()),
                            course_result.status,
                    ];
                    if grade_avg {
                        let avg_formatted = course_result.get_grade_stats(&stine).map_or_else(
                            || "_".to_string(), |g| g.average.unwrap_or_default().to_string());
                        row.add_cell(Cell::new(&avg_formatted));
                    }
                    table.add_row(row);
                }


                table.add_row(
                    row![
                        format!("Semester [{}]", semester_result.semester.to_string().red()),
                        "",
                        semester_result.semester_gpa.unwrap_or_default().to_string().green().bold(),
                        semester_result.semester_credits.to_string().green().bold(),
                        ""
                    ]
                );

                table.add_empty_row();
            }

            println!();
            table.printstd();
        }
        Some(("registration-status", submatches)) => {
            let mut spinner = Spinner::new(Spinners::Dots,
                                           "Fetching registration status".into());

            let registrations = stine.get_my_registrations(LazyLevel::FullLazy).
                context("Failed fetching stine registrations").unwrap();
            spinner.stop();

            let mut table_pending = Table::new();
            table_pending.add_row(row![c => "--- pending ---".bold()]);
            for mut pending_submodule in registrations.pending_submodules {

                // colorizing requires an extra request for every submodule
                // so only do this if reducing of request is not wanted
                let name = if !submatches.get_flag("reduce") {
                    colorize_event_type(
                        pending_submodule.name.to_string(),
                        pending_submodule.info(&stine).event_type)
                } else {
                    pending_submodule.name.to_string().white()
                };

                table_pending.add_row(row![
                        name
                    ]);
            }

            let mut table_accepted = Table::new();
            table_accepted.add_row(row![c => "--- accepted ---".green().bold()]);
            for accepted_submodule in registrations.accepted_submodules {
                table_accepted.add_row(row![accepted_submodule.name]);
            }

            let mut table_rejected = Table::new();
            table_rejected.add_row(row![c => "--- Rejected ---".red().bold()]);
            for rejected_submodule in registrations.rejected_submodules {
                table_rejected.add_row(row![rejected_submodule.name]);
            }

            let mut table_accepted_modules = Table::new();
            table_accepted_modules.add_row(row![c => "--- accepted modules ---".green().bold()]);
            for module in registrations.accepted_modules {
                table_accepted_modules.add_row(row![module.name]);
            }

            println!();
            table_pending.printstd();
            println!();
            table_accepted.printstd();
            println!();
            table_rejected.printstd();
            println!();
            table_accepted_modules.printstd();
        }
        Some(("notify", sub_matches)) => {
            notify::notify_command(sub_matches, &mut stine);
        }
        Some(("check", _)) => {
            println!("{} {}",
                     stine_rs::BASE_URL.underline(),
                     "is available and your credentials work 😃".bright_green())
        }

        _ => unimplemented!(),
    }


    auth_cfg.session = stine.session.unwrap();
    auth_cfg.cnsc_cookie = stine.cnsc_cookie.unwrap();


    if matches.get_flag("save_config") {
        save_cfg(&CONFIG_PATH, &mut auth_cfg)
            .with_context(|| format!("Failed saving config to {}", &CONFIG_PATH.display())).unwrap();
        println!("{} [{}]",
                 "> Saved credentials and session to config file".bright_green(),
                 &CONFIG_PATH.display().to_string().underline());
    }
}


#[test]
fn verify_cmd() {
    get_command().debug_assert();
}