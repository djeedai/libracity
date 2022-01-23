#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Error {
    LoadLevels,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::LoadLevels
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::LoadLevels
    }
}
