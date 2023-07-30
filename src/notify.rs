use std::{env, fs, io};
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::path::Path;

use chrono::Utc;
use clap::{ArgMatches, ValueEnum};
use if_chain::if_chain;
use lazy_static::lazy_static;
use lettre::{Message, SmtpTransport, Transport};
use lettre::message::{Attachment, MultiPart, SinglePart, SinglePartBuilder};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use thiserror::Error;

use stine_rs::{CourseResult, Document, LazyLevel, MyRegistrations, RegistrationPeriod, SemesterResult, Stine};

use crate::Language;

// path for comparison files
const NOTIFY_PATH: &str = "./notify";

/// Events one can be notified for
#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum NotifyEvent {
    ExamResult,
    RegistrationPeriods,
    Documents,
    RegistrationStatus,
}

#[derive(Error, Debug)]
enum EmailError {
    #[error("Invalid email format")]
    InvalidEmail(String),
    #[error("Missing Smtp settings: host, port")]
    MissingSMTPInfo,
}

// Load smtp url and port for common domains
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
            port,
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
            smtp_settings: settings.clone(),
        })
    }
}

/// Loads `EmailAuthConfig` from CLI Matches
fn get_email_cfg(sub_matches: &ArgMatches) -> EmailAuthConfig {
    let email_address: &String = sub_matches.get_one("email_address").unwrap();
    let email_password: &String = sub_matches.get_one("email_password").unwrap();
    let smtp_host: Option<&String> = sub_matches.get_one("smtp_server");
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
                Err(error) => {
                    error!("Failed determining smtp settings: {error}");
                    panic!("{error}")
                },
            }
        }
    }
}

// TODO: rewrite using actions {EmailAction, PrintAction, SystemNotificationAction, ...}
pub(crate) fn notify_command(sub_matches: &ArgMatches, stine: &mut Stine) {
    let mut events: Vec<NotifyEvent> = sub_matches.get_many("events").unwrap_or_default().copied().collect();

    if events.is_empty() {
        events = vec![NotifyEvent::ExamResult,
                      NotifyEvent::RegistrationPeriods,
                      NotifyEvent::Documents,
                      NotifyEvent::RegistrationStatus];
    }

    let language: Option<&Language> = sub_matches.get_one::<Language>("language");
    let overwrite_lang: bool = sub_matches.get_flag("force_language");
    let dry_run: bool = sub_matches.get_flag("dry");

    debug!("Language arg: {language:#?}");
    debug!("overwrite_lang: {overwrite_lang}");

    if let Some(lang) = language {
        stine.set_language(&stine_rs::Language::from(lang.clone()))
            .expect("Failed changing language");
    }

    let email_cfg = get_email_cfg(sub_matches);

    info!("Selected Events: {events:#?}");

    // lots of unwraps in this line O_O
    let files_path = env::current_exe().unwrap().parent().unwrap().join(NOTIFY_PATH);

    let notifications = events.iter().map(|event| {
        match event {
            NotifyEvent::ExamResult =>
                { exam_update(stine, language, overwrite_lang, &files_path, dry_run) }
            NotifyEvent::RegistrationPeriods =>
                { period_update(stine, &files_path, dry_run) }
            NotifyEvent::Documents =>
                { documents_update(stine, &files_path, dry_run) }
            NotifyEvent::RegistrationStatus =>
                { registration_status_update(stine, language, overwrite_lang, &files_path, dry_run) }
        }
    });

    for group in notifications {
        if group.notifications.is_empty() {
            continue;
        }

        if dry_run {
            println!("[!] {}", group.message);

            for n in group.notifications {
                println!("{n}");
            }
        } else {
            send_email(
                format!("Stine Notifier - {}", group.message),
                group.notifications.join("\n"),
                group.attachments,
                &email_cfg);
        }
    }
}

