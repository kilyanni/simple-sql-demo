use std::fmt::Display;

use chrono::NaiveDateTime;
use rust_decimal::Decimal;

use crate::db::{
    courses::{Activity, CourseOverview, Material, Member},
    forums::ForumActivity,
    notifications::UnreadNotifications,
    quizzes::{QuizResult, QuizStats},
    uploads::{GradedSubmission, UploadStatus},
    users::Person,
};

/// One table row per query result.
pub trait Row {
    fn cells(&self) -> Vec<String>;
}

fn opt<T: Display>(v: Option<&T>) -> String {
    v.map(ToString::to_string).unwrap_or_default()
}

/// Point values are `NUMERIC(6, 2)`; trailing zeros are noise in a table cell.
pub fn points(value: Decimal) -> String {
    value.normalize().to_string()
}

/// German label for `quiz.grading_method`.
pub fn grading_method(method: &str) -> &'static str {
    match method {
        "highest" => "bester Versuch",
        "average" => "Durchschnitt",
        "first" => "erster Versuch",
        "last" => "letzter Versuch",
        _ => "unbekannt",
    }
}

fn dt(t: &NaiveDateTime) -> String {
    t.format("%Y-%m-%d %H:%M").to_string()
}

fn odt(t: Option<&NaiveDateTime>) -> String {
    t.map(dt).unwrap_or_default()
}

impl Row for CourseOverview {
    fn cells(&self) -> Vec<String> {
        vec![
            self.course.clone(),
            self.start_date.to_string(),
            opt(self.end_date.as_ref()),
            self.active_members.to_string(),
            self.sections.to_string(),
            self.files.to_string(),
            self.assignments.to_string(),
            self.quizzes.to_string(),
            self.forums.to_string(),
        ]
    }
}

impl Row for Material {
    fn cells(&self) -> Vec<String> {
        vec![
            self.course.clone(),
            self.section.clone(),
            self.file.clone(),
            self.r#type.clone(),
        ]
    }
}

impl Row for UploadStatus {
    fn cells(&self) -> Vec<String> {
        vec![
            self.course.clone(),
            self.assignment.clone(),
            points(self.max_points),
            self.submitted.to_string(),
            self.graded.to_string(),
            opt(self.average.as_ref()),
        ]
    }
}

impl Row for Person {
    fn cells(&self) -> Vec<String> {
        vec![self.full_name.clone(), self.email.clone()]
    }
}

impl Row for GradedSubmission {
    fn cells(&self) -> Vec<String> {
        vec![
            self.course.clone(),
            self.assignment.clone(),
            format!("{} / {}", points(self.points), points(self.max_points)),
            self.percent
                .map_or_else(|| "n/a".into(), |p| format!("{p} %")),
            opt(self.feedback.as_ref()),
            dt(&self.graded_at),
        ]
    }
}

impl Row for ForumActivity {
    fn cells(&self) -> Vec<String> {
        vec![
            self.course.clone(),
            self.forum.clone(),
            self.threads.to_string(),
            self.posts.to_string(),
            self.authors.to_string(),
            odt(self.last_post.as_ref()),
        ]
    }
}

impl Row for QuizResult {
    fn cells(&self) -> Vec<String> {
        vec![
            self.quiz.clone(),
            self.course.clone(),
            dt(&self.started_at),
            odt(self.submitted_at.as_ref()),
            match self.points {
                Some(scored) => format!(
                    "{} / {}",
                    points(scored),
                    self.max_points.map_or_else(String::new, points)
                ),
                None => "Noch nicht abgegeben".into(),
            },
        ]
    }
}

impl Row for QuizStats {
    fn cells(&self) -> Vec<String> {
        vec![
            self.quiz.clone(),
            self.course.clone(),
            grading_method(&self.grading_method).to_string(),
            self.attempts.to_string(),
            self.students.to_string(),
            opt(self.avg_points.as_ref()),
            self.pending.to_string(),
        ]
    }
}

impl Row for Member {
    fn cells(&self) -> Vec<String> {
        vec![
            self.full_name.clone(),
            self.email.clone(),
            opt(self.roles.as_ref()),
            dt(&self.joined_at),
            self.status.clone(),
        ]
    }
}

impl Row for Activity {
    fn cells(&self) -> Vec<String> {
        vec![
            self.full_name.clone(),
            self.submissions.to_string(),
            opt(self.avg_points.as_ref()),
        ]
    }
}

impl Row for UnreadNotifications {
    fn cells(&self) -> Vec<String> {
        vec![
            self.full_name.clone(),
            self.email.clone(),
            self.unread.to_string(),
            dt(&self.oldest_unread),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_percentage_has_an_explicit_placeholder() {
        let grade = GradedSubmission {
            course: "Course".into(),
            assignment: "Practice".into(),
            points: Decimal::ZERO,
            max_points: Decimal::ZERO,
            percent: None,
            feedback: None,
            graded_at: chrono::NaiveDate::from_ymd_opt(2026, 9, 20)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap(),
        };
        assert_eq!(grade.cells()[3], "n/a");
    }
}
