use std::fmt::{Display, Formatter};
use std::str::FromStr;
use anyhow::anyhow;
use either::{Either, Left, Right};
use serde::{Deserialize, Serialize};
use crate::CourseResult;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemesterResult {
    pub semester: Semester,
    pub courses: Vec<CourseResult>,
    pub semester_gpa: Result<f32, String>,
    pub semester_credits: String,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SemesterType {
    SummerSemester,
    WinterSemester,
}

impl Display for SemesterType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::SummerSemester => write!(f, "SuSe"),
            Self::WinterSemester => write!(f, "WiSe"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Semester {
    pub season: SemesterType,
    pub year: Either<i32, (i32, i32)>,
}

impl Semester {
    pub const fn new(season: SemesterType, year: Either<i32, (i32, i32)>) -> Self {
        Semester {
            season,
            year
        }
    }

    pub const fn new_summer(year: i32) -> Self {
        Semester {
            season: SemesterType::SummerSemester,
            year: Either::Left(year)
        }
    }

    pub const fn new_winter(year1: i32, year2: i32) -> Self {
        Semester {
            season: SemesterType::WinterSemester,
            year: Either::Right((year1, year2))
        }
    }
}

impl Display for Semester {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let year: String = match &self.year {
            Left(year) => year.to_string(),
            Right((year1, year2)) => format!("{year1}/{year2}"),
        };

        write!(f, "{} {}", self.season, year)
    }
}

impl FromStr for Semester {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {

        let mut split = s.split_whitespace();
        if let Some(season) = split.next() {

            let season = match season.to_lowercase().as_str() {
                "suse" | "sose" => SemesterType::SummerSemester,
                "wise" => SemesterType::WinterSemester,
                _ => return Err(anyhow!("Invalid semester type: Valid: ['suse', 'sose', 'wise']")),
            };

            if let Some(year) = split.next() {
                let either_year: Either<i32, (i32, i32)> = if !year.contains('/') {
                    Left(year.parse()?)
                } else {
                    let mut split = year.split_terminator('/');
                    let first: i32 = split.next().ok_or_else(|| anyhow!("Missing first year for '/' terminator"))?.parse()?;
                    let second: i32 = split.next().ok_or_else(|| anyhow!("Missing second year for '/' terminator"))?.parse()?;

                    Right((first, second))
                };

                return Ok(Self {
                    season,
                    year: either_year,
                })
            }

            return Err(anyhow!("Missing year in semester string"))
        }

        Err(anyhow!("Failed parsing Semester string"))
    }
}