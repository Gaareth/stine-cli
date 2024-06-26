use std::fmt::{Display, Formatter};
use std::str::FromStr;

use reqwest::blocking::Response;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, CONNECTION, CONTENT_LENGTH, COOKIE, HeaderMap, HeaderValue, HOST, USER_AGENT};
use serde::{Deserialize, Serialize};

use crate::{EventType, Semester, Stine, stine::API_URL};

pub mod cipher;
mod parse;
//TODO: implement: appointments and eventinfo, and messages

// Possible types / prg_names / programm names?:
// GETPERSONTYPE
// GETMATERIAL
// DELETEMESSAGE
// SENDMESSAGECOURSE
// GETEXAMS
// GETMESSAGES
// REPLYMESSAGE
// SETMESSAGESTATUS
// GETEVENTDOWNLOAD
// GETAPPOINTMENTS
// GETEVENTINFO
// GETEVENTS

#[derive(Debug)]
/// StudentEvent
/// Contains general info about an event
/// Endpoint: GETEVENTS
pub struct StudentEvent {
    pub course_id: Option<String>,
    pub course_data_id: Option<String>,
    pub course_number: Option<String>,
    pub course_name: Option<String>,
    pub event_type: Option<String>,
    pub event_category: Option<EventType>,
    pub semester_id: Option<String>,
    pub semester_name: Option<Semester>,
    pub credits: Option<f32>,
    pub small_groups: Option<i32>,
    pub language: Option<String>,
    pub faculty_name: Option<String>,
    pub max_students: Option<i32>,
    pub instructors_string: Option<String>,
    pub module_name: Option<String>,
    pub module_number: Option<String>,
    pub is_listener: Option<bool>,
    pub accepted_status: Option<bool>,
    pub material_present: Option<bool>,
    pub info_present: Option<bool>,
}

// TODO: not every type should be string
/// **Will change**
#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename = "mgns1:studentExam")]
#[serde(rename_all = "camelCase")]
pub struct StudentExam {
    #[serde(rename = "examID")]
    pub exam_id: String,
    pub exam_name: String,
    pub context: String,
    pub context_type: String,
    pub subject: String,
    pub begin_date: String,

    pub due_date: String,

    pub time_from: String,
    pub time_to: String,
    pub grade: String,
    pub grade_description: String,

    /// the instructor(s)
    pub instructor_string: String,
    pub status: String,
    pub status_system: String,

    #[serde(rename = "semesterID")]
    pub semester_id: String,
    pub semester_name: String,

    // #[serde(flatten)]
    // other: HashMap<String, Value>,
}

impl Display for StudentExam {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename = "mgns1:Message")]
pub struct StudentExams {
    #[serde(rename = "$value")]
    pub exams: Vec<StudentExam>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ActorType {
    Applicant,
    Instructor,
    ExternalStudent,
    Sponsor,
    InterestedParties,
    Employee,
    Internship,
    Student,

    Unknown(String),
}

impl FromStr for ActorType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ADM" => Ok(Self::Applicant),
            "DOZ" => Ok(Self::Instructor),
            "EXS" => Ok(Self::ExternalStudent),
            "FOE" => Ok(Self::Sponsor),
            "INT" => Ok(Self::InterestedParties),
            "MAB" => Ok(Self::Employee),
            "PRA" => Ok(Self::Internship),
            "STD" => Ok(Self::Student),

            _ => Ok(Self::Unknown(s.to_string())),
        }
    }
}


impl Stine {
    /// Get mobile endpoint response
    pub fn get_mobile(&self, prg_name: &str, args: Vec<&str>) -> Result<Response, reqwest::Error> {
        let s_id = self.session.as_ref().unwrap().to_string();

        let args = String::from("-A") + &cipher::encrypt_arguments(
            prg_name.to_string(), s_id, args);

        let url = format!("{API_URL}?APPNAME=CampusNet&PRGNAME=ACTIONMOBILE&ARGUMENTS={args}");

        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_str("www.stine.uni-hamburg.de").unwrap());
        headers.insert(ACCEPT, HeaderValue::from_str("application/json").unwrap());
        headers.insert(CONNECTION, HeaderValue::from_str("keep-alive").unwrap());
        headers.insert(COOKIE, format!("cnsc={}", self.cnsc_cookie.as_ref().unwrap()).parse().unwrap());
        headers.insert(USER_AGENT, HeaderValue::from_str("STiNE/202 CFNetwork/1390 Darwin/22.0.0").unwrap());
        headers.insert(CONTENT_LENGTH, HeaderValue::from_str("0").unwrap());
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_str("gzip, deflate, br").unwrap());


        log::debug!("GET to: {url}, \nArguments: {args}");
        self.client.get(url).headers(headers).send()
    }

    pub fn get_exams_mobile(&self) -> Result<StudentExams, anyhow::Error> {
        let xml_response = self.get_mobile("GETEXAMS",
                                           vec!["000000", "STD"])?;
        Ok(parse::parse_get_exams(xml_response.text()?)?)
    }

    pub fn get_actor_type(&self) -> Result<ActorType, anyhow::Error> {
        // "1" for short results, e.g. "STD", "0" for results like "student",
        // however ActorType::FromString only works for the short version
        let xml_response = self.get_mobile("GETPERSONTYPE", vec!["000000", "1"])?;
        parse::parse_actor_type(xml_response.text()?)
    }

    pub fn get_student_events(&self) -> Result<Vec<StudentEvent>, anyhow::Error> {
        let xml_response = self.get_mobile("GETEVENTS", vec!["000000"])?;
        parse::parse_student_events(xml_response.text()?)
    }
}
