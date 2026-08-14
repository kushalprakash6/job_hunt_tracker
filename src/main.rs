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


impl App {
    fn new() -> Self {
        let path = db_path();
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).expect("create app data directory"); }
        let db = Connection::open(path).expect("open SQLite database");
        // Initialize DB schema. If you add/remove fields from `Application`, update
        // this schema so the table columns match the struct fields and defaults.
        db.execute_batch(r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS applications (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                company TEXT NOT NULL,
                location TEXT NOT NULL DEFAULT '',
                title TEXT NOT NULL,
                link TEXT NOT NULL DEFAULT '',
                application_date TEXT NOT NULL,
                status TEXT NOT NULL,
                contact_name TEXT NOT NULL DEFAULT '',
                contact_phone TEXT NOT NULL DEFAULT '',
                contact_email TEXT NOT NULL DEFAULT '',
                notes TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS activities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                application_id INTEGER NOT NULL REFERENCES applications(id) ON DELETE CASCADE,
                activity_type TEXT NOT NULL,
                activity_date TEXT NOT NULL,
                notes TEXT NOT NULL DEFAULT ''
            );
        "#).expect("initialize database");
        // Default `followup_days` is 14 (two weeks). Change this value to alter follow-up detection.
        let mut app = Self { 
            db, 
            page: Page::Dashboard, 
            applications: Vec::new(), 
            form: Form::default(), 
            editing_id: None, 
            selected_id: None, 
            search: String::new(), 
            status_filter: None, 
            location_filter: String::new(), 
            followup_days: 14, 
            message: None, 
            analysis_from: String::new(), 
            analysis_to: String::new() 
        };
        app.reload();
        app
    }

    fn reload(&mut self) {
        let mut stmt = self.db.prepare(r#"SELECT a.id,a.company,a.location,a.title,a.link,a.application_date,a.status,a.contact_name,a.contact_phone,a.contact_email,a.notes,(SELECT MAX(activity_date) FROM activities WHERE application_id=a.id) FROM applications a ORDER BY a.application_date DESC,a.id DESC"#).unwrap();
        let rows = stmt.query_map(
            [], |r| Ok(Application {
                 id:r.get(0)?, 
                 company:r.get(1)?, 
                 location:r.get(2)?, 
                 title:r.get(3)?, 
                 link:r.get(4)?, 
                 application_date:r.get(5)?, 
                 status:Status::from_str(&r.get::<_,String>(6)?), 
                 contact_name:r.get(7)?, 
                 contact_phone:r.get(8)?, 
                 contact_email:r.get(9)?, 
                 notes:r.get(10)?, 
                 last_contact:r.get(11)? }))
                 .unwrap();
        self.applications = rows.filter_map(Result::ok).collect();
    }

    fn save_form(&mut self) {
        if self.form.company.trim().is_empty() || self.form.title.trim().is_empty() {
            self.message = Some("Company and job title are required.".into()); 
            return;
        }
        let now = today();
        match self.editing_id {
            Some(id) => { 
                self.db.execute(
                    "UPDATE applications SET company=?1,location=?2,title=?3,link=?4,application_date=?5,status=?6,contact_name=?7,contact_phone=?8,contact_email=?9,notes=?10,updated_at=?11 WHERE id=?12", 
                    params![self.form.company.trim(),
                    self.form.location.trim(),
                    self.form.title.trim(),
                    self.form.link.trim(),
                    self.form.application_date,
                    self.form.status.as_str(),
                    self.form.contact_name.trim(),
                    self.form.contact_phone.trim(),
                    self.form.contact_email.trim(),
                    self.form.notes.trim(),
                    now,id])
                    .unwrap(); 
            },
            None => {
                self.db.execute(
                    "INSERT INTO applications(company,location,title,link,application_date,status,contact_name,contact_phone,contact_email,notes,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)", 
                    params![self.form.company.trim(),
                    self.form.location.trim(),
                    self.form.title.trim(),
                    self.form.link.trim(),
                    self.form.application_date,
                    self.form.status.as_str(),
                    self.form.contact_name.trim(),
                    self.form.contact_phone.trim(),
                    self.form.contact_email.trim(),
                    self.form.notes.trim(),
                    now])
                    .unwrap();
                let id = self.db.last_insert_rowid();
                self.db.execute(
                    "INSERT INTO activities(application_id,activity_type,activity_date,notes) VALUES(?1,?2,?3,?4)", 
                    params![id,"Application submitted",
                    self.form.application_date,
                    "Initial application record"])
                    .unwrap();
            }
        }
        self.reload(); 
        self.message=Some("Application saved.".into()); 
        self.page=Page::Applications; 
        self.editing_id=None;
    }

}
fn main() {
    println!("Hello, world!");
}
