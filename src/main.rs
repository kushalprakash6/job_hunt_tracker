use rusqlite::{params, Connection};
use chrono::{Duration, Local, NaiveDate};


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


fn main() {
    println!("Hello, world!");
}