/// checks for new registration periods
fn period_update(stine: &Stine, path: &Path, dry: bool) -> NotificationGroup {
    let registration_periods: Vec<RegistrationPeriod> = stine.get_registration_periods()
        .expect("Request Error while trying to fetch registration periods");

    let file_name_path = path.join("send_period_notifications.json");
    let mut send_periods: HashSet<RegistrationPeriod> = read_data(&file_name_path).unwrap_or_default();

    // let mut notifications: Vec<String> = vec![];
    // for reg_period in registration_periods {
    //     let datetime_now = Utc::now();
    //     let period = reg_period.period();
    //
    //     if datetime_now >= period.start && datetime_now <= period.end
    //         && !send_periods.contains(&reg_period) {
    //
    //         let body = format!(
    //             "The {} just started. \
    //                         \n Further information: {}", reg_period.name(), period.to_string());
    //         //
    //         // send_email(
    //         //     format!("Stine Notifier: The {} just started", reg_period.name()),
    //         //     body,
    //         //     &email_cfg.clone()
    //         // );
    //
    //         notifications.push(body);
    //         send_periods.push(reg_period);
    //     }
    // }
    // send_email(
    //     format!("Stine Notifier: The {} just started", reg_period.name()),
    //     body,
    //     &email_cfg.clone()
    // );

    let new_reg_periods = registration_periods.into_iter().filter(|reg_period| {
        let datetime_now = Utc::now();
        let period = reg_period.period();

        let new = datetime_now >= period.start && datetime_now <= period.end
            && !send_periods.contains(reg_period);
        if new {
            send_periods.insert(*reg_period);
        }
        new
    });

    let notifications = new_reg_periods.map(|reg_period| {
        format!("The {} just started. \
                                        \n Further information: {}", reg_period.name(), reg_period.period().to_string())
    }).collect();

    if !dry {
        write_data(&file_name_path, send_periods);
    }

    NotificationGroup::new("A new registration period just started", notifications, vec![])
}

#[derive(Clone, Serialize, Deserialize)]
struct MyRegistrationsSerialized {
    pub pending_submodules: Vec<String>,
    pub accepted_submodules: Vec<String>,
    pub rejected_submodules: Vec<String>,
    pub accepted_modules: Vec<String>,
}

impl From<MyRegistrations> for MyRegistrationsSerialized {
    fn from(v: MyRegistrations) -> Self {
        Self {
            pending_submodules: to_string_vec(v.pending_submodules),
            accepted_submodules: to_string_vec(v.accepted_submodules),
            rejected_submodules: to_string_vec(v.rejected_submodules),
            accepted_modules: to_string_vec(v.accepted_modules),
        }
    }
}

fn registration_status_update(stine: &mut Stine,
                              arg_lang: Option<&Language>, overwrite_lang: bool,
                              path: &Path,
                              dry: bool) -> NotificationGroup {
    let file_name = "my_registrations.json";

    let mut changes: Vec<(String, Change<String>)> = vec![];

    // probably necessary, but just in case, idk
    if let Some(lang) = arg_lang {
        stine.set_language(&stine_rs::Language::from(lang.clone()))
            .expect("Failed changing language");
    }

    let current = stine.get_my_registrations(LazyLevel::FullLazy).unwrap();
    let current = MyRegistrationsSerialized::from(current);
    let current_cloned = current.clone();

    let old: Option<DataWrapper<MyRegistrationsSerialized>> = load_data(
        path, file_name, arg_lang, overwrite_lang, stine);
    // data exists
    if let Some(old) = old {
        let old = old.data;

        changes.extend(
            calc_format_changes(old.accepted_modules, current.accepted_modules, "Accepted module registrations"));

        changes.extend(
            calc_format_changes(old.accepted_submodules, current.accepted_submodules, "Accepted registrations"));

        changes.extend(
            calc_format_changes(old.pending_submodules, current.pending_submodules, "Pending registrations"));

        changes.extend(
            calc_format_changes(old.rejected_submodules, current.rejected_submodules, "Rejected registrations"));
        trace!("Calculated registration status changes");
        debug!("{:#?}", changes);
    } else {
        let file_path = path.join(file_name);
        warn!("This seems to be the first check for new registrations status updates [{} does not exist]. Therefore you won't receive any notifications.\
        Only subsequent runs will results in changes and notifications.", file_path.display());
    }

    // save to file
    save_dw(&stine, arg_lang, &path, dry, &file_name, current_cloned);

    NotificationGroup::from_changes(Changes::new(changes), "Change in your registrations", vec![])
}

fn save_dw<T: Serialize>(stine: &&mut Stine, arg_lang: Option<&Language>, path: &&Path, dry: bool, file_name: &&str, data: T) {
    if !dry {
        let data = DataWrapper {
            language: arg_lang.unwrap_or(
                &Language::from(stine.get_language().expect("Failed fetching STINE language"))
            ).clone(),
            data,
        };

        write_data(&path.join(file_name), data);
    } else {
        warn!("Dry run: No write to cache file")
    }
}

fn to_string_vec<T: ToString>(vec: Vec<T>) -> Vec<String> {
    vec.iter().map(T::to_string).collect()
}

