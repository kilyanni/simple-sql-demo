//! Opt-in PostgreSQL regression tests. The fixture applies the migration and the
//! small seed to a private schema inside a transaction that is never committed.
use std::sync::atomic::{AtomicU32, Ordering};

use super::{courses, quizzes, uploads, users};
use rust_decimal::Decimal;
use sqlx::{Connection, PgConnection};
use uuid::Uuid;

// Decimal digits become hex digits, matching the literal ids in seeds/seed.sql.
const fn bcd(mut x: u128) -> u128 {
    let mut out = 0;
    let mut shift = 0;
    while x > 0 {
        out |= (x % 10) << shift;
        x /= 10;
        shift += 4;
    }
    out
}

const fn id(kind: u128, n: u128) -> Uuid {
    Uuid::from_u128(bcd(kind) << 48 | bcd(n))
}

// Rows of seeds/seed.sql.
const PROF: Uuid = id(1, 1);
const MAX: Uuid = id(1, 2);
const ANNA: Uuid = id(1, 3);
const JONAS: Uuid = id(1, 4);
const STUDENT_ROLE: Uuid = id(2, 1);
const DBS: Uuid = id(6, 1);
const ALGO: Uuid = id(6, 2);
const ALGO_INTRO: Uuid = id(7, 4);
const UPLOAD: Uuid = id(10, 1);
const ANNA_SUBMISSION: Uuid = id(11, 2);
const QUIZ: Uuid = id(15, 1);
const MAX_ATTEMPT: Uuid = id(16, 1);
const CORRECT_OPTION: Uuid = id(14, 1);
// Rows the tests add.
const TEAM: Uuid = id(20, 1);
const ALLOWED_TEAMS: Uuid = id(21, 1);

async fn fixture() -> PgConnection {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let url = std::env::var("TEST_DATABASE_URL").expect("set TEST_DATABASE_URL");
    let mut db = PgConnection::connect(&url).await.unwrap();
    let schema = format!(
        "dashboard_test_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    // SAFETY: The schema name is not user input, hence we can assert it to be safe.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "BEGIN; CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema};"
    )))
    .execute(&mut db)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20260920000000_initial_schema.sql"
    ))
    .execute(&mut db)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!("../../seeds/seed.sql"))
        .execute(&mut db)
        .await
        .unwrap();
    // The transaction never commits, so deferred constraints must be checked here.
    sqlx::raw_sql("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut db)
        .await
        .unwrap();
    // A refused write aborts the transaction; tests roll back to here to continue.
    sqlx::raw_sql("SAVEPOINT probe")
        .execute(&mut db)
        .await
        .unwrap();
    db
}

async fn try_run(db: &mut PgConnection, sql: &'static str, ids: &[Uuid]) -> sqlx::Result<()> {
    let mut query = sqlx::query(sql);
    for id in ids {
        query = query.bind(*id);
    }
    query.execute(db).await.map(|_| ())
}

async fn run(db: &mut PgConnection, sql: &'static str, ids: &[Uuid]) {
    try_run(db, sql, ids).await.unwrap();
}

async fn missing(db: &mut PgConnection) -> Vec<String> {
    uploads::missing_submissions(db, UPLOAD)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.full_name)
        .collect()
}

async fn anna_grades(db: &mut PgConnection) -> usize {
    uploads::grades(db, ANNA).await.unwrap().len()
}

/// Max scores 10 and Anna 15. Adds a blank submitted attempt by Max and a draft
/// by Jonas that has an answer selected.
async fn add_blank_and_draft(db: &mut PgConnection) {
    run(db, "INSERT INTO quiz_attempt (uuid, quiz_uuid, student_uuid, submitted_at) VALUES ($1, $2, $3, now()), ($4, $2, $5, NULL)",
        &[id(16, 3), QUIZ, MAX, id(16, 4), JONAS]).await;
    run(
        db,
        "INSERT INTO quiz_answer_option VALUES ($1, $2)",
        &[id(16, 4), CORRECT_OPTION],
    )
    .await;
}

