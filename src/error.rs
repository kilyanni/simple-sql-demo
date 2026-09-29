use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::views;

pub enum AppError {
    Database(sqlx::Error),
    NotFound,
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        Self::Database(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, title, message) = match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "Nicht gefunden",
                "Eintrag nicht gefunden.",
            ),
            Self::Database(error) => {
                eprintln!("Database request failed: {error}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Fehler",
                    "Daten konnten nicht geladen werden.",
                )
            }
        };
        (status, views::empty("", title, message)).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
