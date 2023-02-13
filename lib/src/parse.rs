pub mod date;
pub mod results;
pub mod utils;
pub mod registrations;
pub mod documents;
pub mod periods;

use crate::parse::utils::{get_next_selection, get_next_selection_html, parse_arg_string, parse_string};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::str::FromStr;
use std::time::Instant;
use chrono::{NaiveDateTime};
use indicatif::{ProgressBar, ProgressStyle};
use scraper::{ElementRef, Html, Selector};

use crate::{Appointment, CourseInfo, EventType, Exam, Group, Lazy, LazyLevel, LazyLoaded, Module, ModuleCategory, SubModule};
use crate::Language;
use crate::Stine;
use crate::parse::date::pre_process_date_string;

// idea: fn parse_module_intern
//       pub fn parse_module() {
//            parse_module_intern();
//                 modules.push();
// }

pub fn parse_get_module_category(html_content: String, stine: &Stine, category_name: &str, lazy: LazyLevel)
-> Option<ModuleCategory> {
    let html_fragment = Html::parse_fragment(&html_content);

    let selector = Selector::parse("#contentSpacer_IE > ul > li").unwrap();
    let anchor_selector = Selector::parse("a").unwrap();

    for category_element in  html_fragment.select(&selector) {
        let anchor: ElementRef = category_element.select(&anchor_selector).next().unwrap();
        let name = anchor.inner_html().trim().to_string();

        // slightly inefficient, as the name gets parsed twice
        // in this method as well as in parse_module_category
        if name == category_name {
            return Some(parse_module_category(&category_element, stine, lazy));
        }
    }

    None
}

pub fn parse_module_category(category_item: &ElementRef, stine: &Stine, lazy: LazyLevel)
    -> ModuleCategory {
    parse_module_category_internal(
        category_item, stine, false, lazy, 0, 0)
}

pub fn parse_module_category_progress_bar(category_item: &ElementRef,
                                           stine: &Stine,
                                           lazy: LazyLevel,
                                           category_index: usize,
                                           category_size: usize)
                             -> ModuleCategory {
    parse_module_category_internal(
        category_item, stine, true, lazy, category_index, category_size)
}

/// Parses a module category from the category item element
/// # Arguments
///  -   category_item: &ElementRef,
///  -   stine: &Stine,
///  -   print_progress_bar: bool,: prints a progress bar
///  -   lazy: LazyLevel,
///  -   category_index: usize: only need for print_progress_bar
///  -   category_size: usize: only need for print_progress_bar
fn parse_module_category_internal(
    category_item: &ElementRef,
    stine: &Stine,
    print_progress_bar: bool,
    lazy: LazyLevel,
    category_index: usize,
    category_size: usize,
)

    -> ModuleCategory {

    let anchor_selector = Selector::parse("a").unwrap();
    let anchor: ElementRef = category_item.select(&anchor_selector).next().unwrap();
    let category_link = anchor.value().attr("href").unwrap().to_string();
    let category_name = anchor.inner_html().trim().to_string();

    let mut module_category = ModuleCategory {
        name: category_name.clone(),
        modules: vec![],
        orphan_submodules: vec![]
    };

    let args = parse_arg_string(category_link.as_str());
    let resp_category = stine.post_with_arg("REGISTRATION", args).unwrap();

    let html_fragment = Html::parse_fragment(&resp_category.text().unwrap());
    let row_selector = Selector::parse(".tbcoursestatus > tbody > tr").unwrap();
    let module_rows: Vec<ElementRef> = html_fragment.select(&row_selector).collect();

    let mut latest_module: Option<Module> = None;

    if print_progress_bar {
        println!("[{}/{}] Parsing category {}", category_index + 1, category_size, category_name);
    }

    let pb = ProgressBar::new(module_rows.len() as u64);

    pb.set_style(ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] [{bar:.cyan/blue}] {pos:>7}/{len:7} {msg}").unwrap()
        .progress_chars("#>-"));

    for row in module_rows {
        let module_el = row.select(&Selector::parse(".tbsubhead.dl-inner").unwrap()).next();
        let sub_module_el = row.select(&Selector::parse(".tbdata.dl-inner").unwrap()).next();

        if let Some(module_el) = module_el {

            if let Some(latest_module) = latest_module.clone() {
                module_category.modules.push(latest_module);
            }
            let module = parse_module(module_el, stine, lazy);

            if print_progress_bar {
                pb.set_message(format!("Module: {}", module.name));
            }

            latest_module = Some(module);

        } else if let Some(sub_module_el) = sub_module_el {
            let sub_module = parse_sub_module(sub_module_el, stine, lazy);

            if print_progress_bar {
                pb.set_message(format!("SubModule: {}", sub_module.name));
            }

            if let Some(latest_module) = latest_module.as_mut() {
                latest_module.sub_modules.push(sub_module);
            } else {
                module_category.orphan_submodules.push(sub_module);
            }
        }

        pb.inc(1);

    }
    if print_progress_bar {
        pb.finish_with_message("Finish");
    }
    // break;

    // save last module
    if let Some(latest_module) = latest_module.clone() {
        module_category.modules.push(latest_module);
    }

    module_category
}