async fn remove_annas_submission(db: &mut PgConnection) {
    run(
        db,
        "DELETE FROM grade WHERE submission_uuid = $1",
        &[ANNA_SUBMISSION],
    )
    .await;
    run(
        db,
        "DELETE FROM submission WHERE uuid = $1",
        &[ANNA_SUBMISSION],
    )
    .await;
}

async fn put_max_and_anna_in_a_team(db: &mut PgConnection) {
    run(
        db,
        "INSERT INTO course_group (uuid, course_uuid, name) VALUES ($1, $2, 'Team')",
        &[TEAM, DBS],
    )
    .await;
    run(
        db,
        "INSERT INTO group_member VALUES ($1, $2), ($1, $3)",
        &[TEAM, MAX, ANNA],
    )
    .await;
}

async fn restrict_upload_to_a_grouping(db: &mut PgConnection) {
    run(
        db,
        "INSERT INTO grouping (uuid, course_uuid, name) VALUES ($1, $2, 'Allowed teams')",
        &[ALLOWED_TEAMS, DBS],
    )
    .await;
    run(
        db,
        "UPDATE file_upload SET grouping_uuid = $1",
        &[ALLOWED_TEAMS],
    )
    .await;
}

/// Fails if the course filter is lost: Max's 100-point submission in the other course would lift his average from 8 to 54.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn activity_excludes_other_courses() {
    let mut db = fixture().await;
    run(
        &mut db,
        "INSERT INTO task (uuid, course_section_uuid, name) VALUES ($1, $2, 'Other course')",
        &[id(9, 3), ALGO_INTRO],
    )
    .await;
    run(&mut db, "INSERT INTO file_upload (uuid, task_uuid, title, max_points) VALUES ($1, $2, 'Other assignment', 100)", &[id(10, 2), id(9, 3)]).await;
    run(
        &mut db,
        "INSERT INTO submission (uuid, file_upload_uuid, submitter_uuid) VALUES ($1, $2, $3)",
        &[id(11, 3), id(10, 2), MAX],
    )
    .await;
    run(
        &mut db,
        "INSERT INTO grade (uuid, grader_uuid, submission_uuid, points) VALUES ($1, $2, $3, 100)",
        &[id(12, 3), PROF, id(11, 3)],
    )
    .await;
    let rows = courses::most_active(&mut db, DBS).await.unwrap();
    let max = rows.iter().find(|r| r.full_name == "Max Müller").unwrap();
    assert_eq!(max.submissions, 1);
    assert_eq!(max.avg_points.unwrap().to_string(), "8.00");
}

/// Fails if a draft is counted or a blank attempt skipped: either one moves the average off the expected value.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn quiz_statistics_count_submitted_attempts_only() {
    let mut db = fixture().await;
    add_blank_and_draft(&mut db).await;
    // (10 + 15 + 0) / 3: the blank attempt counts as zero, the draft not at all.
    let stats = quizzes::stats(&mut db).await.unwrap();
    assert_eq!((stats[0].attempts, stats[0].students), (3, 2));
    assert_eq!(stats[0].avg_points.unwrap().to_string(), "8.33");
    assert_eq!(quizzes::list(&mut db).await.unwrap()[0].attempts, 3);
    let items = quizzes::item_analysis(&mut db, QUIZ).await.unwrap();
    assert!(items.iter().all(|r| r.attempts == 3));
    assert_eq!(items.iter().map(|r| r.chosen).sum::<i64>(), 6);
}

/// Fails if a draft is scored: its selected correct answer would give Jonas points and a place in the ranking.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn drafts_have_no_score_and_no_ranking() {
    let mut db = fixture().await;
    add_blank_and_draft(&mut db).await;
    let draft = quizzes::results(&mut db, JONAS).await.unwrap();
    assert_eq!(draft[0].points, None);
    assert!(quizzes::ranking(&mut db, JONAS).await.unwrap().is_empty());
    let results = quizzes::results(&mut db, MAX).await.unwrap();
    assert!(
        results
            .iter()
            .any(|r| r.submitted_at.is_some() && r.points == Some(Decimal::ZERO))
    );
}

