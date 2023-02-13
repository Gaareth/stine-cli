use chrono::{NaiveDateTime};
use scraper::{Html, Selector};

use crate::Document;
use crate::parse::date::{parse_dmy_date, parse_time, stine_naive_to_utc};
use crate::parse::utils::{get_next_selection, get_next_selection_html};
use crate::stine;


// use thiserror::Error;
// #[derive(Error, Debug)]
// pub enum DocumentParseError {
//     MissingNameField,
//     MissingDateField,
//     MissingTimeField,
//     MissingStatusField,
//     MissingDownloadField,
//
//     InvalidDate
// }

pub fn parse_documents(html_content: String) -> Vec<Document> {
    let mut documents: Vec<Document> = Vec::new();

    let html: Html = Html::parse_fragment(&html_content);

    let table = get_next_selection_html(
        &html, ".tb > tbody:nth-child(1)").unwrap();
    for (row_count, row) in table.select(&Selector::parse("tr").unwrap()).enumerate() {
        // first row is the table header, which is inside the tbody??? wtf
        if row_count == 0 {
            continue
        }

        let name = get_next_selection(row, "td:nth-child(1)").unwrap().inner_html();

        let date_str = get_next_selection(row, "td:nth-child(2)").unwrap().inner_html();
        let time_str = get_next_selection(row, "td:nth-child(3)").unwrap().inner_html();


        let naive_dt = NaiveDateTime::new
            (
                parse_dmy_date(date_str.as_str()).unwrap(),
                parse_time(time_str.as_str()).unwrap()
            );

        let datetime = stine_naive_to_utc(naive_dt);

        let status_str = get_next_selection(row, "td:nth-child(4)").unwrap().inner_html();
        let status = if status_str.is_empty() { None } else { Some(status_str) };

        let download = String::from(stine::BASE_URL)
            + get_next_selection(row, ".download").unwrap().value().attr("href").unwrap();

        documents.push(Document {
            name,
            datetime,
            status,
            download,
        });
    }

    documents
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate};
    use crate::Document;
    use crate::parse::date::stine_naive_to_utc;
    use crate::parse::documents::parse_documents;

    #[test]
    fn test_document_parsing() {
        let html_content = r#"
        <table class="tb">
            <tbody><tr>
                <td class="tbhead">Name</td>
                <td class="tbhead">Date</td>
                <td class="tbhead">Time</td>
                <td class="tbhead">Status</td>
                <td class="tbhead">&nbsp;</td>
            </tr>

            <tr>
                <td class="tbdata">OnlineSemesterbescheinigung</td>
                <td class="tbdata">23.08.22</td>
                <td class="tbdata">14:46</td>
                <td class="tbdata"></td>
                <td class="tbdata">
                    <a class="img download" href="/scripts/filetransfer.exe?LINK">Download</a>
                </td>
            </tr>
            <tr>
                <td class="tbdata">OnlineZahlträger</td>
                <td class="tbdata">01.08.22</td>
                <td class="tbdata">18:24</td>
                <td class="tbdata"></td>
                <td class="tbdata">
                    <a class="img download" href="/scripts/filetransfer.exe?LINK">Download</a>
                </td>
            </tr>

            </tbody>
        </table>
        "#;

        let docs = parse_documents(html_content.to_owned());
        assert_eq!(vec![
            Document {
                name: "OnlineSemesterbescheinigung".to_string(),
                datetime: stine_naive_to_utc(
                    NaiveDate::from_ymd(2022, 8, 23).and_hms(14, 46, 0)
                ),
                status: None,
                download: "https://stine.uni-hamburg.de/scripts/filetransfer.exe?LINK".to_string()
            },
            Document {
                name: "OnlineZahlträger".to_string(),
                datetime: stine_naive_to_utc(
                    NaiveDate::from_ymd(2022, 8, 1).and_hms(18, 24, 0)
                ),
                status: None,
                download: "https://stine.uni-hamburg.de/scripts/filetransfer.exe?LINK".to_string()
            },
        ], docs);

    }
}