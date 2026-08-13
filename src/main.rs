
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

fn main() {
    println!("Hello, world!");
}