pub fn search_module_by_id(html_content: String, stine: &Stine, module_id: String, lazy: LazyLevel) {
    let html_fragment = Html::parse_fragment(&html_content);
    let selector = Selector::parse("#contentSpacer_IE > ul > li").unwrap();
    for category_element in html_fragment.select(&selector) {

    }
}

pub fn parse_modules(html_content: String, stine: &Stine, print_progress_bar: bool, lazy: LazyLevel)
    -> Vec<ModuleCategory> {
    let elapsed = Instant::now();

    let html_fragment = Html::parse_fragment(&html_content);

    let selector = Selector::parse("#contentSpacer_IE > ul > li").unwrap();
    let mut categories: Vec<ModuleCategory> = Vec::new();

    let category_elements: Vec<ElementRef> = html_fragment.select(&selector).collect();

    for (i, category_item) in category_elements.iter().enumerate() {
        let module_category =  parse_module_category_progress_bar(
            category_item, stine, lazy,
            i, category_elements.len());

        categories.push(module_category);
    }

    if print_progress_bar {
        println!("Finished parsing all Stine modules in {}", indicatif::HumanDuration(elapsed.elapsed()));
    }
    categories
}

pub fn parse_exams(html: &Html, stine: &Stine) -> Vec<Exam> {
    let mut exams: Vec<Exam> = Vec::new();

    let summary = if Stine::get_language_from_resp(html) == Language::German { "Modulabschlussprüfungen" } else { "Final module exams" };
    let exam_sel = Selector::parse(&format!(".tb[summary=\"{}\"] > tbody > .tbdata", summary)).unwrap();
    let selection = html.select(&exam_sel);

    for row in selection {
        let exam_name = get_next_selection(row, ".rw-detail-exam").unwrap().inner_html();
        let exam_datetime = get_next_selection(row, ".rw-detail-date").unwrap().inner_html();
        let exam_instructors = get_next_selection(row, ".rw-detail-instructors").unwrap().inner_html();
        let exam_mandatory = get_next_selection(row, ".rw-detail-compulsory").unwrap().inner_html();

        let mut date_vec = exam_datetime.split(',').map(str::trim).collect::<Vec<&str>>();
        let time_vec = date_vec.last().unwrap().split(" - ").collect::<Vec<_>>();
        date_vec.remove(date_vec.len()-1);

        let date_str = pre_process_date_string(&date_vec.join(", "));

        let mut df = None;
        let mut dt = None;

        if let Ok(date_str) = date_str {
            // "Do, 21. Jul. 2022"

            let format =  "%a,%e. %b. %Y %H:%M";
            let format2 = "%a,%e. %b %Y %H:%M";

            // let error_msg: &str = &*format!("Failed parsing exam date {date_str}");

            // // dbg!(&date_str);
            // let naive_date = NaiveDate::parse_from_str(&date_str, date_format)
            //     .expect(error_msg);
            //
            // let from_time = NaiveTime::parse_from_str(time_vec[0].trim(), time_format)
            //     .expect(error_msg);
            // let to_time = NaiveTime::parse_from_str(time_vec[1].trim(), time_format)
            //     .expect(error_msg);
            //
            // dt = Some(naive_date.and_time(to_time));
            // df = Some(naive_date.and_time(from_time));

            dt = date::try_parse_datetime(
                format!("{} {}", date_str, time_vec[0].trim()).as_str(),
                format, format2).ok();

            df = date::try_parse_datetime(
                format!("{} {}", date_str, time_vec[1].trim()).as_str(),
                format, format2).ok();
        }


        let is_mandatory: Option<bool> = match exam_mandatory.to_lowercase().trim() {
            "ja" | "yes" => Some(true),
            "nein" | "no" => Some(false),
            _ => None
        };

        let exam = Exam {
            name: parse_string(exam_name),
            datetime_from: df,
            datetime_to: dt,
            instructors: parse_instructors(exam_instructors),
            is_mandatory,
            is_mandatory_raw: exam_mandatory.trim().to_lowercase(),
        };

        exams.push(exam);
    }

    exams
}

