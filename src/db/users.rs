use sqlx::PgExecutor;
use uuid::Uuid;

pub struct Student {
    pub uuid: Uuid,
    pub full_name: String,
    pub email: String,
}

pub struct Person {
    pub full_name: String,
    pub email: String,
}

/// Users with the role Studierende in a course they are a member of, for the student selector.
pub async fn students(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<Student>> {
    sqlx::query_as!(
        Student,
        r#"
SELECT DISTINCT
    u.uuid,
    u.full_name,
    u.email
FROM "user" AS u
INNER JOIN v_course_student AS st ON u.uuid = st.user_uuid
ORDER BY u.full_name
"#
    )
    .fetch_all(db)
    .await
}
