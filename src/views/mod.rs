mod components;
mod rows;

use maud::{DOCTYPE, Markup, PreEscaped, html};
use uuid::Uuid;

use crate::db::{
    courses::{Activity, Course, CourseOverview, Material, Member},
    forums::ForumActivity,
    notifications::UnreadNotifications,
    quizzes::{ItemCell, QuizRanking, QuizRef, QuizResult, QuizStats},
    uploads::{GradedSubmission, SubmissionTime, Upload, UploadStatus},
    users::{Person, Student},
};
use components::{answer_table, bar_chart, card, meter, stat, table, timing_chart};
use rows::{grading_method, points};

/// Joins a course with the item it contains in labels and captions.
const SEP: &str = " \u{b7} ";

const NAV: &[(&str, &str)] = &[
    ("/", "Kursübersicht"),
    ("/assignments", "Aufgaben"),
    ("/quizzes", "Quizze"),
    ("/forums", "Foren"),
    ("/student", "Studentenprofil"),
    ("/course", "Kursdetails"),
    ("/notifications", "Benachrichtigungen"),
];

pub fn layout(current: &str, title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="de" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) (SEP) "Moodle Dashboard" }
                link rel="stylesheet" href="/static/pico.min.css";
                link rel="stylesheet" href="/static/app.css";
                // The selects submit on change; without scripting the buttons take over.
                noscript { style { (PreEscaped("form button[type=submit] { display: inline-block; }")) } }
            }
            body {
                header.container {
                    nav {
                        ul { li { strong { "Moodle Dashboard" } } }
                        ul {
                            @for (href, label) in NAV {
                                li { a href=(href) aria-current=[(*href == current).then_some("page")] { (label) } }
                            }
                        }
                    }
                }
                main.container { h1 { (title) } (body) }
            }
        }
    }
}

pub fn empty(current: &str, title: &str, msg: &str) -> Markup {
    layout(current, title, html! { p.empty { (msg) } })
}

pub fn overview(rows: &[CourseOverview]) -> Markup {
    let sum = |f: fn(&CourseOverview) -> i64| rows.iter().map(f).sum::<i64>();
    layout(
        "/",
        "Kursübersicht",
        html! {
            div.grid {
                (stat("Kurse", rows.len()))
                (stat("Einschreibungen", sum(|r| r.active_members)))
                (stat("Aufgaben", sum(|r| r.assignments)))
                (stat("Quizze", sum(|r| r.quizzes)))
                (stat("Foren", sum(|r| r.forums)))
            }
            (card("Kurse", table(
                &["Kurs", "Start", "Ende", "Aktive Mitglieder", "Abschnitte", "Dateien", "Aufgaben", "Quizze", "Foren"],
                rows,
            )))
        },
    )
}

pub fn assignments(rows: &[UploadStatus]) -> Markup {
    let submitted: i64 = rows.iter().map(|r| r.submitted).sum();
    let graded: i64 = rows.iter().map(|r| r.graded).sum();
    let chart: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                format!("{}{SEP}{}", r.course, r.assignment),
                r.submitted as f64,
            )
        })
        .collect();
    layout(
        "/assignments",
        "Abgabenstatus aller Aufgaben",
        html! {
            div.grid {
                (stat("Aufgaben", rows.len()))
                (stat("Eingereicht", submitted))
                (stat("Benotet", graded))
                (stat("Unbenotet", submitted - graded))
            }
            (card("Bewertungsfortschritt", html! {
                @for r in rows {
                    (meter(&format!("{}{SEP}{}", r.course, r.assignment), r.graded as f64, r.submitted as f64,
                           &format!("{} / {} benotet", r.graded, r.submitted)))
                }
            }))
            (card("Aufgaben", html! {
                (table(&["Kurs", "Aufgabe", "Max. Punkte", "Eingereicht", "Benotet", "Durchschnitt"], rows))
                (bar_chart("Abgaben je Aufgabe", &chart, None, None))
            }))
        },
    )
}

pub fn quizzes(
    stats: &[QuizStats],
    list: &[QuizRef],
    selected: Option<Uuid>,
    items: &[ItemCell],
) -> Markup {
    let chart: Vec<_> = stats
        .iter()
        .filter_map(|r| {
            r.avg_points
                .map(|p| (r.quiz.clone(), p.try_into().unwrap_or(0.0)))
        })
        .collect();
    let title = list
        .iter()
        .find(|q| Some(q.uuid) == selected)
        .map(|q| format!("{}{SEP}{}", q.course, q.title));
    layout(
        "/quizzes",
        "Quiz-Statistiken",
        html! {
            (card("Quizze", html! {
                p { "Statistiken und Antwortverteilung berücksichtigen nur abgegebene Versuche; leere Abgaben zählen mit 0 Punkten. Die Wertung bestimmt, welcher Versuch in die Rangliste eingeht. Freitextantworten zählen mit den vergebenen Punkten, noch nicht bewertete mit 0. Die Antwortverteilung umfasst nur Auswahlfragen." }
                (table(&["Quiz", "Kurs", "Wertung", "Abgegebene Versuche", "Studierende", "Durchschnitt je Versuch", "Offene Freitexte"], stats))
                @if !chart.is_empty() { (bar_chart("Durchschnittliche Punktzahl je Quiz", &chart, None, None)) }
            }))
            (card("Item-Analyse", html! {
                form method="get" {
                    label { "Quiz" select name="uuid" onchange="this.form.submit()" {
                        @for q in list {
                            option value=(q.uuid) selected[Some(q.uuid) == selected] { (q.course) (SEP) (q.title) }
                        }
                    } }
                    button type="submit" { "Anzeigen" }
                }
                @match title {
                    Some(t) => (answer_table(&format!("Antwortverteilung: {t}"), items)),
                    None => p.empty { "Keine Quizze in der Datenbank." },
                }
            }))
        },
    )
}

