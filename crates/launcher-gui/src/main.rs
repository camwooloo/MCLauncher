//! iced front-end for the Minecraft launcher.
//!
//! Ties `launcher-core` together into a usable app: pick an account (offline or
//! Microsoft), pick a loader + version, set memory, and Play. Installation
//! progress is shown via a shared reporter polled by a timer subscription.

// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;
mod shared;
mod tasks;

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use iced::widget::{
    button, column, container, horizontal_rule, pick_list, progress_bar, row, scrollable, slider,
    text, text_input, Space,
};
use iced::{Element, Length, Subscription, Task};

use launcher_core::account::Account;
use launcher_core::paths::Paths;

use settings::Settings;
use shared::Shared;
use tasks::PlayRequest;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,launcher_core=info".into()),
        )
        .init();

    iced::application("MCLauncher", App::update, App::view)
        .subscription(App::subscription)
        .window_size((760.0, 620.0))
        .run_with(App::new)
}

/// The supported loaders (Forge pending — see `launcher_core::modloader`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loader {
    Vanilla,
    Fabric,
    Quilt,
}

impl Loader {
    const ALL: [Loader; 3] = [Loader::Vanilla, Loader::Fabric, Loader::Quilt];

    fn from_label(s: &str) -> Loader {
        match s {
            "Fabric" => Loader::Fabric,
            "Quilt" => Loader::Quilt,
            _ => Loader::Vanilla,
        }
    }
}

impl fmt::Display for Loader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Loader::Vanilla => "Vanilla",
            Loader::Fabric => "Fabric",
            Loader::Quilt => "Quilt",
        };
        f.write_str(s)
    }
}

struct App {
    paths: Paths,
    shared: Arc<Shared>,
    settings: Settings,

    versions: Vec<String>,
    selected_version: Option<String>,
    selected_loader: Loader,

    account: Option<Account>,
    username_input: String,
    client_id_input: String,

    busy: bool,
    status: String,
}

#[derive(Debug, Clone)]
enum Message {
    Loaded(SettingsBundle),
    ReleasesLoaded(Result<Vec<String>, String>),
    LoaderSelected(Loader),
    VersionSelected(String),
    MemoryChanged(u32),
    UsernameChanged(String),
    ClientIdChanged(String),
    AddOffline,
    LoginMicrosoft,
    LoginFinished(Result<(Account, String), String>),
    Logout,
    Play,
    PlayFinished(Result<String, String>),
    Tick,
    Noop,
}

/// Bundle of state loaded asynchronously at startup.
#[derive(Debug, Clone)]
struct SettingsBundle {
    settings: Settings,
    account: Option<Account>,
}

impl App {
    fn new() -> (App, Task<Message>) {
        let paths = Paths::discover()
            .unwrap_or_else(|_| Paths::with_dirs("./.minecraft", "./mclauncher-data"));

        let app = App {
            paths: paths.clone(),
            shared: Arc::new(Shared::default()),
            settings: Settings::default(),
            versions: Vec::new(),
            selected_version: None,
            selected_loader: Loader::Vanilla,
            account: None,
            username_input: "Player".to_string(),
            client_id_input: String::new(),
            busy: false,
            status: "Loading…".to_string(),
        };

        let load = Task::perform(load_initial(paths), Message::Loaded);
        let releases = Task::perform(tasks::load_releases(), Message::ReleasesLoaded);
        (app, Task::batch([load, releases]))
    }

