use std::fmt::Display;

use maud::{Markup, PreEscaped, html};
use plotters::{coord::Shift, prelude::*};

use super::rows::Row;
use crate::db::{quizzes::ItemCell, uploads::SubmissionTime};

pub fn card(title: &str, body: Markup) -> Markup {
    html! { article { h2 { (title) } (body) } }
}

pub fn stat(label: &str, value: impl Display) -> Markup {
    html! { article.stat { small { (label) } div.stat-value { (value) } } }
}

pub fn meter(label: &str, value: f64, max: f64, text: &str) -> Markup {
    html! {
        div.meter {
            span { (label) }
            progress value=(value) max=(max.max(1.0)) {}
            span { (text) }
        }
    }
}

pub fn table<T: Row>(headers: &[&str], rows: &[T]) -> Markup {
    table_rows(headers, rows.iter().map(Row::cells).collect())
}

pub fn table_rows(headers: &[&str], rows: Vec<Vec<String>>) -> Markup {
    html! {
        div.overflow-auto { table {
            thead { tr { @for h in headers { th { (h) } } } }
            tbody {
                @if rows.is_empty() {
                    tr { td colspan=(headers.len()) { "Keine Einträge." } }
                }
                @for row in rows {
                    tr { @for cell in row { td { (cell) } } }
                }
            }
        } }
    }
}

type PlotResult = Result<(), Box<dyn std::error::Error>>;

fn svg_chart(
    title: &str,
    height: u32,
    draw: impl FnOnce(DrawingArea<SVGBackend<'_>, Shift>) -> PlotResult,
) -> Markup {
    let mut svg = String::new();
    let result = draw(SVGBackend::with_string(&mut svg, (960, height)).into_drawing_area());
    if let Err(error) = result {
        eprintln!("Chart rendering failed: {error}");
        return html! { p { "Diagramm konnte nicht dargestellt werden." } };
    }
    html! {
        figure.chart {
            figcaption { (title) }
            // Only Plotters-generated SVG is inserted unescaped; Plotters escapes text.
            div role="img" aria-label=(title) { (PreEscaped(svg)) }
        }
    }
}

/// Simple bar chart, e.g. for rankings.
/// If `highlight` is set, all other bars get muted coloring.
pub fn bar_chart(
    title: &str,
    data: &[(String, f64)],
    highlight: Option<usize>,
    max: Option<f64>,
) -> Markup {
    if data.is_empty() {
        return html! { p { "Keine Einträge." } };
    }
    let low = data.iter().map(|(_, v)| *v).fold(0.0, f64::min);
    let high = data
        .iter()
        .map(|(_, v)| *v)
        .fold(max.unwrap_or(1.0).max(1.0), f64::max);
    svg_chart(title, 80 + data.len() as u32 * 26, |root| {
        let mut chart = ChartBuilder::on(&root)
            .margin(15)
            .x_label_area_size(35)
            .y_label_area_size(360)
            .build_cartesian_2d(low..high, (0..data.len()).into_segmented())?;
        chart
            .configure_mesh()
            .disable_mesh()
            .y_labels(data.len())
            .label_style(("sans-serif", 12))
            .y_label_formatter(&|v| match v {
                SegmentValue::CenterOf(i) if *i < data.len() => data[data.len() - 1 - i].0.clone(),
                _ => String::new(),
            })
            .draw()?;
        chart.draw_series(data.iter().enumerate().map(|(i, (_, value))| {
            let y = data.len() - 1 - i;
            let color = if highlight.is_none_or(|selected| selected == i) {
                BLUE
            } else {
                RGBColor(180, 180, 180)
            };
            let mut bar = Rectangle::new(
                [
                    (0.0, SegmentValue::Exact(y)),
                    (*value, SegmentValue::Exact(y + 1)),
                ],
                color.filled(),
            );
            bar.set_margin(4, 4, 0, 0);
            bar
        }))?;
        root.present()?;
        Ok(())
    })
}

pub fn timing_chart(title: &str, times: &[SubmissionTime]) -> Markup {
    let mut hours: Vec<_> = times
        .iter()
        .filter_map(SubmissionTime::hours_after_due)
        .collect();
    if hours.is_empty() {
        return html! { p { "Keine Frist oder keine Abgaben." } };
    }
    hours.sort_by(f64::total_cmp);
    let low = hours[0].min(-24.0);
    let high = hours[hours.len() - 1].max(24.0);
    let mut points = vec![(low, 0)];
    for (i, hour) in hours.iter().enumerate() {
        // We want a staircase not a line-plot, so 2 points per elem
        points.extend([(*hour, i as u32), (*hour, i as u32 + 1)]);
    }
    points.push((high, hours.len() as u32));
    svg_chart(title, 280, |root| {
        let mut chart = ChartBuilder::on(&root)
            .margin(15)
            .x_label_area_size(45)
            .y_label_area_size(55)
            .build_cartesian_2d(low..high, 0u32..hours.len() as u32 + 1)?;
        chart
            .configure_mesh()
            .x_desc("Stunden relativ zur Frist (0)")
            .y_desc("Abgaben")
            .draw()?;
        chart.draw_series(LineSeries::new(points, BLUE.stroke_width(2)))?;
        chart.draw_series(LineSeries::new([(0.0, 0), (0.0, hours.len() as u32)], RED))?;
        root.present()?;
        Ok(())
    })
}

pub fn answer_table(title: &str, cells: &[ItemCell]) -> Markup {
    html! {
        p { (title) }
        (table_rows(&["Frage", "Antwort", "Wertung", "Gewählt", "Abgegebene Versuche", "Anteil"], cells.iter().map(|c| {
            vec![format!("F{}: {}", c.position, c.question), c.option_text.clone(),
                 c.fraction.to_string(), c.chosen.to_string(), c.attempts.to_string(), format!("{:.0} %", c.share())]
        }).collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn answer_table_handles_many_options_and_escapes_text() {
        let cells: Vec<_> = (0..28)
            .map(|i| ItemCell {
                position: 1,
                question: "<Question>".into(),
                option_text: format!("Answer {i}"),
                fraction: Decimal::ZERO,
                chosen: 1,
                attempts: 2,
            })
            .collect();
        let html = answer_table("Options", &cells).into_string();
        assert!(html.contains("Answer 27"));
        assert!(html.contains("50 %"));
        assert!(html.contains("&lt;Question&gt;"));
        assert!(
            answer_table("Empty", &[])
                .into_string()
                .contains("Keine Einträge.")
        );
    }

    #[test]
    fn plotters_renders_zero_negative_and_escaped_labels() {
        let html = bar_chart(
            "Scores",
            &[
                ("<script>alert(1)</script>".into(), -2.0),
                ("Zero".into(), 0.0),
            ],
            Some(1),
            Some(0.0),
        )
        .into_string();
        assert!(html.contains("<svg"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
