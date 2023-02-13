use std::collections::HashMap;
use std::fmt::{Debug, Display};

use std::fs;

use std::path::Path;
use std::process::exit;
use std::str::FromStr;
use anyhow::anyhow;
use chrono::Utc;
use clap::{ArgMatches, ValueEnum};
use if_chain::if_chain;
use lazy_static::lazy_static;
use lettre::{Message, SmtpTransport, Transport};
use lettre::transport::smtp::authentication::Credentials;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use stine_rs::{CourseResult, Document, RegistrationPeriod, Stine, SemesterResult};
use crate::Language;


/// Events one can be notified for
#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum NotifyEvent {
    ExamResult,
    RegistrationPeriods,
    Documents
}

#[derive(Error, Debug)]
enum EmailError {
    #[error("Invalid email format")]
    InvalidEmail(String),
    #[error("Missing Smtp settings: host, port")]
    MissingSMTPInfo,
}

lazy_static! {
    static ref SMTP_SETTINGS_MAP: HashMap<String, SmtpSettings> = HashMap::from([
        ("outlook.de".to_owned(), SmtpSettings::new("smtp-mail.outlook.com", 587)),
        ("outlook.com".to_owned(), SmtpSettings::new("smtp-mail.outlook.com", 587)),

        ("gmail.com".to_owned(), SmtpSettings::new("smtp.gmail.com", 587)),
        ("gmx.net".to_owned(), SmtpSettings::new("mail.gmx.net", 587)),

        ("web.de".to_owned(), SmtpSettings::new("smtp.web.de", 587)),
    ]);
}

#[derive(Clone)]
struct SmtpSettings {
    host: String,
    port: u16,
}

impl SmtpSettings {
    pub fn new(host: &str, port: u16) -> Self {
        SmtpSettings {
            host: String::from(host),
            port
        }
    }
}

#[derive(Clone)]
struct EmailAuthConfig {
    email_address: String,
    password: String,
    smtp_settings: SmtpSettings,
}

impl EmailAuthConfig {
    pub fn new(email_address: String, password: String) -> Result<EmailAuthConfig, EmailError> {
        let domain = email_address.split_once('@')
            .ok_or_else(|| EmailError::InvalidEmail("Missing @ in email".to_owned()))?.1;

        let settings = SMTP_SETTINGS_MAP.get(domain)
            .ok_or(EmailError::MissingSMTPInfo)?;

        Ok(EmailAuthConfig {
            email_address,
            password,
            smtp_settings: settings.clone()
        })
    }
}

fn get_email_cfg(sub_matches: &ArgMatches) -> EmailAuthConfig {
    let email_address: &String = sub_matches.get_one("email_address").unwrap();
    let email_password: &String = sub_matches.get_one("email_password").unwrap();
    let smtp_host:Option<&String> = sub_matches.get_one("smtp_server");
    let smtp_port: Option<&u16> = sub_matches.get_one("smtp_port");

    if_chain! {
        if let Some(host) = smtp_host;
        if let Some(port) = smtp_port;

        then {
            EmailAuthConfig {
                email_address: email_address.to_owned(),
                password: email_password.to_owned(),
                smtp_settings: SmtpSettings::new(host, *port),
            }
        } else {
            match EmailAuthConfig::new(email_address.to_owned(), email_password.to_owned()) {
                Ok(cfg) => cfg,
                Err(error) => {eprintln!("Error: {error}"); exit(-1)},
            }
        }
    }
}

pub(crate) fn notify_command(sub_matches: &ArgMatches, stine: &mut Stine) {
    let mut events: Vec<NotifyEvent> = sub_matches.get_many("events").unwrap_or_default().copied().collect();

    if events.is_empty() {
        events = vec![NotifyEvent::ExamResult, NotifyEvent::RegistrationPeriods, NotifyEvent::Documents];
    }

    let language: Option<&Language> = sub_matches.get_one::<Language>("language");
    let overwrite_lang: bool = sub_matches.get_flag("force_language");

    if let Some(lang) = language {
        dbg!(&lang);
        stine.set_language(&stine_rs::Language::from(lang.clone()))
            .expect("Failed changing language");
    }

    let email_cfg = get_email_cfg(sub_matches);

    println!("Events: {events:#?}");
    for event in events {
        match event {
            NotifyEvent::ExamResult =>
                { exam_update(stine, &email_cfg,language, overwrite_lang) }
            NotifyEvent::RegistrationPeriods =>
                { period_update(stine, &email_cfg) }
            NotifyEvent::Documents =>
                { documents_update(stine, &email_cfg) }
        }
    }
}

