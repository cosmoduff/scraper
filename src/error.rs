use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum FwPullError {
    VendorParse,
    SoupNotFound(String),
    Reqwest(reqwest::Error),
    IO(std::io::Error),
    Json(serde_json::Error),
    FantocciniCmd(fantoccini::error::CmdError),
}

impl fmt::Display for FwPullError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FwPullError::VendorParse => write!(f, "Unknown vendor"),
            FwPullError::SoupNotFound(i) => write!(f, "Could not find {} in HTML", i),
            FwPullError::Reqwest(e) => write!(f, "{}", e),
            FwPullError::Json(e) => write!(f, "{}", e),
            FwPullError::IO(e) => write!(f, "{}", e),
            FwPullError::Json(e) => write!(f, "{}", e),
            FwPullError::FantocciniCmd(e) => write!(f, "{}", e),
        }
    }
}

impl Error for FwPullError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FwPullError::VendorParse => None,
            FwPullError::SoupNotFound(_e) => None,
            FwPullError::Reqwest(e) => Some(e),
            FwPullError::IO(e) => Some(e),
            FwPullError::Json(e) => Some(e),
            FwPullError::FantocciniCmd(e) => Some(e),
        }
    }
}

impl From<serde_json::Error> for FwPullError {
    fn from(err: serde_json::Error) -> Self {
        FwPullError::Json(err)
    }
}

impl From<std::io::Error> for FwPullError {
    fn from(err: std::io::Error) -> Self {
        FwPullError::IO(err)
    }
}

impl From<fantoccini::error::CmdError> for FwPullError {
    fn from(err: fantoccini::error::CmdError) -> Self {
        FwPullError::FantocciniCmd(err)
    }
}

impl From<reqwest::Error> for FwPullError {
    fn from(err: reqwest::Error) -> Self {
        FwPullError::Reqwest(err)
    }
}
