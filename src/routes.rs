use crate::AppState;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use maud::Markup;
use serde::Deserialize;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, Result};
use crate::views;

#[derive(Deserialize)]
pub struct UuidParam {
    uuid: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct CourseParams {
    uuid: Option<Uuid>,
    assignment: Option<Uuid>,
}

// Missing parameters select a default; explicit unknown IDs are stale/invalid links.
fn select(
    requested: Option<Uuid>,
    mut available: impl Iterator<Item = Uuid>,
) -> Result<Option<Uuid>> {
    match requested {
        Some(uuid) if available.any(|id| id == uuid) => Ok(Some(uuid)),
        Some(_) => Err(AppError::NotFound),
        None => Ok(available.next()),
    }
}

// The two stylesheets are compiled in, so the binary runs from anywhere.
fn css(body: &'static str) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], body)
}

pub async fn pico_css() -> impl IntoResponse {
    css(include_str!("../static/pico.min.css"))
}

pub async fn app_css() -> impl IntoResponse {
    css(include_str!("../static/app.css"))
}

pub async fn not_found() -> AppError {
    AppError::NotFound
}

pub async fn overview(State(state): State<AppState>) -> Result<Markup> {
    Ok(views::overview(&db::courses::overview(&state.pool).await?))
}

pub async fn assignments(State(state): State<AppState>) -> Result<Markup> {
    Ok(views::assignments(&db::uploads::status(&state.pool).await?))
}

pub async fn quizzes(State(state): State<AppState>, Query(p): Query<UuidParam>) -> Result<Markup> {
    let (stats, list) = tokio::try_join!(
        db::quizzes::stats(&state.pool),
        db::quizzes::list(&state.pool)
    )?;
    let default = list.iter().max_by_key(|q| q.attempts).map(|q| q.uuid);
    let selected = select(p.uuid.or(default), list.iter().map(|q| q.uuid))?;
    let items = match selected {
        Some(q) => db::quizzes::item_analysis(&state.pool, q).await?,
        None => Vec::new(),
    };
    Ok(views::quizzes(&stats, &list, selected, &items))
}

pub async fn forums(State(state): State<AppState>) -> Result<Markup> {
    Ok(views::forums(&db::forums::activity(&state.pool).await?))
}

pub async fn notifications(State(state): State<AppState>) -> Result<Markup> {
    Ok(views::notifications(
        &db::notifications::unread(&state.pool).await?,
    ))
}

pub async fn student(State(state): State<AppState>, Query(p): Query<UuidParam>) -> Result<Markup> {
    let list = db::users::students(&state.pool).await?;
    let Some(selected) = select(p.uuid, list.iter().map(|s| s.uuid))? else {
        return Ok(views::empty(
            "/student",
            "Studentenprofil",
            "Keine Studierenden in der Datenbank.",
        ));
    };
    let (materials, grades, results, ranking) = tokio::try_join!(
        db::courses::materials(&state.pool, selected),
        db::uploads::grades(&state.pool, selected),
        db::quizzes::results(&state.pool, selected),
        db::quizzes::ranking(&state.pool, selected),
    )?;
    Ok(views::student(
        &list, selected, &materials, &grades, &results, &ranking,
    ))
}

pub async fn course(
    State(state): State<AppState>,
    Query(p): Query<CourseParams>,
) -> Result<Markup> {
    let courses = db::courses::list(&state.pool).await?;
    let Some(selected) = select(p.uuid, courses.iter().map(|c| c.uuid))? else {
        return Ok(views::empty(
            "/course",
            "Kursdetails",
            "Keine Kurse in der Datenbank.",
        ));
    };
    let (members, most_active, uploads) = tokio::try_join!(
        db::courses::members(&state.pool, selected),
        db::courses::most_active(&state.pool, selected),
        db::uploads::list(&state.pool, selected),
    )?;
    let upload = select(p.assignment, uploads.iter().map(|u| u.uuid))?;
    let (missing, times) = match upload {
        Some(a) => {
            let (m, t) = tokio::try_join!(
                db::uploads::missing_submissions(&state.pool, a),
                db::uploads::submission_times(&state.pool, a)
            )?;
            (Some(m), t)
        }
        None => (None, Vec::new()),
    };
    Ok(views::course(views::CoursePage {
        courses: &courses,
        selected,
        members: &members,
        most_active: &most_active,
        uploads: &uploads,
        upload,
        missing: missing.as_deref(),
        times: &times,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, response::IntoResponse};

    #[test]
    fn selection_distinguishes_defaults_from_invalid_links() {
        let valid = Uuid::from_u128(1);
        let missing = Uuid::from_u128(2);
        assert_eq!(select(None, [valid].into_iter()).ok(), Some(Some(valid)));
        assert_eq!(
            select(Some(valid), [valid].into_iter()).ok(),
            Some(Some(valid))
        );
        assert_eq!(select(None, [].into_iter()).ok(), Some(None));
        for ids in [vec![valid], vec![]] {
            let Err(error) = select(Some(missing), ids.into_iter()) else {
                panic!("unknown IDs must fail, even when no records exist");
            };
            assert_eq!(error.into_response().status(), StatusCode::NOT_FOUND);
        }
    }
}
