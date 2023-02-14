use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::parse::date::parse_period;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Clone)]
pub struct Period {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl ToString for Period {
    fn to_string(&self) -> String {
        format!("{} - {}",
                self.start.format("%Y-%m-%d %H:%M:%S"),
                self.end.format("%Y-%m-%d %H:%M:%S"))
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Clone)]
pub enum RegistrationPeriod {
    Early(Period),
    General(Period),
    Late(Period),
    FirstSemester(Period),
    ChangesAndCorrections(Period),
}


#[derive(Error, Debug)]
pub enum PeriodParseError {
    #[error("Missing split ' - ' or ' to ' pattern")]
    MissingSplitSymbol,
    #[error("Invalid period type string")]
    /// Valid types: "Vorgezogene Phase" | "Early registration period" ,
    ///            "Anmeldephase" | "General registration period",
    ///             "Nachmeldephase" | "Late registration period" ,
    ///             "Erstsemester" | "Registration period for first-semester students",
    ///             "Ummelde- und Korrektur-Phase" | "Changes and corrections period",
    InvalidPeriodType,

    #[error("Invalid Date")]
    /// Valid formats: Eng: "%a,%e %B %Y,%l:%M %P", Ger: ""%a, %d.%m.%y, %H:%M Uhr"
    InvalidDate(#[from] anyhow::Error),
}

impl RegistrationPeriod {
    pub(crate) fn parse(period_type: &str, period: &str) -> Result<Self, PeriodParseError> {
        let period = parse_period(period)?;

        match period_type {
            "Vorgezogene Phase" | "Early registration period" => Ok(RegistrationPeriod::Early(period)),
            "Anmeldephase" | "General registration period" => Ok(RegistrationPeriod::General(period)),
            "Nachmeldephase" | "Late registration period" => Ok(RegistrationPeriod::Late(period)),
            "Erstsemester" | "Registration period for first-semester students" => Ok(RegistrationPeriod::FirstSemester(period)),
            "Ummelde- und Korrektur-Phase" | "Changes and corrections period" => Ok(RegistrationPeriod::ChangesAndCorrections(period)),
            _ => Err(PeriodParseError::InvalidPeriodType),
        }
    }

    pub fn name(&self) -> String {
        match self {
            RegistrationPeriod::Early(_) => "Early registration period",
            RegistrationPeriod::General(_) => "General registration period",
            RegistrationPeriod::Late(_) => "Late registration period",
            RegistrationPeriod::FirstSemester(_) => "Registration period for first-semester students",
            RegistrationPeriod::ChangesAndCorrections(_) => "Changes and corrections period",
        }.to_string()
    }

    pub const fn period(&self) -> &Period {
        match self {
            RegistrationPeriod::Early(period)
            | RegistrationPeriod::General(period)
            | RegistrationPeriod::Late(period)
            | RegistrationPeriod::FirstSemester(period)
            | RegistrationPeriod::ChangesAndCorrections(period) => period,
        }
    }
}