/// Fails if the ranking took the latest attempt or averaged: Max's blank retry would drop him below his 10.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn ranking_uses_the_best_attempt() {
    let mut db = fixture().await;
    add_blank_and_draft(&mut db).await;
    let ranking = quizzes::ranking(&mut db, MAX).await.unwrap();
    assert_eq!(ranking.len(), 1);
    assert_eq!(
        (ranking[0].rank, ranking[0].participants, ranking[0].points),
        (2, 2, Decimal::from(10))
    );
}

/// Fails if an unmarked free-text answer scored anything, or if marking it did not raise the score and clear the pending count.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn free_text_answers_score_only_once_marked() {
    let mut db = fixture().await;
    run(&mut db, "INSERT INTO question (uuid, course_uuid, body, type, default_points) VALUES ($1, $2, 'Warum?', 'free_text', 5)", &[id(13, 4), DBS]).await;
    run(
        &mut db,
        "INSERT INTO quiz_question VALUES ($1, $2, 4, 5)",
        &[QUIZ, id(13, 4)],
    )
    .await;
    run(&mut db, "INSERT INTO quiz_answer_text (attempt_uuid, question_uuid, free_text) VALUES ($1, $2, 'weil')", &[MAX_ATTEMPT, id(13, 4)]).await;
    // The question raises the maximum at once, the answer scores only when marked.
    let before = quizzes::results(&mut db, MAX).await.unwrap();
    assert_eq!(before[0].max_points, Some(Decimal::from(25)));
    assert_eq!(before[0].points, Some(Decimal::from(10)));
    assert_eq!(quizzes::stats(&mut db).await.unwrap()[0].pending, 1);
    run(&mut db, "INSERT INTO grade (uuid, grader_uuid, attempt_uuid, question_uuid, points) VALUES ($1, $2, $3, $4, 5)", &[id(12, 9), PROF, MAX_ATTEMPT, id(13, 4)]).await;
    let after = quizzes::results(&mut db, MAX).await.unwrap();
    assert_eq!(after[0].points, Some(Decimal::from(15)));
    assert_eq!(quizzes::stats(&mut db).await.unwrap()[0].pending, 0);
    let ranking = quizzes::ranking(&mut db, MAX).await.unwrap();
    assert_eq!(
        (ranking[0].rank, ranking[0].participants, ranking[0].points),
        (1, 2, Decimal::from(15))
    );
}

/// Fails if either trigger lets an answer through for a question the attempted quiz does not contain.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn answers_are_confined_to_the_attempted_quiz() {
    let mut db = fixture().await;
    // A question of the course that is in no quiz.
    run(&mut db, "INSERT INTO question (uuid, course_uuid, body, type, default_points) VALUES ($1, $2, 'Fremd', 'mcq', 5)", &[id(13, 5), DBS]).await;
    run(&mut db, "INSERT INTO question_option (uuid, question_uuid, position, body, fraction) VALUES ($1, $2, 1, 'X', 1)", &[id(14, 10), id(13, 5)]).await;
    let chosen = try_run(
        &mut db,
        "INSERT INTO quiz_answer_option VALUES ($1, $2)",
        &[MAX_ATTEMPT, id(14, 10)],
    )
    .await
    .unwrap_err();
    assert!(chosen.to_string().contains("not part of the quiz"));
    run(&mut db, "ROLLBACK TO SAVEPOINT probe", &[]).await;
    let written = try_run(&mut db, "INSERT INTO quiz_answer_text (attempt_uuid, question_uuid, free_text) VALUES ($1, $2, 'weil')", &[MAX_ATTEMPT, id(13, 5)])
        .await
        .unwrap_err();
    assert!(written.to_string().contains("not part of the quiz"));
}

/// Fails if staff or inactive members are listed as missing, or if an existing submission is overlooked.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn missing_submissions_are_active_students_without_one() {
    let mut db = fixture().await;
    // Students submitted while the professor and the HiWi did not.
    // Ensure the latter is not miscounted as missing submissions.
    assert!(missing(&mut db).await.is_empty());
    remove_annas_submission(&mut db).await;
    assert_eq!(missing(&mut db).await, ["Anna Weber"]);
    run(
        &mut db,
        "UPDATE course_member SET status = 'inactive' WHERE user_uuid = $1",
        &[ANNA],
    )
    .await;
    assert!(missing(&mut db).await.is_empty());
}

