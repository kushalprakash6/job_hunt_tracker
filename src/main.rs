use chrono::{Duration, Local, NaiveDate};
use eframe::egui::{self, Color32, RichText};
use rusqlite::{Connection, params};
use std::{path::PathBuf, sync::Arc};

fn app_icon_data() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../icons/icon.png"))
        .expect("valid app icon PNG")
}

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
        Status::ALL
            .into_iter()
            .find(|x| x.as_str() == s)
            .unwrap_or(Status::Applied)
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
            company: String::new(),
            location: String::new(),
            title: String::new(),
            link: String::new(),
            application_date: today(),
            status: Status::Applied,
            contact_name: String::new(),
            contact_phone: String::new(),
            contact_email: String::new(),
            notes: String::new(),
        }
    }
}

#[derive(PartialEq)]
enum Page {
    Dashboard,
    Add,
    Applications,
    Detail,
    FollowUps,
    Analytics,
    Settings,
}

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

/// Truncate a string to `max` characters, adding an ellipsis if truncated.
fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

fn sanitize_text(value: &str, max_len: usize) -> String {
    let cleaned: String = value
        .chars()
        .filter(|ch| !ch.is_control() && *ch != '\0')
        .collect();
    let trimmed = cleaned.trim();
    let limited = if trimmed.chars().count() > max_len {
        trimmed.chars().take(max_len).collect::<String>()
    } else {
        trimmed.to_string()
    };
    limited
}

fn validate_text_field(field_name: &str, value: &str, max_len: usize) -> Result<String, String> {
    let sanitized = sanitize_text(value, max_len);
    let lowered = sanitized.to_ascii_lowercase();
    let suspicious_patterns = [
        "drop table",
        "delete from",
        "insert into",
        "update applications",
        "update activities",
        "alter table",
        "truncate table",
        "union select",
        "or 1=1",
        "--",
        "/*",
        "*/",
    ];

    if suspicious_patterns.iter().any(|pattern| lowered.contains(pattern)) {
        return Err(format!("Invalid {} value detected.", field_name));
    }

    Ok(sanitized)
}

fn validate_form(form: &Form) -> Result<(), String> {
    if form.company.trim().is_empty() || form.title.trim().is_empty() {
        return Err("Company and job title are required.".to_string());
    }

    validate_text_field("company", &form.company, 200)?;
    validate_text_field("location", &form.location, 200)?;
    validate_text_field("title", &form.title, 200)?;
    validate_text_field("link", &form.link, 2048)?;
    validate_text_field("contact_name", &form.contact_name, 200)?;
    validate_text_field("contact_phone", &form.contact_phone, 100)?;
    validate_text_field("contact_email", &form.contact_email, 200)?;
    validate_text_field("notes", &form.notes, 5000)?;
    Ok(())
}

fn db_path() -> PathBuf {
    // Determine where to store the local SQLite DB.
    // On Windows `LOCALAPPDATA` will be used; on Unix-like systems `HOME` is used.
    // Change this function if you want the DB in a custom location.
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("JobHuntTracker").join("jobs.db")
}

