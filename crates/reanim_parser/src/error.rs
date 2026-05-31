use std::fmt;

#[derive(Debug)]
pub enum ReanimError {
    Io(std::io::Error),
    Xml(quick_xml::Error),
    InvalidXml(String),
}

impl fmt::Display for ReanimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Xml(e) => write!(f, "XML error: {e}"),
            Self::InvalidXml(s) => write!(f, "Invalid XML: {s}"),
        }
    }
}

impl std::error::Error for ReanimError {}

impl From<std::io::Error> for ReanimError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<quick_xml::Error> for ReanimError {
    fn from(e: quick_xml::Error) -> Self {
        Self::Xml(e)
    }
}

pub type Result<T> = std::result::Result<T, ReanimError>;
