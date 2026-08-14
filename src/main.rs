use chrono::{Duration, Local, NaiveDate};
use eframe::egui::{self, Color32, RichText};
use rusqlite::{params, Connection};
use std::path::PathBuf;


#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    NotApplied,
    Applied,
    FirstCallTelephone,
    FirstCallVideo,
    FurtherRounds,
    Selected,
    Rejected,
    ReminderSent,
}

impl Status {
    const ALL: [Status; 8] = [
        Status::NotApplied,
        Status::Applied,
        Status::FirstCallTelephone,
        Status::FirstCallVideo,
        Status::FurtherRounds,
        Status::Selected,
        Status::Rejected,
        Status::ReminderSent,
    ];

    /// Storing in DB as string
    fn as_str(self) -> &'static str {
        match self {
            Status::NotApplied => "Not Applied",
            Status::Applied => "Applied",
            Status::FirstCallTelephone => "First Call (Telephone)",
            Status::FirstCallVideo => "First Call (Video)",
            Status::FurtherRounds => "Further Rounds",
            Status::Selected => "Selected",
            Status::Rejected => "Rejected",
            Status::ReminderSent => "Reminder Sent",
        }
    }

    /// Read from DB as string
    fn from_str(s: &str) -> Self {
        Status::ALL.into_iter().find(|x| x.as_str() == s).unwrap_or(Status::Applied)
    }
}


// `Application` represents a single job application record stored in SQLite.
// If you change the fields here, update the DB schema in `App::new()` accordingly.
#[derive(Clone)]
struct Application {
    id: i64,
    company: String,
    location: String,
    title: String,
    link: String,
    application_date: String,
    status: Status,
    contact_name: String,
    contact_phone: String,
    contact_email: String,
    notes: String,
    last_contact: Option<String>,
}

struct Form {
    company: String,
    location: String,
    title: String,
    link: String,
    application_date: String,
    status: Status,
    contact_name: String,
    contact_phone: String,
    contact_email: String,
    notes: String,
}
impl Default for Form {
    fn default() -> Self {
        Self {
            company: String::new(), location: String::new(), title: String::new(), link: String::new(),
            application_date: today(), status: Status::Applied, contact_name: String::new(),
            contact_phone: String::new(), contact_email: String::new(), notes: String::new(),
        }
    }
}

#[derive(PartialEq)]
enum Page { Dashboard, Add, Applications, Detail, FollowUps, Analytics, Settings }


struct App {
    db: Connection,
    page: Page,
    applications: Vec<Application>,
    form: Form,
    editing_id: Option<i64>,
    selected_id: Option<i64>,
    search: String,
    status_filter: Option<Status>,
    location_filter: String,
    // `followup_days` controls how many days without contact will mark an application
    // as needing a follow-up. Lower values => more frequent follow-up reminders.
    followup_days: i64,
    message: Option<String>,
    analysis_from: String,
    analysis_to: String,
}

/// Function to get todays date
/// 
fn today() -> String {
     Local::now().date_naive().format("%Y-%m-%d").to_string() 
    }
    
fn db_path() -> PathBuf {
    // Determine where to store the local SQLite DB.
    // On Windows `LOCALAPPDATA` will be used; on Unix-like systems `HOME` is used.
    // Change this function if you want the DB in a custom location.
    let base = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("JobHuntTracker").join("jobs.db")
}

fn main() {
    println!("Hello, world!");
}
