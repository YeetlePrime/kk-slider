use reqwest::StatusCode;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Error(String),

    #[error("Could not locate element \"{0}\"")]
    MissingElement(String),

    #[error("The generated selector \"{0}\" is invalid")]
    InvalidSelector(String),

    #[error("No number could be parsed from String \"{0}\"")]
    CouldNotParseNumber(String),

    #[error("Failed to send the request")]
    RequestError(#[from] reqwest::Error),

    #[error("ERROR: {0:?} [{1}]")]
    ResponseStatusError(StatusCode, String),

    #[error("Could not create directory.")]
    FileError(#[from] std::io::Error),

    #[error("Could not parse to json")]
    JsonError(#[from] serde_json::Error),

    #[error("No url for {0} was found")]
    MissingUrl(String),


}