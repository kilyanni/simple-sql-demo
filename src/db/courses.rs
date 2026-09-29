use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;
use sqlx::PgExecutor;
use uuid::Uuid;

pub struct Course {
    pub uuid: Uuid,
    pub title: String,
    pub start_date: NaiveDate,
    pub max_capacity: Option<i32>,
}

/// All courses, newest first, for the course selector.
pub async fn list(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<Course>> {
    sqlx::query_as!(
        Course,
        "SELECT uuid, title, start_date, max_capacity FROM course ORDER BY start_date DESC, title"
    )
    .fetch_all(db)
    .await
}

pub struct CourseOverview {
    pub course: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub active_members: i64,
    pub sections: i64,
    pub files: i64,
    pub assignments: i64,
    pub quizzes: i64,
    pub forums: i64,
}

/// Per-course counts from the view `v_course_overview`.
pub async fn overview(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<CourseOverview>> {
    sqlx::query_as!(
        CourseOverview,
        r#"
SELECT
    course AS "course!",
    start_date AS "start_date!",
    end_date,
    active_members AS "active_members!",
    sections AS "sections!",
    files AS "files!",
    assignments AS "assignments!",
    quizzes AS "quizzes!",
    forums AS "forums!"
FROM v_course_overview
ORDER BY start_date DESC, course ASC
"#
    )
    .fetch_all(db)
    .await
}

pub struct Material {
    pub course: String,
    pub section: String,
    pub file: String,
    pub r#type: String,
}

/// Visible files in the active courses of one student.
pub async fn materials(db: impl PgExecutor<'_>, student: Uuid) -> sqlx::Result<Vec<Material>> {
    sqlx::query_as!(
        Material,
        r#"
SELECT
    c.title AS course,
    cs.title AS section,
    f.title AS file,
    f.type
FROM "user" AS u
INNER JOIN course_member AS cm ON u.uuid = cm.user_uuid
INNER JOIN course AS c ON cm.course_uuid = c.uuid
INNER JOIN course_section AS cs ON c.uuid = cs.course_uuid
INNER JOIN file AS f ON cs.uuid = f.section_uuid
WHERE
    u.uuid = $1
    AND cm.status = 'active'
    AND cs.visibility = 'visible'
ORDER BY c.title, cs.position, f.title
"#,
        student
    )
    .fetch_all(db)
    .await
}

pub struct Member {
    pub full_name: String,
    pub email: String,
    pub roles: Option<String>,
    pub joined_at: NaiveDateTime,
    pub status: String,
}

/// Members of one course with their roles.
pub async fn members(db: impl PgExecutor<'_>, course: Uuid) -> sqlx::Result<Vec<Member>> {
    sqlx::query_as!(
        Member,
        r#"
SELECT
    u.full_name,
    u.email,
    cm.joined_at,
    cm.status,
    STRING_AGG(r.name, ', ' ORDER BY r.name) AS roles
FROM course_member AS cm
INNER JOIN "user" AS u ON cm.user_uuid = u.uuid
LEFT JOIN role_assignment AS ra
    ON
        u.uuid = ra.user_uuid
        AND cm.course_uuid = ra.course_uuid
LEFT JOIN role AS r ON ra.role_uuid = r.uuid
WHERE cm.course_uuid = $1
GROUP BY cm.user_uuid, cm.course_uuid, u.uuid
ORDER BY u.full_name
"#,
        course
    )
    .fetch_all(db)
    .await
}

pub struct Activity {
    pub full_name: String,
    pub submissions: i64,
    pub avg_points: Option<Decimal>,
}

/// Members of one course ranked by submission count and average grade.
pub async fn most_active(db: impl PgExecutor<'_>, course: Uuid) -> sqlx::Result<Vec<Activity>> {
    sqlx::query_as!(
        Activity,
        r#"
SELECT
    u.full_name,
    COUNT(DISTINCT s.uuid) AS "submissions!",
    ROUND(AVG(g.points), 2) AS avg_points
FROM "user" AS u
INNER JOIN submission AS s ON u.uuid = s.submitter_uuid
INNER JOIN file_upload AS fu ON s.file_upload_uuid = fu.uuid
INNER JOIN task AS t ON fu.task_uuid = t.uuid
INNER JOIN course_section AS cs ON t.course_section_uuid = cs.uuid
LEFT JOIN grade AS g ON s.uuid = g.submission_uuid
WHERE
    cs.course_uuid = $1
    AND u.uuid IN (
        SELECT user_uuid FROM course_member
        WHERE course_uuid = $1
    )
GROUP BY u.uuid
ORDER BY COUNT(DISTINCT s.uuid) DESC, avg_points DESC NULLS LAST
"#,
        course
    )
    .fetch_all(db)
    .await
}