pub fn parse_module(module: ElementRef, stine: &Stine, lazy: LazyLevel)  -> Module {
    let module_anchor = module.select(
        &Selector::parse("p > strong > a").unwrap()
    ).next().unwrap();

    let mod_text: String = module_anchor.text().collect::<Vec<&str>>().join(" ");
    // dbg!(&mod_text);

    let module_number: String = mod_text.split_once(' ').unwrap().0.trim().to_string();
    let module_name: String = mod_text.split_once(' ').unwrap().1.trim().to_string();
    let module_link: String = module_anchor.value().attr("href").unwrap().to_string();

    let module_owner = module.select(&Selector::parse("p").unwrap()).nth(1).unwrap().inner_html().trim().to_string();

    let mut module = Module {
        module_number,
        name: module_name,
        sub_modules: vec![],
        exams: vec![],
        owner: module_owner,
        timetable_name: None,
        duration: None,
        electives: None,
        credits: None,
        start_semester: None,
        attributes: HashMap::new(),
    };

    if lazy == LazyLevel::FullLazy {
        return module
    }

    let resp = stine.post_with_arg("MODULEDETAILS", parse_arg_string(module_link.as_str())).unwrap();
    let html_fragment = Html::parse_fragment(&resp.text().unwrap());

    let mut text: Vec<&str> = html_fragment.select(&Selector::parse(".tbdata > td").unwrap()).next().unwrap().text().collect();
    text = text.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    let mut latest_key: Option<String> = None;

    for (c, entry) in text.iter().enumerate() {
        match entry.to_lowercase().trim() {
            "displayed in timetable as:" | "anzeige im stundenplan:" => {
                module.timetable_name = Some(text[c+1].to_string())
            },
            "duration:" | "dauer:" => {
                module.duration = text[c+1].parse::<i32>().ok()
            },
            "number of electives:" | "anzahl wahlkurse:" => {
                module.electives = text[c+1].parse::<i32>().ok()
            },
            "credits:" => {
                module.credits = Some(text[c+1].to_string())
            },
            "start semester:" | "startsemester:" => {
                module.start_semester = Some(text[c+1].to_string())
            },

            _ => {
                // multiline values
                if let Some(latest) = latest_key.clone() {
                    if entry.trim().chars().last().unwrap_or_default() != ':'
                        && text.get(c + 1).unwrap_or(&"").trim().chars().last().unwrap_or_default() != ':' {

                        let v = module.attributes.entry(latest).or_default();
                        let new_v = entry.trim().to_string();

                        if !v.is_empty() {
                            v.push('\n');
                        }

                        v.push_str(new_v.as_str());
                    }
                }


                let value = text.get(c+1);

                if let Some(value) = value {

                    // check if line is a key
                    if entry.trim().chars().last().unwrap_or_default() == ':' && entry.trim().len() > 1 {
                        module.attributes.insert(entry.trim().to_lowercase(), (*value).to_string());
                    } else if !entry.contains(':') && value.trim() == ":" {
                        latest_key = Some(entry.trim().to_lowercase());
                        module.attributes.insert(entry.trim().to_lowercase(), String::new());
                    }
                }
            }
        }
    }

    module.exams = parse_exams(&html_fragment, stine);
    module
}