impl App {
    fn new() -> Self {
        let path = db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create app data directory");
        }
        let db = Connection::open(path).expect("open SQLite database");
        // Initialize DB schema. If you add/remove fields from `Application`, update
        // this schema so the table columns match the struct fields and defaults.
        db.execute_batch(
            r#"
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
        "#,
        )
        .expect("initialize database");
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
            analysis_to: String::new(),
        };
        app.reload();
        app
    }

    fn reload(&mut self) {
        let mut stmt = self.db.prepare(r#"SELECT a.id,a.company,a.location,a.title,a.link,a.application_date,a.status,a.contact_name,a.contact_phone,a.contact_email,a.notes,(SELECT MAX(activity_date) FROM activities WHERE application_id=a.id) FROM applications a ORDER BY a.application_date DESC,a.id DESC"#).unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok(Application {
                    id: r.get(0)?,
                    company: r.get(1)?,
                    location: r.get(2)?,
                    title: r.get(3)?,
                    link: r.get(4)?,
                    application_date: r.get(5)?,
                    status: Status::from_str(&r.get::<_, String>(6)?),
                    contact_name: r.get(7)?,
                    contact_phone: r.get(8)?,
                    contact_email: r.get(9)?,
                    notes: r.get(10)?,
                    last_contact: r.get(11)?,
                })
            })
            .unwrap();
        self.applications = rows.filter_map(Result::ok).collect();
    }

    fn save_form(&mut self) {
        match validate_form(&self.form) {
            Ok(()) => {}
            Err(msg) => {
                self.message = Some(msg);
                return;
            }
        }

        let now = today();
        let company = sanitize_text(&self.form.company, 200);
        let location = sanitize_text(&self.form.location, 200);
        let title = sanitize_text(&self.form.title, 200);
        let link = sanitize_text(&self.form.link, 2048);
        let contact_name = sanitize_text(&self.form.contact_name, 200);
        let contact_phone = sanitize_text(&self.form.contact_phone, 100);
        let contact_email = sanitize_text(&self.form.contact_email, 200);
        let notes = sanitize_text(&self.form.notes, 5000);

        match self.editing_id {
            Some(id) => {
                self.db.execute(
                    "UPDATE applications SET company=?1,location=?2,title=?3,link=?4,application_date=?5,status=?6,contact_name=?7,contact_phone=?8,contact_email=?9,notes=?10,updated_at=?11 WHERE id=?12", 
                    params![company,
                    location,
                    title,
                    link,
                    self.form.application_date,
                    self.form.status.as_str(),
                    contact_name,
                    contact_phone,
                    contact_email,
                    notes,
                    now,id])
                    .unwrap();
            }
            None => {
                self.db.execute(
                    "INSERT INTO applications(company,location,title,link,application_date,status,contact_name,contact_phone,contact_email,notes,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)", 
                    params![company,
                    location,
                    title,
                    link,
                    self.form.application_date,
                    self.form.status.as_str(),
                    contact_name,
                    contact_phone,
                    contact_email,
                    notes,
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
        self.message = Some("Application saved.".into());
        self.page = Page::Applications;
        self.editing_id = None;
    }

    fn begin_new(&mut self) {
        self.form = Form::default();
        self.editing_id = None;
        self.page = Page::Add;
    }

    fn begin_edit(&mut self, id: i64) {
        if let Some(a) = self.applications.iter().find(|a| a.id == id) {
            self.form = Form {
                company: a.company.clone(),
                location: a.location.clone(),
                title: a.title.clone(),
                link: a.link.clone(),
                application_date: a.application_date.clone(),
                status: a.status,
                contact_name: a.contact_name.clone(),
                contact_phone: a.contact_phone.clone(),
                contact_email: a.contact_email.clone(),
                notes: a.notes.clone(),
            };
            self.editing_id = Some(id);
            self.page = Page::Add;
        }
    }

    fn selected(&self) -> Option<&Application> {
        self.selected_id
            .and_then(|id| self.applications.iter().find(|a| a.id == id))
    }

    fn add_activity(&mut self, id: i64, kind: &str) {
        let kind = match validate_text_field("activity type", kind, 100) {
            Ok(value) => value,
            Err(_) => return,
        };

        self.db.execute(
            "INSERT INTO activities(application_id,activity_type,activity_date,notes) VALUES(?1,?2,?3,'')",
            params![id,kind,today()])
            .unwrap();
        self.reload();
    }

    fn filter_analytics_applications(&self) -> Vec<Application> {
        let from = NaiveDate::parse_from_str(&self.analysis_from, "%Y-%m-%d").ok();
        let to = NaiveDate::parse_from_str(&self.analysis_to, "%Y-%m-%d").ok();

        self.applications
            .iter()
            .filter(|a| {
                if let Ok(app_date) = NaiveDate::parse_from_str(&a.application_date, "%Y-%m-%d") {
                    if let Some(from_date) = from {
                        if app_date < from_date {
                            return false;
                        }
                    }
                    if let Some(to_date) = to {
                        if app_date > to_date {
                            return false;
                        }
                    }
                    true
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }

    fn followups(&self) -> Vec<Application> {
        // cutoff = today - followup_days. Applications with status `Applied` and last_contact
        // (or application_date if no contact) on or before the cutoff are considered for follow-up.
        // Change `self.followup_days` (Settings) to tune sensitivity.
        let cutoff = Local::now().date_naive() - Duration::days(self.followup_days);
        self.applications
            .iter()
            .filter(|a| {
                matches!(a.status, Status::Applied)
                    && NaiveDate::parse_from_str(
                        a.last_contact.as_deref().unwrap_or(&a.application_date),
                        "%Y-%m-%d",
                    )
                    .map(|d| d <= cutoff)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    fn nav(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.heading("Job Hunt Tracker");
            ui.separator();
            for (p, label) in [
                (Page::Dashboard, "Dashboard"),
                (Page::Add, "Add Application"),
                (Page::Applications, "Applications"),
                (Page::FollowUps, "Follow Ups"),
                (Page::Analytics, "Analytics"),
                (Page::Settings, "Settings"),
            ] {
                if ui.selectable_label(self.page == p, label).clicked() {
                    if p == Page::Add {
                        self.begin_new()
                    } else {
                        self.page = p;
                    }
                }
            }
        });
    }

    fn header(&mut self, ui: &mut egui::Ui, title: &str) {
        ui.horizontal(|ui| {
            ui.heading(title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ Add Application").clicked() {
                    self.begin_new();
                }
            });
        });
        ui.separator();
    }

    fn dashboard(&mut self, ui: &mut egui::Ui) {
        self.header(ui, "Dashboard");
        let total = self.applications.len();
        let applied = self
            .applications
            .iter()
            .filter(|a| a.status != Status::NotApplied)
            .count();
        let selected = self
            .applications
            .iter()
            .filter(|a| a.status == Status::Selected)
            .count();
        let rejected = self
            .applications
            .iter()
            .filter(|a| a.status == Status::Rejected)
            .count();
        let f = self.followups();
        ui.columns(4, |cols| {
            for (c, (n, l)) in cols.iter_mut().zip([
                (total, "Total"),
                (applied, "Applied"),
                (selected, "Selected"),
                (rejected, "Rejected"),
            ]) {
                c.vertical_centered(|ui| {
                    ui.label(RichText::new(n.to_string()).size(28.0));
                    ui.label(l);
                });
            }
        });
        ui.add_space(20.0);
        if !f.is_empty() {
            ui.colored_label(
                Color32::from_rgb(180, 80, 40),
                format!(
                    "{} follow up{} due",
                    f.len(),
                    if f.len() == 1 { "" } else { "s" }
                ),
            );
            for a in f.iter().take(8) {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} | {} | {}",
                        a.company, a.title, a.application_date
                    ));
                    if ui.button("Open").clicked() {
                        self.selected_id = Some(a.id);
                        self.page = Page::Detail;
                    }
                });
            }
            ui.separator();
        }
        ui.heading("Quick actions");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Add application").clicked() {
                self.begin_new();
            }
            if ui.button("View applications").clicked() {
                self.page = Page::Applications;
            }
            if ui.button("View analytics").clicked() {
                self.page = Page::Analytics;
            }
        });
    }

    fn add_page(&mut self, ui: &mut egui::Ui) {
        self.header(
            ui,
            if self.editing_id.is_some() {
                "Edit Application"
            } else {
                "Add Application"
            },
        );
        egui::Grid::new("form")
            .num_columns(2)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label("Company *");
                ui.text_edit_singleline(&mut self.form.company);
                ui.end_row();

                ui.label("Location");
                ui.text_edit_singleline(&mut self.form.location);
                ui.end_row();

                ui.label("Job title *");
                ui.text_edit_singleline(&mut self.form.title);
                ui.end_row();

                ui.label("Job link");
                ui.text_edit_singleline(&mut self.form.link);
                ui.end_row();

                // `application_date` is expected in YYYY-MM-DD format. If you change formatting,
                // update parsing and analytics code that reads this field.
                ui.label("Application date");
                ui.text_edit_singleline(&mut self.form.application_date);
                ui.end_row();

                // `Status` selects the workflow stage. The displayed and stored strings are
                // produced by `Status::as_str()`; `Status::from_str()` reads DB values back.
                ui.label("Status");
                egui::ComboBox::from_id_salt("form_status")
                    .selected_text(self.form.status.as_str())
                    .show_ui(ui, |ui| {
                        for s in Status::ALL {
                            ui.selectable_value(&mut self.form.status, s, s.as_str());
                        }
                    });
                ui.end_row();

                ui.label("Contact name");
                ui.text_edit_singleline(&mut self.form.contact_name);
                ui.end_row();

                ui.label("Contact phone");
                ui.text_edit_singleline(&mut self.form.contact_phone);
                ui.end_row();

                ui.label("Contact email");
                ui.text_edit_singleline(&mut self.form.contact_email);
                ui.end_row();

                ui.label("Notes");
                ui.text_edit_multiline(&mut self.form.notes);
                ui.end_row();
            });

        ui.add_space(12.0);
        if ui.button("Save").clicked() {
            self.save_form();
        }
        if let Some(m) = &self.message {
            ui.label(m);
        }
    }

    fn list_page(&mut self, ui: &mut egui::Ui) {
        self.header(ui, "Applications");

        ui.horizontal(|ui| {
            ui.label("Search");
            ui.text_edit_singleline(&mut self.search);
            ui.label("Status");
            egui::ComboBox::from_id_salt("filter_status")
                .selected_text(self.status_filter.map(|s| s.as_str()).unwrap_or("All"))
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(self.status_filter.is_none(), "All")
                        .clicked()
                    {
                        self.status_filter = None;
                    }
                    for s in Status::ALL {
                        ui.selectable_value(&mut self.status_filter, Some(s), s.as_str());
                    }
                });
            ui.label("Location");
            ui.text_edit_singleline(&mut self.location_filter);
        });

        ui.separator();

        let q = self.search.to_lowercase();
        let lf = self.location_filter.to_lowercase();
        let rows: Vec<Application> = self
            .applications
            .iter()
            .filter(|a| {
                (q.is_empty()
                    || a.company.to_lowercase().contains(&q)
                    || a.title.to_lowercase().contains(&q)
                    || a.contact_name.to_lowercase().contains(&q))
                    && (lf.is_empty() || a.location.to_lowercase().contains(&lf))
                    && (self.status_filter.is_none() || Some(a.status) == self.status_filter)
            })
            .cloned()
            .collect();

        // Limits for visible characters per column. These keep the grid tidy while
        // allowing horizontal scrolling for full content (via hover/tooltip if desired).
        const COMPANY_MAX: usize = 40;
        const TITLE_MAX: usize = 60;
        const LOCATION_MAX: usize = 30;
        const STATUS_MAX: usize = 18;

        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("apps")
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    for h in ["Company", "Job", "Location", "Date", "Status", "Actions"] {
                        ui.strong(h);
                    }
                    ui.end_row();

                    for a in rows {
                        ui.add(egui::Label::new(truncate(&a.company, COMPANY_MAX))).on_hover_text(&a.company);
                        ui.add(egui::Label::new(truncate(&a.title, TITLE_MAX))).on_hover_text(&a.title);
                        ui.add(egui::Label::new(truncate(&a.location, LOCATION_MAX))).on_hover_text(&a.location);
                        ui.label(&a.application_date);
                        ui.add(egui::Label::new(truncate(a.status.as_str(), STATUS_MAX))).on_hover_text(a.status.as_str());
                        ui.horizontal(|ui| {
                            if ui.button("Open").clicked() {
                                self.selected_id = Some(a.id);
                                self.page = Page::Detail;
                            }
                            if ui.button("Edit").clicked() {
                                self.begin_edit(a.id);
                            }
                            if ui.button("Delete").clicked() {
                                self.db
                                    .execute("DELETE FROM applications WHERE id=?1", params![a.id])
                                    .unwrap();
                                self.reload();
                            }
                        });
                        ui.end_row();
                    }
                });
        });
    }

    fn detail_page(&mut self, ui: &mut egui::Ui) {
        let Some(a) = self.selected().cloned() else {
            self.page = Page::Applications;
            return;
        };
        self.header(ui, "Application Detail");
        ui.heading(format!("{} — {}", a.company, a.title));
        ui.label(format!("Location: {}", a.location));
        ui.label(format!("Applied: {}", a.application_date));
        ui.label(format!("Status: {}", a.status.as_str()));
        if !a.link.is_empty() && ui.button("Open job link").clicked() {
            let _ = open::that(&a.link);
        }
        ui.separator();
        ui.heading("Contact");
        ui.label(format!(
            "{} | {} | {}",
            if a.contact_name.is_empty() {
                "No name"
            } else {
                &a.contact_name
            },
            if a.contact_phone.is_empty() {
                "No phone"
            } else {
                &a.contact_phone
            },
            if a.contact_email.is_empty() {
                "No email"
            } else {
                &a.contact_email
            }
        ));
        ui.horizontal(|ui| {
            for k in [
                "Email sent",
                "Phone call made",
                "Reminder follow up",
                "Interview",
            ] {
                if ui.button(k).clicked() {
                    self.add_activity(a.id, k);
                }
            }
        });
        ui.separator();
        ui.heading("History");
        let mut stmt=self.db.prepare("SELECT activity_date,activity_type,notes FROM activities WHERE application_id=?1 ORDER BY activity_date,id").unwrap();
        let it = stmt
            .query_map(params![a.id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .unwrap();
        for x in it.filter_map(Result::ok) {
            ui.label(format!("{}  |  {}  {}", x.0, x.1, x.2));
        }
        if !a.notes.is_empty() {
            ui.separator();
            ui.heading("Notes");
            ui.label(a.notes);
        }
    }

    fn followup_page(&mut self, ui: &mut egui::Ui) {
        self.header(ui, "Follow Ups");
        let f = self.followups();
        ui.label(format!(
            "No contact for {} days or more: {} application(s)",
            self.followup_days,
            f.len()
        ));
        egui::ScrollArea::both().show(ui, |ui| {
            for a in f {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong(format!("{} — {}", a.company, a.title));
                    ui.label(format!(
                        "last contact: {}",
                        a.last_contact.unwrap_or(a.application_date)
                    ));
                    if ui.button("Open").clicked() {
                        self.selected_id = Some(a.id);
                        self.page = Page::Detail;
                    }
                });
            }
        });
    }

    fn analytics_page(&mut self, ui: &mut egui::Ui) {
        self.header(ui, "Analytics");

        ui.horizontal(|ui| {
            ui.label("From:");
            ui.text_edit_singleline(&mut self.analysis_from);
            ui.label("To:");
            ui.text_edit_singleline(&mut self.analysis_to);
            if ui.button("Clear").clicked() {
                self.analysis_from.clear();
                self.analysis_to.clear();
            }
        });

        egui::ScrollArea::both().show(ui, |ui| {
            let filtered = self.filter_analytics_applications();
            let total = filtered
                .iter()
                .filter(|a| a.status != Status::NotApplied)
                .count();
            ui.label(format!("{} applications submitted", total));

            ui.heading("Funnel");
            let stages = [
                (
                    "Applications",
                    filtered
                        .iter()
                        .filter(|a| a.status != Status::NotApplied)
                        .count(),
                ),
                (
                    "First Call",
                    filtered
                        .iter()
                        .filter(|a| {
                            matches!(
                                a.status,
                                Status::FirstCallTelephone
                                    | Status::FirstCallVideo
                                    | Status::FurtherRounds
                                    | Status::Selected
                            )
                        })
                        .count(),
                ),
                (
                    "Further Rounds",
                    filtered
                        .iter()
                        .filter(|a| matches!(a.status, Status::FurtherRounds | Status::Selected))
                        .count(),
                ),
                (
                    "Selected",
                    filtered
                        .iter()
                        .filter(|a| a.status == Status::Selected)
                        .count(),
                ),
            ];
            let max = stages.iter().map(|x| x.1).max().unwrap_or(1).max(1) as f32;
            for (name, n) in stages {
                ui.horizontal(|ui| {
                    ui.label(format!("{:<16}", name));
                    let bar_width = ui.available_width() * ((n as f32) / max).max(0.02);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(bar_width, 28.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 4.0, ui.visuals().selection.bg_fill);
                    ui.painter().rect_stroke(
                        rect,
                        4.0,
                        ui.visuals().widgets.active.bg_stroke,
                        egui::StrokeKind::Inside,
                    );
                    ui.label(n.to_string());
                });
            }
            ui.separator();
            ui.heading("By status");
            egui::Grid::new("analytics_status_grid").striped(true).show(ui, |ui| {
                for s in Status::ALL {
                    let n = filtered.iter().filter(|a| a.status == s).count();
                    ui.label(s.as_str());
                    ui.label(RichText::new(n.to_string()).monospace());
                    ui.end_row();
                }
            });

            ui.separator();
            ui.heading("By location");
            let mut map = std::collections::BTreeMap::<String, usize>::new();
            for a in &filtered {
                *map.entry(if a.location.is_empty() {
                    "Unknown".into()
                } else {
                    a.location.clone()
                })
                .or_default() += 1;
            }
            egui::Grid::new("analytics_location_grid").striped(true).show(ui, |ui| {
                for (k, v) in map {
                    ui.label(k);
                    ui.label(RichText::new(v.to_string()).monospace());
                    ui.end_row();
                }
            });

            ui.separator();
            ui.heading("By company");
            let mut company_map = std::collections::BTreeMap::<String, usize>::new();
            for a in &filtered {
                *company_map.entry(a.company.clone()).or_default() += 1;
            }
            egui::Grid::new("analytics_company_grid").striped(true).show(ui, |ui| {
                for (k, v) in company_map {
                    ui.label(k);
                    ui.label(RichText::new(v.to_string()).monospace());
                    ui.end_row();
                }
            });

            ui.separator();
            ui.heading("Applications by date");
            let mut date_map = std::collections::BTreeMap::<String, usize>::new();
            for a in &filtered {
                *date_map.entry(a.application_date.clone()).or_default() += 1;
            }
            egui::Grid::new("analytics_date_grid").striped(true).show(ui, |ui| {
                for (k, v) in date_map {
                    ui.label(k);
                    ui.label(RichText::new(v.to_string()).monospace());
                    ui.end_row();
                }
            });
        });
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        self.header(ui, "Settings");
        ui.horizontal(|ui| {
            ui.label("Follow up after");
            ui.add(egui::DragValue::new(&mut self.followup_days).range(1..=90));
            ui.label("days");
        });
        ui.separator();
        ui.label(format!("Database: {}", db_path().display()));
        ui.label("The database is local SQLite data. Back it up by copying the jobs.db file while the application is closed.");
        ui.separator();
        ui.heading("About");
        ui.label(format!("Author: {}", env!("CARGO_PKG_AUTHORS")));
        ui.label(format!("Version: {}", env!("CARGO_PKG_VERSION")));
        ui.label("© 2026 Kushal Prakash. All rights reserved.");
    }
    // `followup_days` UI uses a drag value constrained to 1..=90 days. Adjust the range here
    // if you want to allow longer or shorter follow-up periods.
}

