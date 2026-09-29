//! `SQLx` checks these PostgreSQL queries at compile time. Counts are declared
//! non-null with the `!` alias; aggregates that can be NULL stay `Option`.

pub mod courses;
pub mod forums;
pub mod notifications;
pub mod quizzes;
pub mod uploads;
pub mod users;

#[cfg(test)]
mod tests;
