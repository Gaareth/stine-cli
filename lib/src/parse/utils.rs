use std::collections::HashMap;
use std::num::ParseFloatError;
use scraper::{ElementRef, Selector};

pub fn wrap_parse_float(unparsed: String) -> Result<f32, String> {
    let parsed: Result<f32, _> = parse_float(&unparsed);

    if let Ok(..) = parsed {
        Ok(parsed.unwrap())
    } else {
        Err(unparsed)
    }
}

pub fn parse_float(s: &str) -> Result<f32, ParseFloatError> {
    let parsable = s.trim().replace(',', ".");
    parsable.parse::<f32>()
}



struct Table {
    names: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn parse_table(table: &ElementRef, column_names: Vec<String>) -> Vec<HashMap<String, String>> {
    if column_names.is_empty() {
        let mut column_names: Vec<String> = Vec::new();
        let col_sel = Selector::parse("td.tbsubhead").expect("Missing thead in table");
        for col in table.select(&col_sel) {
            let inner = col.inner_html().trim().to_string();
            if !inner.is_empty() {
                column_names.push(inner);
            }
        }
    }

    let mut rows: Vec<HashMap<String, String>> = Vec::new();

    let row_sel = Selector::parse("tr").unwrap();
    let selected_rows = table.select(&row_sel);
    for row in selected_rows {
        let data_sel = Selector::parse("td").unwrap();
        let selected_cells = row.select(&data_sel);

        let mut row: HashMap<String, String> = HashMap::new();
        for (col_i, cell) in selected_cells.enumerate() {
            if col_i < column_names.len() {
                row.insert(column_names[col_i].to_string(), cell.inner_html().trim().to_string());
            }
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }

    rows
}

pub fn get_next_selection<'a>(html: ElementRef<'a>, css_selector: &str) -> Option<ElementRef<'a>> {
    html.select(&Selector::parse(css_selector).unwrap()).next()
}

pub fn get_next_selection_html<'a>(html: &'a scraper::Html, css_selector: &str) -> Option<ElementRef<'a>> {
    html.select(&Selector::parse(css_selector).unwrap()).next()
}

// pub(crate) fn get_selections<'a, 'b>(html: ElementRef<'a>, css_selector: &str) -> Select<'a, 'b> {
//     html.select(&Selector::parse(css_selector).unwrap()).collect()
// }

/// Parses a stine argument string
/// Example: -N511551515,-N89150515,-N0014 => \[N89150515,N0014\]
/// **Note:** The first argument gets removed, because you likely don't want it as its the session_id.
/// If you need the session_id, refer to [`Stine`]
pub fn parse_arg_string(args_str: &str) -> Vec<String> {
    let mut args: Vec<String> = args_str.split("ARGUMENTS=").nth(1).unwrap_or_default()
        .split(',').map(std::string::ToString::to_string).collect();

    // first arg is the session id and gets added to every requests
    args.remove(0);

    args
}

/// Parses and processes scraped string.
/// Removes trailing and leading, whitespace, new lines and replaces &nbsp; with a simple whitespace: " "
pub fn parse_string<S: AsRef<str>>(s: S) -> String {
    s.as_ref().replace("&nbsp;", " ").trim().to_string()
}

// TODO:
pub fn remove_multi_whitespace() {
    todo!()
}