fn calc_changes<'a, T: Eq + Hash + Clone + 'a>(old: Vec<T>, current: Vec<T>)
                                               -> (Vec<T>, Vec<T>) {
    let mut new = vec![];
    let mut removed = vec![];

    calc_symmetric_diff(old.clone(),
                        current)
        .into_iter().for_each(|m| {
        if old.contains(&m) {
            removed.push(m)
        } else {
            new.push(m)
        }
    });

    (new, removed)
}

fn format_changes<'a, T: Eq + Hash + Clone + ToString + 'a>(new: Vec<T>, removed: Vec<T>, message: &'a str)
                                                            -> impl Iterator<Item=(String, Change<String>)> + 'a {
    let new_i = new.into_iter()
        .map(move |t| (format!("[New] {message}"), Change::new(String::new(), t.to_string())));
    let removed_i = removed.into_iter()
        .map(move |t| (format!("[Removed] {message}"), Change::new(String::new(), t.to_string())));
    new_i.chain(removed_i)
}

fn calc_format_changes<'a, T: Eq + Hash + Clone + ToString + 'a>(old: Vec<T>, current: Vec<T>, message: &'a str)
                                                                 -> impl Iterator<Item=(String, Change<String>)> + 'a {
    let (new, removed) = calc_changes(old, current);
    format_changes(new, removed, message)
}

fn calc_symmetric_diff<T: Eq + Hash + Clone>(old: Vec<T>, new: Vec<T>) -> Vec<T> {
    let old: HashSet<T> = old.into_iter().collect();
    let new: HashSet<T> = new.into_iter().collect();
    return old.symmetric_difference(&new).cloned().collect();
}

/// Checks for new stine documents
/// Tries to download and return the documents as email attachments
fn documents_update(stine: &Stine, path: &Path, dry: bool) -> NotificationGroup {
    trace!("Checking for new documents");
    let current_documents: Vec<Document> = stine.get_documents()
        .expect("Request Error while trying to fetch documents");

    let file_name = "documents.json";
    let file_path = path.join(file_name);

    let mut changes: Vec<(String, Change<String>)> = vec![];
    let mut attachments = vec![];

    if let Ok(old_docs) = read_data::<Vec<Document>>(&file_path) {
        let (new, removed) = calc_changes(old_docs, current_documents.clone());
        for new_doc in new.clone() {
            if let Ok(content) = download_document(stine, &new_doc) {
                let content_type = ContentType::parse("application/pdf").unwrap();
                let attachment = Attachment::new(new_doc.name)
                    .body(content, content_type);

                attachments.push(attachment);
            } else {
                error!("Failed downloading document for [{}]", new_doc.name);
            }
        }
        changes = format_changes(new, removed, "Documents").collect();


        if changes.is_empty() {
            trace!("[!] No new documents found")
        }
    } else {
        warn!("This seems to be the first check for new documents [{} does not exist]. Therefore you won't receive any notifications.\
        Only subsequent runs will results in changes and notifications.", file_path.display());

        // trace!("documents.json at: [{:#?}] not found. No diffs to output", &file_path);
    }

    if !dry {
        trace!("Writing current documents to file");
        write_data(&file_path, current_documents);
    }

    NotificationGroup::from_changes(Changes::new(changes),
                                    "There are new documents in your stine account",
                                    attachments)
}

fn download_document(stine: &Stine, doc: &Document) -> Result<Vec<u8>, reqwest::Error> {
    let response = stine.get(&doc.download).unwrap();
    Ok(response.bytes()?.as_ref().to_vec())
}

struct Changes {
    changes: Vec<(String, Change<String>)>,
}

impl Changes {
    pub fn new(changes: Vec<(String, Change<String>)>) -> Self {
        Self {
            changes
        }
    }
}


#[derive(Debug)]
struct NotificationGroup {
    message: String,
    notifications: Vec<String>,
    attachments: Vec<SinglePart>,
}


impl NotificationGroup {
    pub fn new(message: &str, notifications: Vec<String>, attachments: Vec<SinglePart>) -> Self {
        Self {
            message: message.to_string(),
            notifications,
            attachments,
        }
    }

    pub fn from_changes(changes: Changes, message: &str, attachments: Vec<SinglePart>) -> Self {
        NotificationGroup::new(message, changes.changes.iter().map(|change| {
            format!("[{}] ({} -> {}) \n", change.0, change.1.old, change.1.new)
        }).collect(), attachments)
    }
}

