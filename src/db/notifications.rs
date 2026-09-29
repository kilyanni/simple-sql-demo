use chrono::NaiveDateTime;
use sqlx::PgExecutor;

pub struct UnreadNotifications {
    pub full_name: String,
    pub email: String,
    pub unread: i64,
    pub oldest_unread: NaiveDateTime,
}

/// Unread notification count per active user.
pub async fn unread(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<UnreadNotifications>> {
    sqlx::query_as!(
        UnreadNotifications,
        r#"
SELECT
    u.full_name,
    u.email,
    COUNT(n.uuid) AS "unread!",
    MIN(n.created_at) AS "oldest_unread!"
FROM "user" AS u
INNER JOIN notification AS n ON u.uuid = n.recipient_uuid
WHERE
    n.read_at IS NULL
    AND u.uuid IN (
        SELECT DISTINCT user_uuid FROM course_member
        WHERE status = 'active'
    )
GROUP BY u.uuid
ORDER BY COUNT(n.uuid) DESC
"#
    )
    .fetch_all(db)
    .await
}