pub fn parse_instructors(instructors_str: String) -> Vec<String> {
    if instructors_str.contains(';') {
        instructors_str.split(';').map(|s| s.trim().to_string()).collect()
    } else {
        vec![instructors_str]
    }
}

pub fn parse_attributes(key: String, value: String, event_inf: &mut CourseInfo) {

    let key = if key.chars().last().unwrap_or_default() == ':' {
        &key[..key.len()-1]
    } else {
        &key
    };


    match key.to_lowercase().trim() {
        "lehrende" | "instructors" => {
            event_inf.instructors = Some(parse_instructors(value));
        },
        "veranstaltungsart" | "event type" => {
            event_inf.event_type = EventType::from_str(value.as_str()).ok();
            event_inf.event_type_raw = Some(value);
        },
        "anzeige im stundenplan" | "displayed in timetable as" => {
            event_inf.timetable_name = Some(value)
        },
        "semesterwochenstunden" | "hours per week" => {
            event_inf.hours_per_week = value.parse::<i32>().ok()
        },
        "credits" => event_inf.credits = Some(value),
        "unterrichtssprache" | "language of instruction" => event_inf.language = Some(value),
        "min. | max. teilnehmerzahl" | "min. | max. participants" => {
            if value.contains('|') {
                let mut split = value.split('|');

                if let Some(min) = split.next() {
                    if let Ok(min_int) = i32::from_str(min.trim()) {
                        event_inf.min_participants = Some(min_int)
                    }
                }

                if let Some(max) = split.next() {
                    if let Ok(max_int) = i32::from_str(max.trim()) {
                        event_inf.max_participants = Some(max_int)
                    }
                }

            } else {
                event_inf.instructors = Some(vec![value]);
            }
        },

        _ => {
            let mut attributes = event_inf.attributes.clone().unwrap_or_default();
            attributes.insert(key.to_string(), value);
            event_inf.attributes = Some(attributes);
        }
    }
}



/// Parses appointment date string
/// # Arguments
/// * `date_str` - Date string in the format: "%a,%e. %b. %Y %H:%M"
///                or if this fails: "%a,%e. %B %Y %H:%M"
pub fn parse_appointment_datetime(date_str: &str) -> Result<NaiveDateTime, Box<dyn Error>> {
    // format: Fri, 8. Apr. 2022 10:15 11:45
    let format = "%a,%e. %b. %Y %H:%M";

    // format: Fri, 8. May 2022 10:15 11:45
    let format2 = "%a,%e. %B %Y %H:%M";

    let date_str = &pre_process_date_string(date_str)?;

    match NaiveDateTime::parse_from_str(date_str, format) {
        Ok(dt) => Ok(dt),
        Err(_) => {
            match NaiveDateTime::parse_from_str(date_str, format2) {
                Ok(dt) => Ok(dt),
                Err(err) => Err(Box::new(err))
            }
        },
    }
}