/// checks for new exam updates
fn exam_update(stine: &mut Stine,
               arg_lang: Option<&Language>, overwrite_lang: bool,
               path: &Path, dry: bool)
               -> NotificationGroup {
    let file_name = "course_results.json";

    // load data first, to check if the saved language differs from the passed one
    let data: Option<DataWrapper<HashMap<String, CourseResult>>> =
        load_data(path, file_name, arg_lang, overwrite_lang, stine);

    let semester_results: Vec<SemesterResult> = stine.get_all_semester_results(LazyLevel::FullLazy)
        .expect("Request Error while trying to fetch all semester results");

    let latest_map = map_semester_results_by_id(semester_results);

    let mut changes = vec![];
    let file_path = path.join(file_name);
    if file_path.exists() {
        let data = data.unwrap();
        let old_map: HashMap<String, CourseResult> = data.data;

        changes = get_exam_changes(old_map, &latest_map);
        debug!("Exam changes: {changes:#?}");
        // if !changes.is_empty() {
        //     // let mut body = String::from("Update in course results: ");
        //     // for change in changes {
        //     //     body.push_str(format!("[{}] ({} -> {}) \n", change.0, change.1.old, change.1.new).as_str());
        //     // }
        //
        // } else {
        //     println!("[!] No new exam updates found")
        // }
    } else {
        warn!("This seems to be the first check for new exams [{} does not exist]. Therefore you won't receive any notifications.\
        Only subsequent runs will results in changes and notifications.", file_path.display())
    }

    save_dw(&stine, arg_lang, &path, dry, &file_name, latest_map);

    NotificationGroup::from_changes(Changes::new(changes), "Update in course results", vec![])
}

/// converts `SemesterResult` list to Map of `CourseResults` where
///     - key: CourseNumber
///     - value: `CourseResult`
fn map_semester_results_by_id(semester_results: Vec<SemesterResult>) -> HashMap<String, CourseResult> {
    let mut courses_map: HashMap<String, CourseResult> = HashMap::new();

    for semester_result in semester_results {
        let courses = semester_result.courses;
        for course in courses {
            courses_map.insert(course.clone().number, course);
        }
    }

    courses_map
}

fn unwrap_or_na<T: Display>(value: Option<T>) -> String {
    if value.is_none() {
        return "N/A".to_string();
    }

    value.unwrap().to_string()
}

#[derive(Debug)]
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
            // compare to old entry

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

/// Abstract Wrapper for Data with an associated language
#[derive(Serialize, Deserialize)]
struct DataWrapper<T> {
    language: Language,
    data: T,
}

/// Loads data from file and checks for language inconsistencies.
/// If the passed language is different from the language of the saved data, the --force_language
/// has to be provided
/// # Returns
///     - None if data cant be read from file
fn load_data<T: DeserializeOwned>(path: &Path, file_name: &str,
                                  passed_lang: Option<&Language>, overwrite_lang: bool,
                                  stine: &mut Stine)
                                  -> Option<DataWrapper<T>> {
    let file_path = path.join(file_name);
    let data: DataWrapper<T> = read_data(&file_path).ok()?;
    let saved_lang = &data.language;
    let saved_stine_lang = stine_rs::Language::from(saved_lang.clone());

    debug!("Saved language: {saved_lang:#?}");

    if let Some(lang) = passed_lang {
        if lang != saved_lang {
            if !overwrite_lang {
                error!(
                    "Passed argument language <{lang:#?}> is different from saved language <{saved_lang:#?}>. \
                    Use --force-language to overwrite the old data");
                panic!();
            } else {
                warn!("Clearing old data(deleting it), \
                because of --force-language and difference of saved an passed language");
                // Clearing file contents, so there won't be any false difference due to language diffs.
                fs::remove_file(file_path.clone())
                    .unwrap_or_else(|_| panic!("Failed deleting old comparison file {file_path:#?}"));
                // return None, so the wrongly loaded data won't be used
                return None;
            }
        }
    } else if stine.get_language().expect("Failed fetching STINE language") != saved_stine_lang {
        // no language passed
        // set stine language to the language saved next to the data, (only if necessary)
        warn!("Changing STINE language: to {saved_stine_lang:#?}");
        stine.set_language(&saved_stine_lang).expect("Failed changing Stine language");
    }

    Some(data)
}

/// # Panics
/// panics when:
/// - filepath hash no parent
/// - JSON file at file_path can't be deserialized
fn read_data<T: DeserializeOwned>(file_path: &Path) -> io::Result<T> {
    fs::create_dir_all(file_path.parent().unwrap())?;
    let data = fs::read_to_string(file_path)?;
    Ok(serde_json::from_str(data.as_str())
        .unwrap_or_else(|_|
            panic!("Failed to deserialize json file. Consider fixing or deleting {file_path:#?}")))
}