impl eframe::App for App {
    fn ui(&mut self, ctx: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("nav")
            .resizable(false)
            .default_size(180.0)
            .show(ctx, |ui| self.nav(ui));
        egui::CentralPanel::default().show(ctx, |ui| match self.page {
            Page::Dashboard => self.dashboard(ui),
            Page::Add => self.add_page(ui),
            Page::Applications => self.list_page(ui),
            Page::Detail => self.detail_page(ui),
            Page::FollowUps => self.followup_page(ui),
            Page::Analytics => self.analytics_page(ui),
            Page::Settings => self.settings_page(ui),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_control_characters() {
        let value = sanitize_text("  some\u{0000}text\n  ", 50);
        assert_eq!(value, "sometext");
    }

    #[test]
    fn rejects_sql_injection_patterns() {
        let err = validate_text_field("company", "'; DROP TABLE applications; --", 200).unwrap_err();
        assert!(err.contains("Invalid company value detected"));
    }
}

fn main() -> eframe::Result {
    // Window sizing: `with_inner_size` sets the starting window size (1200x760).
    // `with_min_inner_size` prevents resizing smaller than 900x600. Tweak these values
    // to change the app's initial or minimum dimensions.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([900.0, 600.0])
            .with_icon(Arc::new(app_icon_data())),
        ..Default::default()
    };
    eframe::run_native(
        "Job Hunt Tracker",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