/// Parses Appointments
/// # Arguments
/// * `table` -  requires tbody > tr,
///            and tr > .rw-course-date | .rw-course-from |.rw-course-to | .rw-course-room | .rw-course-instruct
fn parse_appointments(table: ElementRef) -> Vec<Appointment> {
    let mut appointments: Vec<Appointment> = Vec::new();

    for row in table.select(&Selector::parse("tbody > tr").unwrap()) {
        if let Some(date) = get_next_selection(row, ".rw-course-date") {
            let date = date.inner_html();
            let from = get_next_selection(row, ".rw-course-from").unwrap().inner_html().trim().to_string();
            let to = get_next_selection(row, ".rw-course-to").unwrap().inner_html().trim().to_string();
            let room_wrapper = get_next_selection(row, ".rw-course-room").unwrap();
            let mut room = room_wrapper.inner_html().trim().to_string();
            // sometimes .rw-course-room contains a link or span to the room
            if let Some(room_link) = get_next_selection(room_wrapper, "[name=\"appointmentRooms\"]") {
                room = room_link.inner_html().trim().to_string();
            }

            let instructor = get_next_selection(row, ".rw-course-instruct").unwrap().inner_html().trim().to_string();

            appointments.push(Appointment {
                from: date::parse_stine_datetime(&format!("{date} {from}")).ok(),
                to: date::parse_stine_datetime(&format!("{date} {to}")).ok(),
                room,
                instructors: parse_instructors(instructor),
            })
        }
    }

    appointments
}

fn parse_groups(table: ElementRef, stine: &Stine, lazy: LazyLevel) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let paragraphs_sel = Selector::parse(".dl-inner > p").unwrap();

    for li in table.select(&Selector::parse("ul > li").unwrap()) {
        let group_name = li.select(&Selector::parse(".dl-ul-li-headline > strong").unwrap()).next().unwrap().inner_html();
        let group_link = get_next_selection(li, ".dl-link > a").unwrap().value().attr("href").unwrap();
        let paragraphs: Vec<ElementRef> = li.select(&paragraphs_sel).into_iter().collect();

        let instructors = paragraphs[1].inner_html();
        let schedule = paragraphs[2].inner_html();

        let appointments_lazy =  if !lazy.is_lazy() {
           Lazy::Loaded(parse_group_appointments(group_link, stine))
        } else {
            Lazy::Unloaded
        };

        groups.push(Group {
            name: group_name,
            instructors: parse_instructors(instructors),
            schedule_str: schedule,
            appointments: LazyLoaded {
                link: group_link.to_owned(),
                status: appointments_lazy,
            },
        })
    }

    groups
}


pub fn parse_group_appointments(group_link: &str, stine: &Stine) -> Vec<Appointment> {
    let resp_group = stine.post_with_arg("COURSEDETAILS",
                                         parse_arg_string(group_link)).unwrap();
    let html_fragment = Html::parse_fragment(&resp_group.text().unwrap());

    let table_selector = Selector::parse(".tb").unwrap();

    let mut appointments = vec![];
    for table in html_fragment.select(&table_selector) {
        let table_caption = table.select(&Selector::parse("caption").unwrap()).next();
        if let Some(caption) = table_caption {
            if vec!["appointments", "termine"].contains(&caption.inner_html().to_lowercase().trim()) {
                appointments = parse_appointments(table);
            }
        }
    }

    appointments
}

pub fn parse_tables(html_fragment: Html, sub_module: &mut SubModule,
                    stine: &Stine, lazy: LazyLevel, link: String) {
    sub_module.appointments = LazyLoaded {
        status: Lazy::Loaded(None),
        link: link.to_owned(),
    };

    sub_module.groups = LazyLoaded {
        status: Lazy::Loaded(None),
        link: link.to_owned(),
    };

    let table_selector = Selector::parse(".tb").unwrap();

    for table in html_fragment.select(&table_selector) {
        let table_caption = table.select(&Selector::parse("caption").unwrap()).next();

        if table_caption.is_some() {
            let caption = table_caption.unwrap().inner_html();
            if vec!["appointments", "termine"].contains(&caption.to_lowercase().trim()) {
                // dbg!(&sub_module.event_inf.name);
                let appointments = Some(parse_appointments(table));
                sub_module.appointments = LazyLoaded {
                    status: Lazy::Loaded(appointments),
                    link: link.to_owned(),
                };
            }
        } else {
            // is not a table but prob. a div
            // dbg!(table.inner_html());
            let tbhead = table.select(&Selector::parse(".tbhead").unwrap()).next().unwrap().inner_html();
            if vec!["kleingruppe(n)", "small group(s)"].contains(&tbhead.to_lowercase().trim()) {

                if let Some(show_all_groups)
                    = get_next_selection(table, ".tbdata > a") {

                    // In case the current site shows a specific group, this will show all groups again

                    let link = show_all_groups.value().attr("href").unwrap();
                    let resp = stine.post_with_arg(
                        "COURSEDETAILS", parse_arg_string(link)).unwrap();
                    let html = Html::parse_fragment(&resp.text().unwrap());
                    let table = html.select(&table_selector).next().unwrap();

                    let groups = Some(parse_groups(table, stine, lazy));
                    sub_module.groups = LazyLoaded {
                        status: Lazy::Loaded(groups),
                        link: link.to_owned(),
                    };

                } else {
                    let groups = Some(parse_groups(table, stine, lazy));
                    sub_module.groups = LazyLoaded {
                        status: Lazy::Loaded(groups),
                        link: link.to_owned(),
                    };
                }
            }
        }

    }
}