/// # Panics
/// panics when:
/// - filepath hash no parent
/// - can't create directory
/// - can't write to file at file_path
/// - data can't be serialized
fn write_data<T: Serialize>(file_path: &Path, data: T) {
    fs::create_dir_all(file_path.parent().unwrap()).expect("Failed creating directory");

    let json_string = serde_json::to_string(&data).expect("Failed serializing data to json");
    fs::write(file_path, json_string)
        .unwrap_or_else(|_| panic!("Failed writing to json file {file_path:#?}. Check your permissions."))
}


fn send_email(subject: String, body: String, attachments: Vec<SinglePart>,
              auth: &EmailAuthConfig) {
    let email_address = auth.clone().email_address;
    let password = auth.clone().password;

    let email = build_email(&subject, body, &email_address, attachments);

    let creds = Credentials::new(email_address, password);

    // Open a remote connection to gmail
    let mailer = SmtpTransport::starttls_relay(&auth.smtp_settings.host)
        .unwrap()
        .port(auth.smtp_settings.port)
        .credentials(creds)
        .build();

    // Send the email
    // TODO: log panics
    match mailer.send(&email) {
        Ok(_) => println!("Email [{subject}] sent successfully!"),
        Err(e) => panic!("Could not send email: {e:?}"),
    }
}

fn build_email(subject: &str, body: String, email_address: &str, attachments: Vec<SinglePart>) -> Message {
    let body = SinglePartBuilder::new().header(ContentType::TEXT_PLAIN).body(body);
    let mut multipart = MultiPart::mixed()
        .singlepart(body);

    for attachment in attachments {
        multipart = multipart.singlepart(attachment);
    }

    Message::builder()
        .from(email_address.parse().unwrap())
        .to(email_address.parse().unwrap())
        .subject(subject)
        .multipart(multipart)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use std::{assert_eq, dbg, env};
    use std::path::PathBuf;

    use lazy_static::lazy_static;
    use lettre::message::Attachment;
    use lettre::message::header::ContentType;

    use stine_rs::{Document, RegistrationPeriod, Stine};

    use crate::notify::{build_email, documents_update, download_document, period_update, read_data, write_data};

    fn auth() -> Stine {
        dotenv::dotenv().ok();

        Stine::new(&env::var("username").unwrap(), &env::var("password").unwrap())
            .expect("Failed authenticating with Stine")
    }

    lazy_static! {
        static ref STINE: Stine = auth();
        static ref TEST_PATH: PathBuf = std::env::temp_dir().join("stine-cli-notify-test/");
    }

    // lazy_static! {
    //     static ref STINE: Mutex<Stine> = Mutex::new(auth());
    // }

    #[test]
    fn test_read_write_data() {
        let file_name = TEST_PATH.join("test.file");
        dbg!(&file_name);
        write_data(&file_name, "test".to_string());
        let data: String = read_data(&file_name).unwrap();
        assert_eq!("test", data);
    }

    // TODO: impl this
    // #[test]
    // fn test_exam_change() {
    //     write_data(&TEST_PATH.join("course_results.json"), Vec::<CourseResult>::new());
    //     let document_notifs = exam_update(&mut STINE, None, false, &TEST_PATH, true);
    //     assert!(!document_notifs.notifications.is_empty());
    // }

    #[test]
    fn test_document_change() {
        write_data(&TEST_PATH.join("documents.json"), Vec::<Document>::new());
        let document_notifs = documents_update(&STINE, &TEST_PATH, true);
        dbg!(&document_notifs);
        assert!(!document_notifs.notifications.is_empty());
    }

    #[test]
    fn test_document_one_added() {
        let mut current_documents: Vec<Document> = STINE.get_documents().unwrap();
        current_documents.remove(0);

        write_data(&TEST_PATH.join("documents.json"), current_documents);
        let document_notifs = documents_update(&STINE, &TEST_PATH, true);
        dbg!(&document_notifs);
        assert_eq!(document_notifs.notifications.len(), 1);
    }

    #[test]
    fn test_periods_change() {
        write_data(&TEST_PATH.join("send_period_notifications.json"), Vec::<RegistrationPeriod>::new());
        let reg_notifs = period_update(&STINE, &TEST_PATH, true);
        // assert!(!reg_notifs.notifications.is_empty()); // is probably empty, because depends on current date
    }

    #[test]
    fn build_attachment_email() {
        let documents = STINE.get_documents().unwrap();
        let d = documents.get(0).unwrap();
        let content = download_document(&STINE, d).unwrap();
        let attachs = vec![
            Attachment::new("aaa.pdf".to_string()).
                body(content, ContentType::parse("application/pdf").unwrap())
        ];

        build_email("AAa", "New EMail".to_string(), "email@example.com", attachs);
    }
}