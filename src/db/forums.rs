use chrono::NaiveDateTime;
use sqlx::PgExecutor;

pub struct ForumActivity {
    pub course: String,
    pub forum: String,
    pub threads: i64,
    pub posts: i64,
    pub authors: i64,
    pub last_post: Option<NaiveDateTime>,
}

/// Thread, post and author counts per forum with the time of the last post.
pub async fn activity(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<ForumActivity>> {
    sqlx::query_as!(
        ForumActivity,
        r#"
SELECT
    c.title AS course,
    fo.title AS forum,
    COUNT(DISTINCT d.uuid) AS "threads!",
    COUNT(p.uuid) AS "posts!",
    COUNT(DISTINCT p.author_uuid) AS "authors!",
    MAX(p.created_at) AS last_post
FROM course AS c
INNER JOIN course_section AS cs ON c.uuid = cs.course_uuid
INNER JOIN forum AS fo ON cs.uuid = fo.course_section_uuid
LEFT JOIN discussion AS d ON fo.uuid = d.forum_uuid
LEFT JOIN post AS p ON d.uuid = p.discussion_uuid
GROUP BY c.uuid, fo.uuid
ORDER BY COUNT(p.uuid) DESC
"#
    )
    .fetch_all(db)
    .await
}