pub fn forums(rows: &[ForumActivity]) -> Markup {
    layout(
        "/forums",
        "Forumsaktivität",
        card(
            "Foren",
            table(
                &[
                    "Kurs",
                    "Forum",
                    "Threads",
                    "Beiträge",
                    "Autoren",
                    "Letzter Beitrag",
                ],
                rows,
            ),
        ),
    )
}

pub fn student(
    list: &[Student],
    selected: Uuid,
    materials: &[Material],
    grades: &[GradedSubmission],
    results: &[QuizResult],
    ranking: &[QuizRanking],
) -> Markup {
    layout(
        "/student",
        "Studentenprofil",
        html! {
            form method="get" {
                label { "Student:in" select name="uuid" onchange="this.form.submit()" {
                    @for s in list {
                        option value=(s.uuid) selected[s.uuid == selected] { (s.full_name) " (" (s.email) ")" }
                    }
                } }
                button type="submit" { "Anzeigen" }
            }
            div.cols {
                (card("Kursmaterialien", table(&["Kurs", "Abschnitt", "Datei", "Typ"], materials)))
                (card("Bewertungen", table(
                    &["Kurs", "Aufgabe", "Punkte", "Prozent", "Feedback", "Bewertet am"],
                    grades,
                )))
            }
            (card("Quiz-Ergebnisse", table(&["Quiz", "Kurs", "Begonnen", "Abgegeben", "Punkte"], results)))
            @if !ranking.is_empty() {
                (card("Einordnung im Kurs", html! {
                    @for r in ranking {
                        (bar_chart(&format!("{} ({}): Platz {} von {}{SEP}{} / {} Punkte",
                                            r.quiz, grading_method(&r.grading_method), r.rank, r.participants,
                                            points(r.points), r.max_points.map_or_else(String::new, points)),
                                   &r.entries, Some(r.selected), r.max_points.and_then(|m| m.try_into().ok())))
                    }
                }))
            }
        },
    )
}

#[derive(Clone, Copy)]
pub struct CoursePage<'a> {
    pub courses: &'a [Course],
    pub selected: Uuid,
    pub members: &'a [Member],
    pub most_active: &'a [Activity],
    pub uploads: &'a [Upload],
    pub upload: Option<Uuid>,
    pub missing: Option<&'a [Person]>,
    pub times: &'a [SubmissionTime],
}

pub fn course(p: CoursePage) -> Markup {
    let CoursePage {
        courses,
        selected,
        members,
        most_active,
        uploads,
        upload,
        missing,
        times,
    } = p;
    let current = courses.iter().find(|c| c.uuid == selected);
    let upload_title = uploads
        .iter()
        .find(|u| Some(u.uuid) == upload)
        .map(|a| a.title.as_str());
    layout(
        "/course",
        "Kursdetails",
        html! {
            form method="get" {
                label { "Kurs" select name="uuid" onchange="this.form.submit()" {
                    @for c in courses {
                        option value=(c.uuid) selected[c.uuid == selected] { (c.title) " (" (c.start_date.format("%Y")) ")" }
                    }
                } }
                button type="submit" { "Anzeigen" }
            }
            @if let Some(cap) = current.and_then(|c| c.max_capacity) {
                @let active = members.iter().filter(|m| m.status == "active").count();
                (card("Belegung", meter("Aktive Mitglieder", active as f64, f64::from(cap), &format!("{active} / {cap}"))))
            }
            div.cols {
                (card("Mitglieder und Rollen", table(&["Name", "E-Mail", "Rollen", "Beigetreten", "Status"], members)))
                (card("Aktivste Teilnehmer:innen", table(&["Name", "Abgaben", "Durchschnitt"], most_active)))
            }
            (card("Fehlende Abgaben", html! {
                @if uploads.is_empty() {
                    p.empty { "Keine Abgaben in diesem Kurs." }
                } @else {
                    form method="get" {
                        input type="hidden" name="uuid" value=(selected);
                        label { "Aufgabe" select name="assignment" onchange="this.form.submit()" {
                            @for u in uploads {
                                option value=(u.uuid) selected[Some(u.uuid) == upload] { (u.title) }
                            }
                        } }
                        button type="submit" { "Anzeigen" }
                    }
                    @match missing {
                        Some([]) => p.empty { "Alle Studierenden haben abgegeben." },
                        Some(rows) => {
                            p { (rows.len()) " Studierende haben noch nicht abgegeben:" }
                            (table(&["Name", "E-Mail"], rows))
                        }
                        None => {}
                    }
                }
            }))
            @if let Some(t) = upload_title {
                (card("Abgabezeitpunkte", timing_chart(&format!("Kumulierte Abgaben: {t}"), times)))
            }
        },
    )
}

pub fn notifications(rows: &[UnreadNotifications]) -> Markup {
    let chart: Vec<_> = rows
        .iter()
        .map(|r| (r.full_name.clone(), r.unread as f64))
        .collect();
    layout(
        "/notifications",
        "Ungelesene Benachrichtigungen aktiver Nutzer",
        html! {
            @if rows.is_empty() {
                p.empty { "Keine ungelesenen Benachrichtigungen." }
            } @else {
                (card("Nutzer", html! {
                    (table(&["Name", "E-Mail", "Ungelesen", "Älteste ungelesene"], rows))
                    (bar_chart("Ungelesene Benachrichtigungen je Nutzer", &chart, None, None))
                }))
            }
        },
    )
}
