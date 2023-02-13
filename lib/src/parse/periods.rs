use scraper::{Html, Selector};
use crate::parse::utils::{get_next_selection_html};
use crate::RegistrationPeriod;

pub fn parse_registration_periods(html_content: String) -> Vec<RegistrationPeriod> {
    let mut registration_periods: Vec<RegistrationPeriod> = Vec::new();

    let html: Html = Html::parse_fragment(&html_content);

    let table_body = get_next_selection_html(
        &html ,"#contentSpacer_IE > table > tbody").unwrap();

    for row in table_body.select(&Selector::parse("tr").unwrap()) {
        let td_sel = Selector::parse("td").unwrap();

        let mut columns = row.select(&td_sel);
        let period_name = columns.next().unwrap().inner_html();
        let period_date = columns.next().unwrap().inner_html();

        registration_periods.push(
            RegistrationPeriod::parse(
                period_name.as_str(),
                period_date.trim()
            ).unwrap()
        );
    }

    registration_periods
}


#[cfg(test)]
mod tests {
    use crate::parse::date::stine_naive_to_utc;
    use crate::parse::periods::parse_registration_periods;
    use chrono::NaiveDate;
    use crate::{Period, RegistrationPeriod};

    #[test]
    fn test_registration_period_parsing() {
        let html_content = r#"
        <div id="contentSpacer_IE">
        <table style="width:700px;" height="150">
             <tbody><tr>
                <td>Early registration period</td>
                <td>Mon, 20 June 2022, 9 am to Thu, 30 June 2022 , 1 pm</td>
              </tr>
              <tr>
                <td>General registration period</td>
                <td>Thu, 1 September 2022, 9 am to Thu, 22 September 2022, 1 pm</td>
              </tr>
              <tr>
                <td>Late registration period</td>
                <td>Tue, 4 October 2022, 9 am to Thu, 6 October 2022, 1 pm</td>
              </tr>
              <tr>
                <td>Registration period for first-semester students</td>
                <td>Mon, 10 October 2022, 9 am to Thu, 13 October 2022, 4 pm</td>
               <!-- ACHTUNG: in einem SOSE: Ende 1 pm , im einem WISE Ende 4 pm -->
              </tr>
              <tr>
                <td>Changes and corrections period</td>
                <td>Mon, 17 October 2022, 9 am to Thu, 27 October 2022, 1 pm</td>

              </tr>
            </tbody>
        </table>
        </div>
        "#;

        let periods = parse_registration_periods(html_content.to_owned());
        assert_eq!(vec![
            RegistrationPeriod::Early(
                Period {
                    start: stine_naive_to_utc(
                        NaiveDate::from_ymd(2022, 6, 20).and_hms(9, 0, 0)
                    ),
                    end: stine_naive_to_utc(
                        NaiveDate::from_ymd(2022, 6, 30).and_hms(13, 0, 0)
                    )
                }
            ),

            RegistrationPeriod::General(
                Period {
                    start: stine_naive_to_utc(
                        NaiveDate::from_ymd(2022, 9, 1).and_hms(9, 0, 0)
                    ),
                    end: stine_naive_to_utc(
                        NaiveDate::from_ymd(2022, 9, 22).and_hms(13, 0, 0)
                    )
                }
            ),

            RegistrationPeriod::Late(
                Period {
                    start: stine_naive_to_utc(
                        NaiveDate::from_ymd(2022, 10, 4).and_hms(9, 0, 0)
                    ),
                    end: stine_naive_to_utc(
                        NaiveDate::from_ymd(2022, 10, 6).and_hms(13, 0, 0)
                    )
                }
            ),

            RegistrationPeriod::FirstSemester(
                Period {
                    start: stine_naive_to_utc(
                        NaiveDate::from_ymd(2022, 10, 10).and_hms(9, 0, 0)
                    ),
                    end: stine_naive_to_utc(
                        NaiveDate::from_ymd(2022, 10, 13).and_hms(16, 0, 0)
                    )
                }
            ),

            RegistrationPeriod::ChangesAndCorrections(
                Period {
                    start: stine_naive_to_utc(
                        NaiveDate::from_ymd(2022, 10, 17).and_hms(9, 0, 0)
                    ),
                    end: stine_naive_to_utc(
                        NaiveDate::from_ymd(2022, 10, 27).and_hms(13, 0, 0)
                    )
                }
            ),

        ], periods);

    }
}