fn period_update(stine: &Stine, email_cfg: &EmailAuthConfig) {
    let registration_periods: Vec<RegistrationPeriod> = stine.get_registration_periods()
        .expect("Request Error while trying to fetch registration periods");

    let file_name = "send_period_notifications.json";
    let mut send_periods: Vec<RegistrationPeriod> = if Path::new(file_name).exists() {
        read_data(file_name)
    } else {
        Vec::new()
    };

    for reg_period in registration_periods {
        let datetime_now = Utc::now();
        let period = reg_period.period();

        if datetime_now >= period.start && datetime_now <= period.end
            && !send_periods.contains(&reg_period) {

            let body = format!(
                "The {} just started. \
                            \n Further information: {}", reg_period.name(), period.to_string());

            send_email(
                format!("Stine Notifier: The {} just started", reg_period.name()),
                body,
                &email_cfg.clone()
            );

            send_periods.push(reg_period);
        }
    }

    write_data(file_name, send_periods);
}

fn documents_update(stine: &Stine, email_cfg: &EmailAuthConfig) {
    let current_documents: Vec<Document> = stine.get_documents()
        .expect("Request Error while trying to fetch documents");

    let file_name = "documents.json";

    if Path::new(file_name).exists() {
        let old_docs: Vec<Document> = read_data(file_name);

        let diffs: Vec<Document> = get_list_diffs(old_docs, current_documents.clone());
        if !diffs.is_empty() {
            let mut body = String::from("New documents:");
            body.push_str(&diffs.iter().map(|d| d.to_string()).collect::<Vec<String>>().join("\n\n "));

            send_email(String::from("Stine Notifier - Documents update"), body, email_cfg)
        } else {
            println!("[!] No new documents found")
        }
    }
    write_data(file_name, current_documents);
}

fn exam_update(stine: &mut Stine, email_cfg: &EmailAuthConfig,
               arg_lang: Option<&Language>, overwrite_lang: bool) {

    let file_name = "course_results.json";

    // load data first, to check if the saved language differs from the passed one
    let data: Option<DataWrapper<HashMap<String, CourseResult>>> =
        load_data(file_name, arg_lang, overwrite_lang, stine);

    let semester_results: Vec<SemesterResult> = stine.get_all_semester_results()
        .expect("Request Error while trying to fetch all semester results");

    dbg!(stine.get_language().unwrap());

    let latest_map = map_semester_results_by_id(semester_results);

    if Path::new(file_name).exists() {
        let data = data.unwrap();
        let old_map: HashMap<String, CourseResult> = data.data;

        let changes: Vec<(String, Change<String>)> = get_exam_changes(old_map, &latest_map);

        if !changes.is_empty() {
            let mut body = String::from("Update in course results: ");
            for change in changes {
                body.push_str(format!("[{}] ({} -> {}) \n", change.0, change.1.old, change.1.new).as_str());
            }

            send_email("Stine Notifier - Exam update".to_owned(), body, email_cfg);
        } else {
            println!("[!] No new exam updates found")
        }
    }

    let data = DataWrapper {
        language: arg_lang.unwrap_or(
            &Language::from(stine.get_language().expect("Failed fetching STINE language"))
        ).clone(),
        data: latest_map,
    };

    write_data(file_name, data);
}

fn map_semester_results_by_id(semester_results: Vec<SemesterResult>) -> HashMap<String, CourseResult>{
    let mut courses_map: HashMap<String, CourseResult> = HashMap::new();

    for semester_result in semester_results {
        let courses = semester_result.courses;
        for course in courses {
            courses_map.insert(course.clone().number, course);
        }
    }

    courses_map
}

/// Get first different List entries
/// After the first entry matches (is the same) the functions thinks the rest of lists are identical
/// Warning: Assumes that the lists are sorted by the date their entries were added
fn get_list_diffs<T: PartialEq + Clone + Debug>(old_list: Vec<T>, new_list: Vec<T>) -> Vec<T> {
    let mut diffs: Vec<T> = Vec::new();

    for (i, new_element) in new_list.iter().enumerate() {

        if let Some(old_element) = old_list.get(i) {
            if new_element != old_element {
                dbg!(&old_element);
                dbg!(&new_element);

                diffs.push(new_element.clone());
            } else {
                // if the lists are sorted by date
                break
            }
        }
    }
    diffs
}