/// Fails if, on a group assignment, an upload without a group counted for anyone, or a group upload counted outside the grouping or through a group of another course.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn group_submissions_cover_the_group_within_its_grouping() {
    let mut db = fixture().await;
    remove_annas_submission(&mut db).await;
    put_max_and_anna_in_a_team(&mut db).await;
    // Without a group, Max's upload counts for nobody on a group assignment, not even for Max.
    run(&mut db, "UPDATE file_upload SET group_mode = true", &[]).await;
    assert_eq!(missing(&mut db).await.len(), 2);
    run(&mut db, "UPDATE submission SET group_uuid = $1", &[TEAM]).await;
    assert!(missing(&mut db).await.is_empty());
    restrict_upload_to_a_grouping(&mut db).await;
    assert_eq!(missing(&mut db).await.len(), 2);
    run(
        &mut db,
        "INSERT INTO grouping_group VALUES ($1, $2)",
        &[ALLOWED_TEAMS, TEAM],
    )
    .await;
    assert!(missing(&mut db).await.is_empty());
    // A group of another course does not count, even with the same members.
    run(&mut db, "UPDATE course_group SET course_uuid = $1", &[ALGO]).await;
    assert_eq!(missing(&mut db).await.len(), 2);
}

/// Fails with a division by zero if the `NULLIF` guard on `max_points` is removed.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn zero_point_assignments_have_no_percentage() {
    let mut db = fixture().await;
    run(&mut db, "UPDATE file_upload SET max_points = 0", &[]).await;
    let grades = uploads::grades(&mut db, MAX).await.unwrap();
    assert_eq!(grades.len(), 1);
    assert_eq!(grades[0].percent, None);
}

/// Fails if Anna saw Max's grade through a group that is invalid for the assignment, or missed it through a valid one.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn group_grades_cover_teammates_only_in_valid_groups() {
    let mut db = fixture().await;
    remove_annas_submission(&mut db).await;
    assert_eq!(anna_grades(&mut db).await, 0);
    put_max_and_anna_in_a_team(&mut db).await;
    run(&mut db, "UPDATE submission SET group_uuid = $1", &[TEAM]).await;
    // Max's upload names the team, but the assignment is still individual.
    assert_eq!(anna_grades(&mut db).await, 0);
    run(&mut db, "UPDATE file_upload SET group_mode = true", &[]).await;
    let grades = uploads::grades(&mut db, ANNA).await.unwrap();
    assert_eq!(grades.len(), 1);
    assert_eq!(grades[0].points, Decimal::from(8));
    // Sanity check
    assert!(uploads::grades(&mut db, JONAS).await.unwrap().is_empty());
    restrict_upload_to_a_grouping(&mut db).await;
    assert_eq!(anna_grades(&mut db).await, 0);
    run(
        &mut db,
        "INSERT INTO grouping_group VALUES ($1, $2)",
        &[ALLOWED_TEAMS, TEAM],
    )
    .await;
    assert_eq!(anna_grades(&mut db).await, 1);
    run(&mut db, "UPDATE course_group SET course_uuid = $1", &[ALGO]).await;
    assert_eq!(anna_grades(&mut db).await, 0);
}

/// Fails if the student role were checked without its course: a role in a course one is not a member of would list the user.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn student_selector_uses_course_scoped_roles() {
    let mut db = fixture().await;
    assert_eq!(users::students(&mut db).await.unwrap().len(), 2);
    // The HiWi becomes a student of the other course, but needs membership there too.
    run(
        &mut db,
        "INSERT INTO role_assignment (user_uuid, role_uuid, course_uuid) VALUES ($1, $2, $3)",
        &[JONAS, STUDENT_ROLE, ALGO],
    )
    .await;
    assert_eq!(users::students(&mut db).await.unwrap().len(), 2);
    run(
        &mut db,
        "INSERT INTO course_member (user_uuid, course_uuid) VALUES ($1, $2)",
        &[JONAS, ALGO],
    )
    .await;
    let students = users::students(&mut db).await.unwrap();
    assert_eq!(students.len(), 3);
    assert!(students.iter().any(|s| s.uuid == JONAS));
}
