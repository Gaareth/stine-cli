use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Language {
    German,
    English,
}

impl FromStr for Language {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "en" => Ok(Self::English),
            "de" => Ok(Self::German),
            _ => Err(()),
        }
    }
}

impl ToString for Language {
    fn to_string(&self) -> String {
        match self {
            Self::English => "en",
            Self::German => "de",
        }.to_string()
    }
}