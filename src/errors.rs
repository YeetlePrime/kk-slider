#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("Could not locate element \"{0}\"")]
    MissingElement(String),

    #[error("The generated selector \"{0}\" is invalid")]
    InvalidSelector(String),

    #[error("No number could be parsed from String \"{0}\"")]
    CouldNotParseNumber(String),
}
