use std::str::FromStr;

use log::{debug, error, trace};
use regex::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::{CourseResult, GradeStats, LazyLevel, Semester, SemesterResult};
use crate::parse::utils::{get_next_selection, parse_float, parse_string, wrap_parse_float};
use crate::Stine;
use crate::types::event::{Lazy, LazyLoaded};

/// Parses the various missing types.
/// If not possible to determine concrete missing type, a fallback value will be return as the Err
fn parse_grad_missing(grade_stats: &mut GradeStats, key: &str, value: &str) -> Result<(), String> {
    let i32_value: Option<i32> = value.parse().ok();

    if key.starts_with("fehlend") || key.starts_with("missing") {
        let missing_type = key.split_once(' ').ok_or(String::new())?.1;
        let re = Regex::new("\\((.*)\\)").unwrap();
        let caps = re.captures(missing_type).ok_or(missing_type)?;
        let missing_type = caps.get(1).unwrap().as_str();
        match missing_type {
            "ill" | "krank" => grade_stats.missing_ill = i32_value,

            "without reason" | "ohne grund" => {
                grade_stats.missing_without_reason = i32_value
            }

            // its the same in both languages
            "annulliert"  => {
                grade_stats.missing_canceled = i32_value
            }

            "excused" | "entschuldigt" => {
                grade_stats.missing_excused = i32_value
            }
            _ => {
                return Err(missing_type.to_string());
            }
        }
    } else {
        error!("Failed parsing grade statistic key: {key}, value: {value}")
    }

    Ok(())
}

fn parse_grade_stats_table(html: &Html) -> Vec<(f32, i32)> {
    let grades_sel: Selector = Selector::parse(".nb > tbody > tr:nth-child(1) > td").unwrap();
    let grade_numbers_sel: Selector = Selector::parse(".nb > tbody > tr:nth-child(2) > td").unwrap();

    // let grades = html.select(&grades_sel)
    //     .filter_map(|e| e.inner_html().parse::<f32>().ok().map(|i| i.to_string()));

    let grades = html.select(&grades_sel)
        .filter_map(|e| parse_float(&e.inner_html()).ok());

    let grade_numbers = html.select(&grade_numbers_sel)
        .filter_map(|e| e.inner_html().trim().parse::<i32>().ok());

    grades.zip(grade_numbers).collect::<Vec<(f32, i32)>>()
}

pub fn parse_grade_stats(html: &Html, exam_id: &str) -> GradeStats {
    let row_sel: Selector = Selector::parse(".tb > .tbdata").unwrap();
    let rows = html.select(&row_sel);

    let mut grade_stats = GradeStats {
        grade_map: parse_grade_stats_table(html),
        average: None,
        available_results: None,
        differing_gs_results: None,
        missing_canceled: None,
        missing_excused: None,
        missing_ill: None,
        missing_without_reason: None,
        missing_other: vec![],
    };

    for row in rows {
        let content = parse_string(row.inner_html()).to_lowercase();
        debug!("Grade stat row: {content}");
        let split = content.split_once(':');
        if split.is_none() {
            error!("Failed parsing grade stats {content} of {exam_id}");
            continue;
        }
        let split = split.unwrap();
        let key = split.0.trim();
        let value = parse_string(split.1);

        match key {
            "average" | "durchschnitt" => {
                grade_stats.average = parse_float(&value).ok();
            }
            "available results" | "vorliegende ergebnisse" => {
                grade_stats.available_results = value.parse::<i32>().ok();
            }
            "results with differing gs" | "ergebnisse mit abweichendem bws" => {
                grade_stats.differing_gs_results = value.parse::<i32>().ok();
            }
            _ => {
                if let Err(missing_type) = parse_grad_missing(&mut grade_stats, key, &value) {
                    // missing type is in lowercase, idk if this is okay
                    if let Ok(value) = value.parse() {
                        grade_stats.missing_other.push((missing_type, value));
                    } else {
                        error!("Failed parsing value of missing type {missing_type}: value: {value}")
                    }
                }
            }
        }
    }
    grade_stats
}

