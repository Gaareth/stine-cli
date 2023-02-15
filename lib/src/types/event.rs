use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Utc};
// use mapstruct::derive::{FromMap};
use scraper::Html;
use serde::{Deserialize, Serialize};
use struct_field_names_as_array::FieldNamesAsArray;

use crate::{parse, Stine};
use crate::LazyLevel::NotLazy;
use crate::parse::{parse_group_appointments, utils};

// use crate::types::RegistrationPeriod::{ChangesAndCorrections, Early, FirstSemester, General, Late};


/// # ModuleCategory
/// ## Attributes:
/// * name: String
/// * modules: Vec<Module>
/// * `orphan_submodules: Vec<SubModule>` - Submodules without parent module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleCategory {
    pub name: String,
    pub modules: Vec<Module>,
    pub orphan_submodules: Vec<SubModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exam {
    pub name: String,
    pub datetime_from: Option<DateTime<Utc>>,
    pub datetime_to: Option<DateTime<Utc>>,
    pub instructors: Vec<String>,
    pub is_mandatory: Option<bool>,
    pub is_mandatory_raw: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    /// Module-Number: InfB-Se1
    pub module_number: String,
    pub name: String,
    pub sub_modules: Vec<SubModule>,
    pub exams: Vec<Exam>,

    pub owner: String,
    pub timetable_name: Option<String>,
    pub duration: Option<i32>,
    pub electives: Option<i32>,
    pub credits: Option<String>,
    pub start_semester: Option<String>,

    pub attributes: HashMap<String, String>,
}


#[derive(Debug, Default, Clone, Serialize, Deserialize)]
/// # CourseInfo
/// Lazy Loaded Course info
/// Note: `event_type` may not be parsed correctly (see [`EventType`] for supported types),
/// `event_type_raw` is used as a fallback.
pub struct CourseInfo {
    pub event_type: Option<EventType>,
    pub event_type_raw: Option<String>,
    pub instructors: Option<Vec<String>>,
    pub timetable_name: Option<String>,
    pub hours_per_week: Option<i32>,
    pub credits: Option<String>,
    pub language: Option<String>,
    pub min_participants: Option<i32>,
    pub max_participants: Option<i32>,

    pub attributes: Option<HashMap<String, String>>,
}


/// # EventType
/// Supported EventTypes for SubModules
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EventType {
    Lecture,
    Exercise,
    Project,
    Internship,
    Seminar,
    Proseminar,
    Tutorial,
    LectureSeries,
    /// General Professional Skills course or ABK-Kurs in German
    GPSCourse,
}

impl FromStr for EventType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().trim() {
            "vorlesung" | "lecture" => Ok(Self::Lecture),
            "übung" | "exercise" | "practical course/lab" => Ok(Self::Exercise),
            "projekt" | "project" => Ok(Self::Project),
            "praktikum" | "internship" => Ok(Self::Internship),
            "seminar" | "seminar" => Ok(Self::Seminar),
            "proseminar" | "introductory seminar" => Ok(Self::Proseminar),
            "tutorium" | "tutorial" => Ok(Self::Tutorial),
            "ringvorlesung" | "lecture series" => Ok(Self::LectureSeries),
            "abk-kurse" | "general professional skills courses" => Ok(Self::GPSCourse),

            _ => Err(()),
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum Lazy<T> {
    Loaded(T),
    Unloaded,
}


/// Laziness levels
/// FullLazy: only the main request to stine will be made
/// NotLazy: will scrape the whole requested object
/// this implementation might be really stupid -.-, but was made to reduce the calls to stine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LazyLevel {
    /// No further API Calls are allowed
    FullLazy = 0,
    OneLink = 1,
    /// Two API Calls to Stine are allowed
    TwoLinks = 2,
    ThreeOrMoreLinks = 3,

    /// Will not prematurely? return data. Will make requests to Stine until full obj is scraped.
    NotLazy = 10,
}

impl LazyLevel {
    pub const fn is_lazy(&self) -> bool {
        !matches!(self, LazyLevel::NotLazy)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LazyLoaded<T> {
    pub(crate) status: Lazy<T>,
    pub(crate) link: String,
}

impl<T> LazyLoaded<T> {
    pub fn unwrap(&self) -> &T {
        match &self.status {
            Lazy::Loaded(data) => data,
            Lazy::Unloaded => {
                panic!("Unwrap failed. Called `unwrap` on `LazyLoaded` with status `Lazy::Unloaded`")
            }
        }
    }

    pub const fn unloaded(link: String) -> Self {
        LazyLoaded {
            status: Lazy::Unloaded,
            link,
        }
    }
}

/// # SubModule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubModule {
    pub id: String,

    /// Lehrveranstaltung-nummer (LV-Nummer): 62-...
    pub course_number: String,

    pub name: String,

    pub(crate) info: LazyLoaded<CourseInfo>,
    pub(crate) appointments: LazyLoaded<Option<Vec<Appointment>>>,
    pub(crate) groups: LazyLoaded<Option<Vec<Group>>>,
}

impl SubModule {
    /// Loads [`CourseInfo`], [`Appointment`] and [`Group`]
    pub fn lazy_load(&mut self, stine: &Stine) {
        let link = self.appointments.link.clone();

        let resp = stine.post_with_arg(
            "COURSEDETAILS",
            utils::parse_arg_string(self.info.link.as_str())).unwrap();
        let html = Html::parse_fragment(&resp.text().unwrap());

        // Loads `info`
        let course_info = parse::parse_course_info(&html, stine);
        self.info = LazyLoaded {
            status: Lazy::Loaded(course_info),
            link: link.clone(),
        };

        // Loads `appointments` and `groups`
        parse::parse_tables(html, self, stine, NotLazy, link);
    }

