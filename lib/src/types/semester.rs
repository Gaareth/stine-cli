use std::fmt::{Display, Formatter};
use std::str::FromStr;

use anyhow::anyhow;
use either::{Either, Left, Right};
use regex::Regex;
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
            year,
        }
    }

    pub const fn new_summer(year: i32) -> Self {
        Semester {
            season: SemesterType::SummerSemester,
            year: Left(year),
        }
    }

    pub const fn new_winter(year1: i32, year2: i32) -> Self {
        Semester {
            season: SemesterType::WinterSemester,
            year: Right((year1, year2)),
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
        // Simple regex = (wise|suse|sose)\s?(\d\d)(/\d\d)?
        let s = s.to_lowercase();
        let re = Regex::new(r"(wise|suse|sose)\s?((\d\d)(?:/(\d\d))?)").unwrap();
        let caps = re.captures(&s)
            .ok_or_else(|| anyhow!("'{s}' invalid semester string.\
            Use the following format: '(wise|suse|sose)\\s?(\\d\\d)(/\\d\\d)?'"))?;
        let sem_type = caps.get(1).unwrap().as_str();
        let sem_year = caps.get(2).unwrap().as_str();

        let season = match sem_type.trim() {
            "suse" | "sose" => SemesterType::SummerSemester,
            "wise" => SemesterType::WinterSemester,
            _ => return Err(anyhow!("Invalid semester type: Valid (case insensitive): ['suse', 'sose', 'wise']")),
        };
        let either_year: Either<i32, (i32, i32)> = if !sem_year.contains('/') {
            Left(sem_year.parse()?)
        } else {
            let first = caps.get(3).unwrap().as_str().parse().unwrap();
            let second = caps.get(4).unwrap().as_str().parse().unwrap();
            Right((first, second))
        };

        Ok(Self {
            season,
            year: either_year,
        })

    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::Semester;
    use crate::SemesterType::{SummerSemester, WinterSemester};

    #[test]
    pub fn test_parse_semester_string() {
        let s = Semester::from_str("wise 22/23").unwrap();
        assert_eq!(s.season, WinterSemester);
        assert_eq!(s.year.right().unwrap(), (22, 23));

         let s = Semester::from_str("wise22/23").unwrap();
        assert_eq!(s.season, WinterSemester);
        assert_eq!(s.year.right().unwrap(), (22, 23));

        let s = Semester::from_str("SoSe 22").unwrap();
        assert_eq!(s.season, SummerSemester);
        assert_eq!(s.year.left().unwrap(), 22);

        let s = Semester::from_str("SUSe 20/23").unwrap();
        assert_eq!(s.season, SummerSemester);
        assert_eq!(s.year.right().unwrap(), (20, 23));
    }

    #[test]
    pub fn test_parse_semester_string_invalid() {
        Semester::from_str("wise").unwrap_err();
        Semester::from_str("wiso 22/23").unwrap_err();
    }
}