// lecture or exercise
pub fn  parse_sub_module(sub_module_element: ElementRef, stine: &Stine, lazy: LazyLevel) -> SubModule {

    let name = get_next_selection(sub_module_element, ".eventTitle")
        .map_or_else(
            || get_next_selection(sub_module_element, "a").unwrap(),
                     |name| name
        ).inner_html().trim().to_string();

    // let owner = sub_module_element.select(&Selector::parse("p").unwrap()).nth(1).unwrap().inner_html().trim().to_string();
    let course_link = get_next_selection(sub_module_element, "a")
        .unwrap().value().attr("href").unwrap().to_string();


    let submodule_id = parse_arg_string(course_link.as_str())[2]
        .split("-N").nth(1).unwrap().to_owned();

    let course_number = name.split_whitespace().next().unwrap().to_string();

    let mut course_info = CourseInfo {
        event_type: None,
        event_type_raw: None,
        instructors: None,
        timetable_name: None,
        hours_per_week: None,
        credits: None,
        language: None,
        min_participants: None,
        max_participants: None,
        attributes: None
    };

    let mut sub_module = SubModule {
        id: submodule_id,
        course_number,
        name,
        info: LazyLoaded::unloaded(course_link.to_owned()),
        appointments: LazyLoaded::unloaded(course_link.to_owned()),
        groups: LazyLoaded::unloaded(course_link.to_owned())
    };

    if lazy == LazyLevel::FullLazy {
        return sub_module
    }

    let resp = stine.post_with_arg("COURSEDETAILS",
                                   parse_arg_string(course_link.as_str())).unwrap();

    let html_fragment = Html::parse_fragment(&resp.text().unwrap());

    parse_course_info(&html_fragment, stine);

    parse_tables(html_fragment, &mut sub_module, stine, lazy, course_link);

    sub_module
}

pub fn parse_course_info(html_fragment: &Html, stine: &Stine) -> CourseInfo {
    let mut course_info = CourseInfo::default();

    let key_sel = Selector::parse("b").unwrap();
    let keys: HashSet<String> = get_next_selection_html(&html_fragment, ".tbdata")
        .unwrap().select(&key_sel)
        .into_iter().map(|s| s.inner_html().trim().to_owned()).collect();

    let mut latest_key: Option<&str> = None;
    let mut latest_value: String = String::new();

    let text: Vec<&str> = get_next_selection_html(&html_fragment, ".tbdata")
        .unwrap().text().map(str::trim).collect();
    for line in text {

        if let Some(latest_key) = latest_key {
            if keys.contains(line) {
                parse_attributes(latest_key.to_string(), latest_value, &mut course_info);
                latest_value = String::new();
            }
        }

        if keys.contains(line) {
            latest_key = Some(line);
        } else if line != ":" && !line.is_empty() {
            if !latest_value.is_empty() && !line.is_empty() {
                latest_value += "\n";
            }
            latest_value += line;
        }
    }

    // last one
    if let Some(latest_key) = latest_key {
        parse_attributes(latest_key.to_string(), latest_value, &mut course_info);
    }

    course_info
}