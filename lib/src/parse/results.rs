use std::str::FromStr;
use scraper::{ElementRef, Html, Selector};

use crate::parse::utils::{get_next_selection, parse_float, wrap_parse_float};
use crate::{Semester, SemesterResult, CourseResult, parse};
use crate::Stine;

/// Parses Course results for one semester by parsing the corresponding table
fn parse_semester_result(html: &Html, semester: Semester) -> SemesterResult {
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
            number = parse::utils::parse_string(number);

            let mut name = get_next_selection(row, "td:nth-child(2)").unwrap().inner_html();
            name = parse::utils::parse_string(name);

            let mut final_grade = get_next_selection(row, "td:nth-child(3)").unwrap().inner_html();
            final_grade = parse::utils::parse_string(final_grade);

            let mut credits = get_next_selection(row, "td:nth-child(4)").unwrap().inner_html();
            credits = parse::utils::parse_string(credits);

            let mut status = get_next_selection(row, "td:nth-child(5)").unwrap().inner_html();
            status = parse::utils::parse_string(status);


            let result: CourseResult = CourseResult {
                number,
                name,
                final_grade: parse_float(&final_grade).ok(),
                credits: if credits.is_empty() { None } else { Some(credits) },
                status
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
                            semesters: Vec<Semester>, all_semesters: bool) -> Vec<SemesterResult> {
    let mut semester_results: Vec<SemesterResult> = Vec::new();

    let html = Html::parse_fragment(&html_content);

    let semesters_sel = &Selector::parse("#semester > option").unwrap();

    for (i, _) in html.select(semesters_sel).enumerate() {
        let semester_option: ElementRef = html.select(
            &Selector::parse(&format!("#semester > option:nth-child({})", i+1))
                .unwrap()).next().unwrap();

        let semester_name: String = semester_option.inner_html();

        if let Ok(semester_parsed) = Semester::from_str(semester_name.as_str()) {
            if !semesters.contains(&semester_parsed) && !all_semesters {
                continue;
            }

            let semester_argument: String = semester_option.value().attr("value").unwrap().to_string();

            // dbg!(format!("Parsing semester: {semester_name}"));


            // Reload website with new semester information
            let resp = stine.post_with_arg("COURSERESULTS",
                                           vec![
                                               String::from("-N000460"), // sidebar argument, necessary
                                               format!("-N{}", semester_argument), // specifies selected semester
                                           ]).unwrap();

            let html_to_parse = Html::parse_fragment(&resp.text().unwrap());

            let semester_result = parse_semester_result(&html_to_parse, semester_parsed);
            semester_results.push(semester_result);
        } else {
            println!("Failed parsing Semester {}. => Skipping", semester_name);
        }
    }


    semester_results
}