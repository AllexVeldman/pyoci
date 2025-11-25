use axum::response::IntoResponse;
use http::StatusCode;

#[derive(Debug, PartialEq, Eq)]
pub struct PyOciError {
    pub status: StatusCode,
    pub message: String,
}

impl std::error::Error for PyOciError {}

impl std::fmt::Display for PyOciError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.status, self.message)
    }
}

impl IntoResponse for PyOciError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
    }
}

impl From<(StatusCode, &str)> for PyOciError {
    fn from((status, message): (StatusCode, &str)) -> Self {
        PyOciError {
            status,
            message: message.to_string(),
        }
    }
}

impl From<(StatusCode, String)> for PyOciError {
    fn from((status, message): (StatusCode, String)) -> Self {
        PyOciError { status, message }
    }
}
