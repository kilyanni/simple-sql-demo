use chrono::NaiveDateTime;
use rust_decimal::Decimal;
use sqlx::PgExecutor;
use uuid::Uuid;

use super::users::Person;

pub struct Upload {
    pub uuid: Uuid,
    pub title: String,
}

/// File upload assignments of one course, for the assignment selector.
pub async fn list(db: impl PgExecutor<'_>, course: Uuid) -> sqlx::Result<Vec<Upload>> {
    sqlx::query_as!(
        Upload,
        r#"
SELECT
    fu.uuid,
    fu.title
FROM file_upload AS fu
INNER JOIN task AS t ON fu.task_uuid = t.uuid
INNER JOIN course_section AS cs ON t.course_section_uuid = cs.uuid
WHERE cs.course_uuid = $1
ORDER BY fu.title
"#,
        course
    )
    .fetch_all(db)
    .await
}

pub struct UploadStatus {
    pub course: String,
    pub assignment: String,
    pub max_points: Decimal,
    pub submitted: i64,
    pub graded: i64,
    pub average: Option<Decimal>,
}

/// Submission count, graded count and average points per assignment.
pub async fn status(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<UploadStatus>> {
    sqlx::query_as!(
        UploadStatus,
        r#"
SELECT
    c.title AS course,
    fu.title AS assignment,
    fu.max_points,
    COUNT(s.uuid) AS "submitted!",
    COUNT(g.uuid) AS "graded!",
    ROUND(AVG(g.points), 2) AS average
FROM file_upload AS fu
INNER JOIN task AS t ON fu.task_uuid = t.uuid
INNER JOIN course_section AS cs ON t.course_section_uuid = cs.uuid
INNER JOIN course AS c ON cs.course_uuid = c.uuid
LEFT JOIN submission AS s ON fu.uuid = s.file_upload_uuid
LEFT JOIN grade AS g ON s.uuid = g.submission_uuid
GROUP BY c.uuid, fu.uuid
ORDER BY c.title, fu.due_date NULLS LAST
"#
    )
    .fetch_all(db)
    .await
}

/// Students of the assignment's course without an own or group submission.
pub async fn missing_submissions(
    db: impl PgExecutor<'_>,
    file_upload: Uuid,
) -> sqlx::Result<Vec<Person>> {
    sqlx::query_as!(
        Person,
        r#"
SELECT
    u.full_name,
    u.email
FROM file_upload AS fu
INNER JOIN task AS t ON fu.task_uuid = t.uuid
INNER JOIN course_section AS cs ON t.course_section_uuid = cs.uuid
INNER JOIN v_course_student AS st ON cs.course_uuid = st.course_uuid
INNER JOIN "user" AS u ON st.user_uuid = u.uuid
WHERE
    fu.uuid = $1
    AND st.status = 'active'
    AND NOT EXISTS (
        SELECT 1 FROM v_submission_member AS sm
        WHERE sm.file_upload_uuid = fu.uuid AND sm.user_uuid = u.uuid
    )
ORDER BY u.full_name
"#,
        file_upload
    )
    .fetch_all(db)
    .await
}

pub struct GradedSubmission {
    pub course: String,
    pub assignment: String,
    pub points: Decimal,
    pub max_points: Decimal,
    pub percent: Option<Decimal>,
    pub feedback: Option<String>,
    pub graded_at: NaiveDateTime,
}

/// Graded submissions of one student, including group submissions.
pub async fn grades(db: impl PgExecutor<'_>, student: Uuid) -> sqlx::Result<Vec<GradedSubmission>> {
    sqlx::query_as!(
        GradedSubmission,
        r#"
SELECT
    c.title AS course,
    fu.title AS assignment,
    g.points,
    fu.max_points,
    g.feedback,
    g.graded_at,
    ROUND(g.points / NULLIF(fu.max_points, 0) * 100, 1) AS percent
FROM v_submission_member AS sm
INNER JOIN submission AS s ON sm.submission_uuid = s.uuid
INNER JOIN file_upload AS fu ON s.file_upload_uuid = fu.uuid
INNER JOIN task AS t ON fu.task_uuid = t.uuid
INNER JOIN course_section AS cs ON t.course_section_uuid = cs.uuid
INNER JOIN course AS c ON cs.course_uuid = c.uuid
INNER JOIN grade AS g ON s.uuid = g.submission_uuid
WHERE sm.user_uuid = $1
ORDER BY g.graded_at DESC
"#,
        student
    )
    .fetch_all(db)
    .await
}

pub struct SubmissionTime {
    pub submitted_at: NaiveDateTime,
    pub due_date: Option<NaiveDateTime>,
}

impl SubmissionTime {
    /// Hours after the due date, negative when early; `None` without a due date.
    pub fn hours_after_due(&self) -> Option<f64> {
        let due = self.due_date?;
        Some((self.submitted_at - due).num_seconds() as f64 / 3600.0)
    }
}

/// Submission times of one assignment with its due date, for the timeline chart.
pub async fn submission_times(
    db: impl PgExecutor<'_>,
    file_upload: Uuid,
) -> sqlx::Result<Vec<SubmissionTime>> {
    sqlx::query_as!(
        SubmissionTime,
        r#"
SELECT
    s.submitted_at,
    fu.due_date
FROM submission AS s
INNER JOIN file_upload AS fu ON s.file_upload_uuid = fu.uuid
WHERE fu.uuid = $1
ORDER BY s.submitted_at
"#,
        file_upload
    )
    .fetch_all(db)
    .await
}