    fn subscription(&self) -> Subscription<Message> {
        // Poll the shared reporter while a background op is running.
        if self.busy {
            iced::time::every(Duration::from_millis(100)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(bundle) => {
                self.settings = bundle.settings;
                self.client_id_input = self.settings.azure_client_id.clone();
                self.selected_loader = Loader::from_label(&self.settings.last_loader);
                if !self.settings.last_version.is_empty() {
                    self.selected_version = Some(self.settings.last_version.clone());
                }
                if let Some(acct) = bundle.account {
                    self.username_input = acct.username.clone();
                    self.account = Some(acct);
                }
                if self.status == "Loading…" {
                    self.status = "Ready".to_string();
                }
                Task::none()
            }
            Message::ReleasesLoaded(Ok(versions)) => {
                if self.selected_version.is_none() {
                    self.selected_version = versions.first().cloned();
                }
                self.versions = versions;
                Task::none()
            }
            Message::ReleasesLoaded(Err(e)) => {
                self.status = format!("Failed to load versions: {e}");
                Task::none()
            }
            Message::LoaderSelected(loader) => {
                self.selected_loader = loader;
                self.settings.last_loader = loader.to_string();
                self.save_settings()
            }
            Message::VersionSelected(v) => {
                self.settings.last_version = v.clone();
                self.selected_version = Some(v);
                self.save_settings()
            }
            Message::MemoryChanged(mb) => {
                self.settings.max_memory_mb = mb;
                Task::none()
            }
            Message::UsernameChanged(name) => {
                self.username_input = name;
                Task::none()
            }
            Message::ClientIdChanged(id) => {
                self.client_id_input = id.clone();
                self.settings.azure_client_id = id;
                Task::none()
            }
            Message::AddOffline => {
                let name = self.username_input.trim();
                if name.is_empty() {
                    self.status = "Enter a username first".to_string();
                    return Task::none();
                }
                let account = Account::offline(name);
                self.account = Some(account.clone());
                self.status = format!("Using offline account “{}”", account.username);
                Task::perform(
                    tasks::persist_account(self.paths.clone(), account, String::new()),
                    |_| Message::Noop,
                )
            }
            Message::LoginMicrosoft => {
                let client_id = self.client_id_input.trim().to_string();
                if client_id.is_empty() {
                    self.status =
                        "Enter your Azure application (client) id to use Microsoft login"
                            .to_string();
                    return Task::none();
                }
                self.busy = true;
                self.shared.begin("Signing in to Microsoft");
                self.status = "Signing in…".to_string();
                let save = self.save_settings();
                let login = Task::perform(
                    tasks::login(client_id, self.shared.clone()),
                    Message::LoginFinished,
                );
                Task::batch([save, login])
            }
            Message::LoginFinished(Ok((account, refresh_token))) => {
                self.busy = false;
                self.shared.clear_login_prompt();
                self.status = format!("Signed in as {}", account.username);
                self.account = Some(account.clone());
                Task::perform(
                    tasks::persist_account(self.paths.clone(), account, refresh_token),
                    |_| Message::Noop,
                )
            }
            Message::LoginFinished(Err(e)) => {
                self.busy = false;
                self.shared.clear_login_prompt();
                self.status = format!("Login failed: {e}");
                Task::none()
            }
            Message::Logout => {
                self.account = None;
                self.status = "Signed out".to_string();
                Task::none()
            }
            Message::Play => {
                let Some(version) = self.selected_version.clone() else {
                    self.status = "Select a version first".to_string();
                    return Task::none();
                };
                let account = self.account.clone().unwrap_or_else(|| {
                    Account::offline(self.username_input.trim())
                });
                self.busy = true;
                self.shared.begin("Preparing");
                self.status = format!("Installing {version}…");

                let req = PlayRequest {
                    paths: self.paths.clone(),
                    loader: self.selected_loader,
                    game_version: version,
                    account,
                    max_memory_mb: self.settings.max_memory_mb,
                    shared: self.shared.clone(),
                };
                let save = self.save_settings();
                let play = Task::perform(tasks::play(req), Message::PlayFinished);
                Task::batch([save, play])
            }
            Message::PlayFinished(Ok(msg)) => {
                self.busy = false;
                self.status = msg;
                Task::none()
            }
            Message::PlayFinished(Err(e)) => {
                self.busy = false;
                self.status = format!("Error: {e}");
                Task::none()
            }
            Message::Tick | Message::Noop => Task::none(),
        }
    }

    fn save_settings(&self) -> Task<Message> {
        let settings = self.settings.clone();
        let path = self.paths.settings_file();
        Task::perform(async move { settings.save(&path).await }, |_| Message::Noop)
    }

    fn view(&self) -> Element<'_, Message> {
        let title = text("MCLauncher").size(30);

        // --- Account section ---------------------------------------------
        let account_section: Element<Message> = match &self.account {
            Some(acct) => row![
                text(format!(
                    "Account: {}  ({})",
                    acct.username,
                    if acct.is_online() { "Microsoft" } else { "offline" }
                )),
                Space::with_width(Length::Fill),
                button("Sign out").on_press(Message::Logout),
            ]
            .spacing(10)
            .align_y(iced::Center)
            .into(),
            None => column![
                text("No account").size(16),
                row![
                    text_input("Offline username", &self.username_input)
                        .on_input(Message::UsernameChanged)
                        .width(Length::Fixed(200.0)),
                    button("Use offline").on_press(Message::AddOffline),
                ]
                .spacing(8),
                row![
                    text_input("Azure client id", &self.client_id_input)
                        .on_input(Message::ClientIdChanged)
                        .width(Length::Fixed(330.0)),
                    self.maybe_press(button("Sign in with Microsoft"), Message::LoginMicrosoft),
                ]
                .spacing(8),
            ]
            .spacing(8)
            .into(),
        };

        // --- Version / loader section ------------------------------------
        let loader_pick = pick_list(
            &Loader::ALL[..],
            Some(self.selected_loader),
            Message::LoaderSelected,
        );
        let version_pick = pick_list(
            self.versions.clone(),
            self.selected_version.clone(),
            Message::VersionSelected,
        )
        .placeholder("version");

        let config_row = row![
            column![text("Loader").size(12), loader_pick].spacing(4),
            column![text("Version").size(12), version_pick].spacing(4),
        ]
        .spacing(20);

        // --- Memory ------------------------------------------------------
        let memory = column![
            text(format!("Max memory: {} MiB", self.settings.max_memory_mb)).size(12),
            slider(512..=16384, self.settings.max_memory_mb, Message::MemoryChanged).step(512u32),
        ]
        .spacing(4);

        // --- Play + progress ---------------------------------------------
        let play_button = {
            let b = button(text(if self.busy { "Working…" } else { "Play" }).size(18)).padding(12);
            if self.busy {
                b
            } else {
                b.on_press(Message::Play)
            }
        };

        let mut progress_area = column![].spacing(6);
        if self.busy {
            let stage = self.shared.current_stage();
            let frac = self.shared.fraction();
            progress_area = progress_area
                .push(text(stage).size(13))
                .push(progress_bar(0.0..=1.0, frac));
            if let Some((code, uri)) = self.shared.login_prompt() {
                progress_area = progress_area.push(
                    container(
                        column![
                            text("Microsoft sign-in").size(14),
                            text(format!("1. Open: {uri}")),
                            text(format!("2. Enter code: {code}")).size(18),
                        ]
                        .spacing(4),
                    )
                    .padding(10),
                );
            }
        }

        let status = text(&self.status).size(13);

        let content = column![
            title,
            horizontal_rule(1),
            account_section,
            horizontal_rule(1),
            config_row,
            memory,
            Space::with_height(Length::Fixed(8.0)),
            play_button,
            progress_area,
            Space::with_height(Length::Fixed(12.0)),
            horizontal_rule(1),
            status,
        ]
        .spacing(14);

        container(scrollable(content))
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// A button that is enabled only when not busy.
    fn maybe_press<'a>(
        &self,
        b: button::Button<'a, Message>,
        msg: Message,
    ) -> button::Button<'a, Message> {
        if self.busy {
            b
        } else {
            b.on_press(msg)
        }
    }
}

/// Load settings + the active account at startup.
async fn load_initial(paths: Paths) -> SettingsBundle {
    use launcher_core::account::AccountStore;
    let settings = Settings::load(&paths.settings_file()).await;
    let store = AccountStore::load(&paths.accounts_file())
        .await
        .unwrap_or_default();
    let account = store.active().map(|a| a.account.clone());
    SettingsBundle { settings, account }
}
