# Job Hunt Tracker

A desktop app for tracking job applications, follow-ups, and progress across the hiring process.

## Features

- Track job applications with company, role, location, link, and date
- Save contact details and notes for each application
- Monitor status across stages such as applied, interview, follow-up, and rejected
- View dashboard summaries and analytics
- Keep local SQLite data for each user
- Runs as a native desktop app on macOS

## Tech Stack

- Rust
- eframe / egui for the desktop UI
- rusqlite with bundled SQLite database support
- chrono for date handling
- open for opening links in the default browser

## Requirements

- Rust 1.85+ (or a current stable version compatible with this project)
- Cargo
- macOS (the app is configured for native macOS desktop use)
- Optional: Xcode Command Line Tools if your environment needs them

## Install Rust

If Rust is not installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Then verify:

```bash
rustc --version
cargo --version
```

## Clone the project

```bash
git clone <your-repo-url>
cd job_hunt_tracker
```

## Run the app locally

```bash
cargo run
```

For a production build:

```bash
cargo build --release
./target/release/job_hunt_tracker
```

## Build a standalone macOS app bundle

This app is set up to bundle into a macOS app and DMG using cargo-bundle.

Install the bundler:

```bash
cargo install cargo-bundle
```

Then build the app bundle:

```bash
cargo bundle --release
```

The output will be created in:

```bash
target/release/bundle/osx/
```

and the DMG in:

```bash
target/release/bundle/dmg/
```

## Project structure

```text
job_hunt_tracker/
├── Cargo.toml
├── README.md
├── icons/
│   └── icon.png
├── src/
│   └── main.rs
└── target/
```

## Database

The app stores data in a local SQLite database at:

```bash
~/Library/Application Support/JobHuntTracker/jobs.db
```

The database is created automatically when the app starts if it does not already exist.

## Notes

- The app icon is configured in Cargo bundle metadata and also set at runtime for the native window.
- The app uses a local SQLite database rather than a server, so no external database service is required.

## License

This project is currently provided as-is for personal or local use.
