use chrono::NaiveDateTime;
use rust_decimal::Decimal;
use sqlx::PgExecutor;
use uuid::Uuid;

fn as_f64(value: Decimal) -> f64 {
    value.try_into().unwrap_or(0.0)
}

pub struct QuizRef {
    pub uuid: Uuid,
    pub title: String,
    pub course: String,
    pub attempts: i64,
}

/// All quizzes with their submitted attempt count, for the quiz selector.
pub async fn list(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<QuizRef>> {
    sqlx::query_as!(
        QuizRef,
        r#"
SELECT
    q.uuid,
    q.title,
    c.title AS course,
    COUNT(qa.uuid) AS "attempts!"
FROM quiz AS q
INNER JOIN task AS t ON q.task_uuid = t.uuid
INNER JOIN course_section AS cs ON t.course_section_uuid = cs.uuid
INNER JOIN course AS c ON cs.course_uuid = c.uuid
LEFT JOIN quiz_attempt AS qa ON q.uuid = qa.quiz_uuid AND qa.submitted_at IS NOT NULL
GROUP BY q.uuid, c.uuid
ORDER BY c.title, q.title
"#
    )
    .fetch_all(db)
    .await
}

pub struct QuizStats {
    pub quiz: String,
    pub course: String,
    pub grading_method: String,
    pub attempts: i64,
    pub students: i64,
    pub avg_points: Option<Decimal>,
    pub pending: i64,
}

/// Attempt count, student count, average score per submitted attempt, and how
/// many free-text answers still await marking.
pub async fn stats(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<QuizStats>> {
    sqlx::query_as!(
        QuizStats,
        r#"
SELECT
    q.title AS quiz,
    c.title AS course,
    q.grading_method,
    COUNT(ap.attempt_uuid) AS "attempts!",
    COUNT(DISTINCT ap.student_uuid) AS "students!",
    ROUND(AVG(ap.points), 2) AS avg_points,
    (
        SELECT COUNT(*)
        FROM quiz_answer_text AS qat
        INNER JOIN quiz_attempt AS qa ON qat.attempt_uuid = qa.uuid
        WHERE
            qa.quiz_uuid = q.uuid
            AND qa.submitted_at IS NOT NULL
            AND NOT EXISTS (
                SELECT 1 FROM grade AS g
                WHERE
                    g.attempt_uuid = qat.attempt_uuid
                    AND g.question_uuid = qat.question_uuid
            )
    ) AS "pending!"
FROM quiz AS q
INNER JOIN task AS t ON q.task_uuid = t.uuid
INNER JOIN course_section AS cs ON t.course_section_uuid = cs.uuid
INNER JOIN course AS c ON cs.course_uuid = c.uuid
LEFT JOIN v_quiz_attempt_points AS ap ON q.uuid = ap.quiz_uuid
GROUP BY q.uuid, c.uuid
ORDER BY c.title, q.title
"#
    )
    .fetch_all(db)
    .await
}

pub struct QuizResult {
    pub quiz: String,
    pub course: String,
    pub started_at: NaiveDateTime,
    pub submitted_at: Option<NaiveDateTime>,
    pub points: Option<Decimal>,
    pub max_points: Option<Decimal>,
}

/// Attempts of one student with their score and the quiz maximum.
pub async fn results(db: impl PgExecutor<'_>, student: Uuid) -> sqlx::Result<Vec<QuizResult>> {
    sqlx::query_as!(
        QuizResult,
        r#"
SELECT
    q.title AS quiz,
    c.title AS course,
    qa.started_at,
    qa.submitted_at,
    ap.points,
    (
        SELECT SUM(points) FROM quiz_question
        WHERE quiz_uuid = q.uuid
    ) AS max_points
FROM quiz_attempt AS qa
INNER JOIN quiz AS q ON qa.quiz_uuid = q.uuid
INNER JOIN task AS t ON q.task_uuid = t.uuid
INNER JOIN course_section AS cs ON t.course_section_uuid = cs.uuid
INNER JOIN course AS c ON cs.course_uuid = c.uuid
LEFT JOIN v_quiz_attempt_points AS ap ON qa.uuid = ap.attempt_uuid
WHERE qa.student_uuid = $1
ORDER BY qa.started_at DESC
"#,
        student
    )
    .fetch_all(db)
    .await
}

pub struct ItemCell {
    pub position: i32,
    pub question: String,
    pub option_text: String,
    pub fraction: Decimal,
    pub chosen: i64,
    pub attempts: i64,
}

impl ItemCell {
    /// Percentage of submitted attempts that chose this option.
    pub fn share(&self) -> f64 {
        if self.attempts > 0 {
            self.chosen as f64 / self.attempts as f64 * 100.0
        } else {
            0.0
        }
    }
}

/// How often each answer option of one quiz was chosen in submitted attempts.
pub async fn item_analysis(db: impl PgExecutor<'_>, quiz: Uuid) -> sqlx::Result<Vec<ItemCell>> {
    sqlx::query_as!(
        ItemCell,
        r#"
SELECT
    qq.position,
    q.body AS question,
    qo.body AS option_text,
    qo.fraction,
    COUNT(qao.attempt_uuid) AS "chosen!",
    (
        SELECT COUNT(*) FROM quiz_attempt
        WHERE quiz_uuid = $1 AND submitted_at IS NOT NULL
    ) AS "attempts!"
FROM quiz_question AS qq
INNER JOIN question AS q ON qq.question_uuid = q.uuid
INNER JOIN question_option AS qo ON q.uuid = qo.question_uuid
LEFT JOIN quiz_answer_option AS qao
    ON
        qo.uuid = qao.selected_option_uuid
        AND qao.attempt_uuid IN (
            SELECT uuid FROM quiz_attempt
            WHERE quiz_uuid = $1 AND submitted_at IS NOT NULL
        )
WHERE qq.quiz_uuid = $1
GROUP BY qq.quiz_uuid, qq.question_uuid, q.uuid, qo.uuid
ORDER BY qq.position, qo.position
"#,
        quiz
    )
    .fetch_all(db)
    .await
}

struct RankRow {
    quiz_uuid: Uuid,
    quiz: String,
    grading_method: String,
    student_uuid: Uuid,
    full_name: String,
    points: Decimal,
    max_points: Option<Decimal>,
    rank: i64,
    participants: i64,
}

/// One quiz the student submitted: every participant's score, and the student's rank.
pub struct QuizRanking {
    pub quiz: String,
    pub grading_method: String,
    pub max_points: Option<Decimal>,
    pub rank: i64,
    pub participants: i64,
    pub points: Decimal,
    pub selected: usize,
    pub entries: Vec<(String, f64)>,
}

/// Score of every student in each quiz the given student submitted, with competition
/// ranks. Scores follow the quiz's grading method, applied by `v_quiz_student_points`.
pub async fn ranking(db: impl PgExecutor<'_>, student: Uuid) -> sqlx::Result<Vec<QuizRanking>> {
    let rows = sqlx::query_as!(
        RankRow,
        r#"
SELECT
    q.uuid AS quiz_uuid,
    q.title AS quiz,
    q.grading_method,
    u.uuid AS student_uuid,
    u.full_name,
    sp.points AS "points!",
    (
        SELECT SUM(points) FROM quiz_question
        WHERE quiz_uuid = q.uuid
    ) AS max_points,
    RANK() OVER (PARTITION BY q.uuid ORDER BY sp.points DESC) AS "rank!",
    COUNT(*) OVER (PARTITION BY q.uuid) AS "participants!"
FROM v_quiz_student_points AS sp
INNER JOIN quiz AS q ON sp.quiz_uuid = q.uuid
INNER JOIN "user" AS u ON sp.student_uuid = u.uuid
WHERE
    q.uuid IN (
        SELECT quiz_uuid FROM quiz_attempt
        WHERE student_uuid = $1 AND submitted_at IS NOT NULL
    )
ORDER BY q.title ASC, q.uuid ASC, sp.points DESC, u.full_name ASC, u.uuid ASC
"#,
        student
    )
    .fetch_all(db)
    .await?;
    Ok(group(&rows, student))
}

// Rows arrive sorted by quiz, so each chunk is one quiz.
fn group(rows: &[RankRow], student: Uuid) -> Vec<QuizRanking> {
    rows.chunk_by(|a, b| a.quiz_uuid == b.quiz_uuid)
        .filter_map(|quiz| {
            let selected = quiz.iter().position(|r| r.student_uuid == student)?;
            let me = &quiz[selected];
            Some(QuizRanking {
                quiz: me.quiz.clone(),
                grading_method: me.grading_method.clone(),
                max_points: me.max_points,
                rank: me.rank,
                participants: me.participants,
                points: me.points,
                selected,
                entries: quiz
                    .iter()
                    .map(|r| (r.full_name.clone(), as_f64(r.points)))
                    .collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping_keeps_equal_titles_separate_and_locates_the_student() {
        let selected = Uuid::from_u128(1);
        let row = |quiz, student, points: i64, rank| RankRow {
            quiz_uuid: Uuid::from_u128(quiz),
            quiz: "Quiz 1".into(),
            grading_method: "highest".into(),
            student_uuid: Uuid::from_u128(student),
            full_name: format!("Student {student}"),
            points: Decimal::from(points),
            max_points: Some(Decimal::from(20)),
            rank,
            participants: 2,
        };
        let rows = [
            row(10, 2, 20, 1),
            row(10, 1, 10, 2),
            row(11, 1, 15, 1),
            row(11, 2, 15, 1),
            row(12, 3, 5, 1),
        ];
        let grouped = group(&rows, selected);
        assert_eq!(grouped.len(), 2);
        assert_eq!(
            (grouped[0].rank, grouped[0].selected, grouped[0].points),
            (2, 1, Decimal::from(10))
        );
        assert_eq!((grouped[1].rank, grouped[1].selected), (1, 0));
        assert_eq!(grouped[1].entries.len(), 2);
    }
}
