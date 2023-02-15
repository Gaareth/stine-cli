extern crate core;

mod notify;

use std::error::Error;
use std::{fs};

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::exit;
use std::str::FromStr;


use stine_rs::{Stine, SemesterResult, SemesterType, LazyLevel, EventType};
use stine_rs::Semester as SemesterStine;

use serde::{Deserialize, Serialize};
use clap::{Arg, arg, ArgAction, ArgMatches, Args, command, Command, value_parser, ValueEnum, FromArgMatches};
use clap_verbosity_flag::Verbosity;
use prettytable::{row, Table};
use colored::Colorize;

use either::Either;

use if_chain::if_chain;
use log::{info, trace};

use simplelog::{ColorChoice, CombinedLogger, LevelFilter, WriteLogger, TermLogger, TerminalMode};

// reusing the config as env file ( ͠° ͟ʖ ͡°), don't know if good or bad ( ͡ʘ ͜ʖ ͡ʘ)
const CONFIG_PATH: &str = "./.env";

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    username: String,
    password: String,
    session: String,
    cnsc_cookie: String,
}

/// `Config` implements `Default`
impl Default for Config {
    fn default() -> Self { Self { username: "".to_string(), password: "".to_string(),
        session: "".to_string(), cnsc_cookie: "".to_string() } }
}

fn load_cfg(config_path: &Path) -> Result<Config, Box<dyn Error>> {
    if config_path.exists() {
        let config: Config = toml::from_str(&fs::read_to_string(config_path)?)?;

        Ok(config)
    } else {
        fs::create_dir_all(config_path.parent().unwrap_or_else(|| Path::new("")))?;

        let mut buffer = File::create(config_path)?;
        buffer.write_all(toml::to_string_pretty(&Config::default())?.as_bytes())?;
        Ok(Config::default())
    }
}

fn save_cfg(config_path: &Path, cfg: &Config) -> Result<(), Box<dyn Error>> {
    if !config_path.exists() {
        fs::create_dir_all(config_path.parent().unwrap_or_else(|| Path::new("")))?;
    }

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
                save_cfg(Path::new(CONFIG_PATH), &auth_cfg).expect("Failed saving config");
                println!("{} [{}]",
                 "> Saved credentials to config file".bright_green(),
                 fs::canonicalize(CONFIG_PATH).unwrap().to_str().unwrap().underline());
            }

            auth_cfg.username = username.to_string();
            auth_cfg.password = password.to_string();

        } else {
            // if no auth arg passed, try to load them from config file

            let use_auth_arg_msg = "Please use --username <USERNAME> and --password <PASSWORD> for authentication.";

            if matches.get_flag("no_config") {
                // no config flag provided, but also no username or password arg provided
                eprintln!("{}", use_auth_arg_msg.red());
                exit(-1);
            } else {
                let cfg: Config = load_cfg(Path::new(CONFIG_PATH))
                    .unwrap_or_else(|_| panic!("Failed loading config file from: {CONFIG_PATH}"));

                // config is empty
                if cfg.username.is_empty() || cfg.password.is_empty() {
                    eprintln!("{}", use_auth_arg_msg.red());
                    exit(-1);
                } else {
                    println!("{} [{}]", "Loading username and password from config file:",
                        fs::canonicalize(CONFIG_PATH).unwrap().to_str().unwrap().underline());
                    auth_cfg = cfg;
                }
            }
        }
    }

    auth_cfg
}

fn check_network_connection() -> bool {
    reqwest::blocking::get("https://google.com").is_ok()
}