    /// returns [`CourseInfo`]
    /// # Side effects:
    /// Loads [`CourseInfo`], [`Appointment`] and [`Group`]
    pub fn info(&mut self, stine: &Stine) -> CourseInfo {
        match &self.info.status {
            Lazy::Loaded(info) => {
                info.clone()
            }
            Lazy::Unloaded => {
                self.lazy_load(stine);
                self.info.unwrap().clone()
            }
        }
    }

    /// returns [`Appointment`]
    /// # Side effects:
    /// Loads [`CourseInfo`], [`Appointment`] and [`Group`]
    pub fn appointments(&mut self, stine: &Stine) -> Option<Vec<Appointment>> {
        match &self.appointments.status {
            Lazy::Loaded(appointments) => {
                appointments.as_ref().cloned()
            }
            Lazy::Unloaded => {
                self.lazy_load(stine);
                self.appointments.unwrap().as_ref().cloned()
            }
        }
    }

    /// returns [`Group`]
    /// # Side effects:
    /// Loads [`CourseInfo`], [`Appointment`] and [`Group`]
    pub fn groups(&mut self, stine: &Stine) -> Option<Vec<Group>> {
        match &self.groups.status {
            Lazy::Loaded(groups) => {
                groups.as_ref().cloned()
            }
            Lazy::Unloaded => {
                self.lazy_load(stine);
                self.groups.unwrap().as_ref().cloned()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    pub instructors: Vec<String>,
    pub schedule_str: String,
    pub(crate) appointments: LazyLoaded<Vec<Appointment>>,
}

impl Group {
    pub fn get_appointments(&mut self, stine: &Stine) -> Vec<Appointment> {
        match &self.appointments.status {
            Lazy::Loaded(appointments) => {
                appointments.clone()
            }
            Lazy::Unloaded => {
                let link = self.appointments.link.clone();

                let data = parse_group_appointments(
                    link.as_str(), stine);
                self.appointments = LazyLoaded {
                    status: Lazy::Loaded(data),
                    link,
                };
                self.appointments.unwrap().clone()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appointment {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub room: String,
    pub instructors: Vec<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize, FieldNamesAsArray)]
pub struct CourseResult {
    pub number: String,
    pub name: String,
    pub final_grade: Option<f32>,
    pub credits: Option<String>,
    pub status: String,

    pub(crate) grade_stats: Option<LazyLoaded<GradeStats>>,
}

impl CourseResult {
    pub fn get_grade_stats(&mut self, stine: &Stine) -> Option<GradeStats> {
        if let Some(grade_stats) = &self.grade_stats {
            match &grade_stats.status {
                Lazy::Loaded(stats) => { Some(stats.clone()) }
                Lazy::Unloaded => {
                    let link = grade_stats.link.clone();

                    let data = stine.get_grade_stats_for_course(&link);

                    self.grade_stats = Some(LazyLoaded {
                        status: Lazy::Loaded(data),
                        link,
                    });
                    Some(self.grade_stats.as_ref().unwrap().unwrap().clone())
                }
            }
        } else {
            None
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradeStats {
    /// Map grade to number of people who got this grade
    /// Should this be a map? Would loose f32 as key and the ordering :(
    pub grade_map: Vec<(f32, i32)>,
    pub average: Option<f32>,
    pub available_results: Option<i32>,
    /// Results with another grading system (GS) (I think)
    pub differing_gs_results: Option<i32>,

    /// rarely used
    ///
    /// *Weird Info*: this reason is called "anulliert" in stine in english??.
    /// So one may ask, if reasons get manually inserted, or if the devs of stine didn't add an
    /// english translation for this case
    pub missing_canceled: Option<i32>,
    /// rarely used
    pub missing_excused: Option<i32>,

    pub missing_ill: Option<i32>,
    pub missing_without_reason: Option<i32>,
    /// In case there is a here not used case. (reason, people missing for this reason)
    /// will be in lowercase, if you think this is wrong, please raise an issue
    pub missing_other: Vec<(String, i32)>,
}