/// Parses Course results for one semester by parsing the corresponding table
fn parse_semester_result(html: &Html, stine: &Stine, semester: Semester, lazy_level: LazyLevel) -> SemesterResult {
    let mut course_results: Vec<CourseResult> = vec![];

    let row_sel: Selector = Selector::parse(".nb > tbody:nth-child(2) > tr").unwrap();

    let mut semester_gpa = String::new();
    let mut semester_credits = String::new();


    // let table_sel: Selector = Selector::parse(".nb > tbody").unwrap();
    // let table_ref = html.select(&table_sel).next().unwrap();
    //
    // let parsed_table: Vec<HashMap<String, String>> =
    //     parse_table(&table_ref,
    //                 CourseResult::FIELD_NAMES_AS_ARRAY.iter().map(|s| s.to_string()).collect());

    // for row in parsed_table {
    //     let mut row = hashmap_to_map(row);
    //     row["final_grade"] = Value::from(parse_float(&row["final_grade"].to_string()).ok());
    //     course_results.push(CourseResult::from_map(row));
    // }

    for row in html.select(&row_sel) {
        if get_next_selection(row, "td:nth-child(1)").is_some() {
            let mut number = get_next_selection(row, "td:nth-child(1)").unwrap().inner_html();
            number = parse_string(number);

            let mut name = get_next_selection(row, "td:nth-child(2)").unwrap().inner_html();
            name = parse_string(name);

            let mut final_grade = get_next_selection(row, "td:nth-child(3)").unwrap().inner_html();
            final_grade = parse_string(final_grade);

            let mut credits = get_next_selection(row, "td:nth-child(4)").unwrap().inner_html();
            credits = parse_string(credits);

            let mut status = get_next_selection(row, "td:nth-child(5)").unwrap().inner_html();
            status = parse_string(status);

            let grade_stats: Option<LazyLoaded<GradeStats>>
                = get_next_selection(row, "td:nth-child(7) > script")
                .and_then(|script| {
                    let script = script.inner_html();
                    // some id like: 381865010228083
                    let course_id_regex = Regex::new("-AMOFF,(.*),-N0").unwrap();
                    let caps = course_id_regex.captures(&script);

                    if let Some(caps) = caps {
                        let id = caps.get(1).unwrap().as_str();
                        let id = &id[2..]; // -N381865010228083 -> 381865010228083

                        return if lazy_level != LazyLevel::FullLazy {
                            trace!("NotLazy: Requesting grade stats for course");
                            Some(LazyLoaded {
                                status: Lazy::Loaded(stine.get_grade_stats_for_course(id)),
                                link: id.to_string(),
                            })
                        } else {
                            Some(LazyLoaded {
                                status: Lazy::Unloaded,
                                link: id.to_string(),
                            })
                        }
                    }
                    //TODO: parse grade stats where there was no grading? NOT AMOFF, but ACOUR as argument and regex pattern
                    error!("Failed parsing grade stats for {name} ({semester})");
                    None
                });


            let result: CourseResult = CourseResult {
                number,
                name,
                final_grade: parse_float(&final_grade).ok(),
                credits: if credits.is_empty() { None } else { Some(credits) },
                status,
                grade_stats,
            };

            course_results.push(result);
        } else {
            semester_gpa = get_next_selection(row, "th:nth-child(2)").unwrap().inner_html();
            semester_credits = get_next_selection(row, "th:nth-child(3)").unwrap().inner_html();
        }
    }

    SemesterResult {
        semester,
        courses: course_results,
        semester_gpa: wrap_parse_float(semester_gpa),
        semester_credits: semester_credits.trim().to_string(),
    }
}

/// Parses course results of multiple semesters
pub fn parse_course_results(html_content: String, stine: &Stine,
                            semesters: Vec<Semester>, all_semesters: bool, lazy_level: LazyLevel) -> Vec<SemesterResult> {
    let mut semester_results: Vec<SemesterResult> = Vec::new();

    let html = Html::parse_fragment(&html_content);

    // Selectable semesters dropdown
    let semesters_sel = &Selector::parse("#semester > option").unwrap();

    // loop through all semesters
    for (i, _) in html.select(semesters_sel).enumerate() {
        let semester_option: ElementRef = html.select(
            &Selector::parse(&format!("#semester > option:nth-child({})", i + 1))
                .unwrap()).next().unwrap();

        let semester_name: String = semester_option.inner_html();

        if let Ok(semester_parsed) = Semester::from_str(semester_name.as_str()) {
            if !semesters.contains(&semester_parsed) && !all_semesters {
                continue;
            }

            let semester_argument: String = semester_option.value().attr("value").unwrap().to_string();

            debug!("Parsing semester: {semester_name}");


            // Reload website with new semester information
            let resp = stine.post_with_arg("COURSERESULTS",
                                           vec![
                                               String::from("-N000460"), // sidebar argument, necessary
                                               format!("-N{semester_argument}"), // specifies selected semester
                                           ]).unwrap();

            // actually parse semester results
            let html_to_parse = Html::parse_fragment(&resp.text().unwrap());

            let semester_result = parse_semester_result(
                &html_to_parse, stine, semester_parsed, lazy_level);
            semester_results.push(semester_result);
        } else {
            error!("Failed parsing Semester {semester_name}. => Skipping");
        }
    }


    semester_results
}