fn authenticate(auth_cfg: &Config, check_network: bool) -> Stine {
    if !auth_cfg.session.is_empty() && !auth_cfg.cnsc_cookie.is_empty() {
        println!("> Authenticating using session cookies");
        if let Ok(stine_session) = Stine::new_session(&auth_cfg.cnsc_cookie, &auth_cfg.session) {
            return stine_session
        } else {
            println!("{}", "Failed authenticating using session cookies.".red());
            println!("> Using credentials");
        }
    }

    match Stine::new(auth_cfg.username.as_str(), auth_cfg.password.as_str()) {
        Ok(stine) => stine,
        Err(error) => {
            if check_network && !check_network_connection() {
                panic!("{}", "Can't reach Network".bright_red());
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
    English
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
           year: semester.year
       }
    }
}

impl From<Semester> for SemesterStine {
    fn from(semester: Semester) -> Self {
        SemesterStine {
            season: semester.season,
            year: semester.year
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
        &self, _cmd: &clap::Command, _arg: Option<&clap::Arg>, value: &std::ffi::OsStr,
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

use thiserror::Error;

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
        .arg(
            arg!(--no_config "Don't use config file for authentication.")
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

                Command::new("courses")
                    .about("Print all available courses")
                    .arg(Arg::new("force-refresh").short('f').long("force-refresh")),

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
                        .help("SMTP Server Address. E.g.: smtp.gmail.com. Necessary if otherwise can't be determined"))
                    .arg(arg!(--smtp_port <SMTP_PORT>)
                        .required(false)
                        .value_parser(value_parser!(u16))
                        .help("SMTP Server PORT. E.g.: 587. Necessary if otherwise can't be determined"))
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
                        .action(ArgAction::SetTrue).help("Only output to stdout.")),

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
        Some(EventType::Proseminar) | Some(EventType::GPSCourse)  => str.magenta(),
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

    CombinedLogger::init(
        vec![
            TermLogger::new(log_level, log_config.clone(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Trace, log_config, File::create("stine-cli.log").unwrap()),
        ]
    ).unwrap();

    info!("LogLevel: [{log_level}]");

    let mut auth_cfg = get_credentials(&matches);

    let mut stine: Stine = authenticate(
        &auth_cfg,
        matches!(matches.subcommand_name(), Some("check")));

    println!("{}", "Successfully authenticated with Stine".bold().green());


    match matches.subcommand() {
        Some(("semester-results", sub_matches)) => {

            let semesters: Vec<Semester> = sub_matches.get_many("semesters")
                .unwrap_or_default().cloned().collect();
            let semesters: Vec<SemesterStine> = semesters.iter().cloned().map(SemesterStine::from).collect();

            // fetch semester results using NotLazy to directly use `GradeStats`
            let semester_results: Vec<SemesterResult> = if semesters.is_empty() {
                stine.get_all_semester_results(LazyLevel::NotLazy)
                    .unwrap_or_else(|_| { panic!("{}", "Request Error while trying to fetch all semester results".bright_red()) })
            } else {
                println!("Selected Semesters: {semesters:#?}");
                stine.get_semester_results(semesters, LazyLevel::NotLazy)
                    .unwrap_or_else(|_| { panic!("{}", "Request Error while trying to fetch semester results".bright_red()) })
            };

            let mut table = Table::new();
            table.add_row(row!["ID", "Name", "Final grade", "Credits", "Status", "Grade Average"]);
            for semester_result in semester_results {
                for mut course_result in semester_result.courses {
                    table.add_row(row![
                            course_result.number,
                            course_result.name,
                            unwrap_option_or_generic(course_result.final_grade, "-"),
                            course_result.credits.as_ref().unwrap_or(&"-".to_string()),
                            course_result.status,
                            course_result.get_grade_stats(&stine).map_or_else(
                            || "_".to_string(), |g| g.average.unwrap_or_default().to_string()),
                    ]);
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

            table.printstd();
        },
        Some(("registration-status", submatches)) => {
            if let Ok(registrations) = stine.get_my_registrations(LazyLevel::FullLazy) {
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

                table_pending.printstd();  println!();
                table_accepted.printstd(); println!();
                table_rejected.printstd(); println!();
                table_accepted_modules.printstd();
            }
        },
        Some(("notify", sub_matches)) => {
            notify::notify_command(sub_matches, &mut stine);
        },
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
        save_cfg(Path::new(CONFIG_PATH), &auth_cfg).expect("Failed saving config");
        println!("{} [{}]",
                 "> Saved credentials and session to config file".bright_green(),
                 fs::canonicalize(CONFIG_PATH).unwrap().to_str().unwrap().underline());
    }
}


#[test]
fn verify_cmd() {
    get_command().debug_assert();
}