use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Stine - Document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub datetime: DateTime<Utc>,
    pub status: Option<String>,
    pub download: String,
}

impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.datetime == other.datetime
            && self.status == other.status
    }
}

impl ToString for Document {
    fn to_string(&self) -> String {
        format!("{}: {}. Status: {:#?}. Download-Link: {}",
                self.name, self.datetime, self.status, self.download)
    }
}
