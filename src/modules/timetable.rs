use chrono::{DateTime, FixedOffset, IsoWeek};

#[derive(Debug, Clone)]
pub enum Provider {
    Lanis,
    Untis,

}

#[derive(Debug, Clone)]
pub struct TimeTable {
    pub week: IsoWeek,
    pub monday: Vec<Entry>,
    pub tuesday: Vec<Entry>,
    pub wednesday: Vec<Entry>,
    pub thursday: Vec<Entry>,
    pub friday: Vec<Entry>,
    pub saturday: Vec<Entry>,
    pub sunday: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// The short of the subject (e.g. INF)
    pub name: String,
    /// The short of the teacher (e.g. RST)
    pub teacher: String,
    /// The full lastname of the teacher (only available if [Provider::Untis] is used as TimeTable [Provider])
    pub full_teacher: String,
    pub school_hours: Vec<i32>,
    pub start: DateTime<FixedOffset>,
    pub end: DateTime<FixedOffset>,
    /// The room number (e.g. B209)
    pub room: String,
    /// Only available if [Provider::Untis] is used as TimeTable [Provider]
    pub substitution: Substitution
}

#[derive(Debug, Clone)]
pub struct Substitution {

}