fn unwrap_or_na<T: Display>(value: Option<T>) -> String {
    if value.is_none() {
        return "N/A".to_string();
    }

    value.unwrap().to_string()
}

struct Change<T> {
    old: T,
    new: T,
}

impl<T> Change<T> {
    pub fn new(old: T, new: T) -> Self {
        Self {
            old,
            new,
        }
    }
}

fn get_exam_changes(old_map: HashMap<String, CourseResult>, new_map: &HashMap<String, CourseResult>)
                    -> Vec<(String, Change<String>)> {

    let mut changes: Vec<(String, Change<String>)> = Vec::new();

    for (course_number, course) in new_map.clone() {
        let name = course.clone().name;

        if let Some(old_course) = old_map.get(&course_number) {

            if old_course.final_grade != course.final_grade {
                changes.push((name.clone(),
                Change::new(
                    unwrap_or_na(old_course.final_grade),
                    unwrap_or_na(course.final_grade))));

                // print_change(&name, unwrap_or_na(old_course.final_grade), unwrap_or_na(course.final_grade));
            }

            if old_course.status != course.status {
                // print_change(&name, &old_course.status, &course.status);
                changes.push((name.clone(),
                              Change::new(old_course.clone().status, course.status)));
            }

        } else if course.final_grade.is_some() &&
            !course.status.is_empty() &&
            course.status != "&nbsp;" {
            // if the exam/course entry is new send an change.
            // But only if the new data is not None or empty.
            // This is to prevent changes like: [Course name] - -> Final Grade: None.
            // Where essentially the exam was added but without relevant info, so you dont want the notification
            changes.push(
                (course.name,
                 Change::new(
                     "-".to_string(),
                     format!("Final Grade: {:?} | Status: {}", course.final_grade, course.status))));
        }
    }

    changes
}


#[derive(Serialize, Deserialize)]
struct DataWrapper<T> {
    language: Language,
    data: T,
}

fn load_data<T: DeserializeOwned>(file_name: &str, passed_lang: Option<&Language>, overwrite_lang: bool, stine: &mut Stine)
    -> Option<DataWrapper<T>> {

    if Path::new(file_name).exists() {
        let data: DataWrapper<T> = read_data(file_name);
        let saved_lang = &data.language;

        if let Some(lang) = passed_lang {
            if lang != saved_lang {
                if !overwrite_lang {
                    panic!(
                        "Passed argument language <{lang:#?}> is different from saved language <{saved_lang:#?}>. \
                    Use --force_language to overwrite the old data");
                }
                else {
                    // Clearing file contents, so there won't be any false difference due to language diffs.
                    fs::write(Path::new(file_name), String::new())
                        .expect("Failed clearing file for language overwrite");
                    //TODO: log the action
                }
            }

        } else if stine.get_language().expect("Failed fetching STINE language") !=
            stine_rs::Language::from(saved_lang.clone()) {

            stine.set_language(&stine_rs::Language::from(saved_lang.clone()))
                .expect("Failed changing Stine language");
            //TODO: weird :/
        }

        Some(data)
    } else {
        None
    }
}

fn read_data<T: DeserializeOwned>(file_path: &str) -> T {
    let path = Path::new("notify").join(file_path);
    let data = fs::read_to_string(path).expect("Failed to read json file");
    serde_json::from_str(data.as_str()).expect("Failed to deserialize json file.")
}

fn write_data<T: Serialize>(file_path: &str, data: T) {
    let path = Path::new("notify").join(file_path);
    let json_string = serde_json::to_string(&data).expect("Failed serializing data to json");
    fs::write(path, json_string)
        .expect("Failed writing to json file :(. Check your permissions.")
}


fn send_email(subject: String, body: String,
             auth: &EmailAuthConfig) {

    let email_address = auth.clone().email_address;
    let password = auth.clone().password;

    let email = Message::builder()
        .from(email_address.parse().unwrap())
        .to(email_address.parse().unwrap())
        .subject(subject.clone())
        .body(body)
        .unwrap();

    let creds = Credentials::new(email_address, password);

    // Open a remote connection to gmail
    let mailer = SmtpTransport::starttls_relay(&auth.smtp_settings.host)
        .unwrap()
        .port(auth.smtp_settings.port)
        .credentials(creds)
        .build();

    // Send the email
    match mailer.send(&email) {
        Ok(_) => println!("Email [{}] sent successfully!", subject),
        Err(e) => panic!("Could not send email: {:?}", e),
    }
}
