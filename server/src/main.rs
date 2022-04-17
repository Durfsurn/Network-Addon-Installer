#![allow(non_snake_case)]
mod files;

use std::{fs, thread::sleep, time::Duration};

use iced::{
    button, executor, text_input, window, Align, Application, Button, Checkbox, Clipboard, Column,
    Command, Element, HorizontalAlignment, Row, Settings, Subscription, Text, TextInput,
    VerticalAlignment,
};
use os_info::Type;

struct Config {
    version: String,
    installer_files: files::InstallerOption,
}
impl Config {
    fn init(installer_files: files::InstallerOption) -> Self {
        Config {
            version: "42.1".into(),
            installer_files,
        }
    }
}

const FOUR_GB: &[u8] = include_bytes!("../static/4gb_patch.exe");
const FAVICON: &[u8] = include_bytes!("../static/favicon.rgba");

pub fn main() -> anyhow::Result<()> {
    let installer_files = files::temp_files()?;

    let mut window_settings = Settings::with_flags(Config::init(installer_files));
    window_settings.window.icon = Some(window::Icon::from_rgba(FAVICON.to_vec(), 256, 256)?);
    window_settings.window.size = (1600, 900);

    Ok(Model::run(window_settings)?)
}

struct Model {
    err: Vec<String>,
    exe_path: String,
    exe_btn: button::State,
    patch_btn: button::State,
    exe_input: text_input::State,
    title: String,
    loading: bool,
    patched: bool,
    os: Type,
    installation: files::InstallerOption,
}

impl Model {
    fn set_err<T, E>(&mut self, err: Result<T, E>)
    where
        E: ToString,
    {
        match err {
            Ok(_) => (),
            Err(e) => {
                self.err.push(e.to_string());
            }
        };
    }
}

#[derive(Debug, Clone)]
enum Message {
    ChangeExePath(String),
    SelectExe(String),
    PatchExe(String, Result<(), String>),
    ToggledCheckbox(bool),
}

impl Application for Model {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = Config;

    fn new(flags: Self::Flags) -> (Self, Command<Message>) {
        let os = os_info::get().os_type();
        (
            Model {
                title: format!(
                    "Network Addon Mod Installer v{} ({})",
                    flags.version,
                    os.to_string()
                ),
                err: Vec::new(),
                exe_path: String::default(),
                exe_btn: button::State::default(),
                patch_btn: button::State::default(),
                exe_input: text_input::State::default(),
                loading: bool::default(),
                patched: bool::default(),
                os,
                installation: flags.installer_files,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn update(&mut self, message: Message, _clipboard: &mut Clipboard) -> Command<Message> {
        match message {
            Message::ChangeExePath(s) => {
                self.exe_path = s;
                Command::none()
            }
            Message::SelectExe(path) => {
                let path = if path.is_empty() {
                    rfd::FileDialog::new()
                        .pick_file()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or(self.exe_path.clone())
                } else {
                    path
                };

                self.exe_path = path.clone();

                self.loading = true;

                Command::perform(tokio::spawn(async {}), move |b| {
                    Message::PatchExe(path.clone(), b.map_err(|e| e.to_string()))
                })
            }
            Message::PatchExe(path, _) => {
                if self.os == os_info::Type::Windows {
                    let uuid = uuid::Uuid::new_v4().to_hyphenated().to_string()[0..8].to_string();
                    let home = directories::UserDirs::new()
                        .map(|h| h.home_dir().to_string_lossy().to_string())
                        .unwrap_or_default();

                    let file_path = format!("{}/Downloads/{}-4gb_patch.exe", home, uuid);

                    let w = fs::write(&file_path, FOUR_GB);
                    self.set_err(w);

                    let out = std::process::Command::new(
                        "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                    )
                    .arg(&file_path)
                    .arg(format!("'{}'", path))
                    .output();

                    self.set_err(out);

                    sleep(Duration::from_secs(1));

                    let r = fs::remove_file(&file_path);
                    self.set_err(r);
                };
                self.loading = false;
                self.patched = true;
                Command::none()
            }
            Message::ToggledCheckbox(_) => Command::none(),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    fn view(&mut self) -> Element<Message> {
        if self.loading {
            Text::new("Loading")
                .vertical_alignment(VerticalAlignment::Center)
                .horizontal_alignment(HorizontalAlignment::Center)
                .into()
        } else {
            let exe_path = Row::new()
                .spacing(5)
                .padding(10)
                .align_items(Align::Center)
                .push(
                    TextInput::new(
                        &mut self.exe_input,
                        "SimCity 4 Executable Location",
                        &self.exe_path,
                        Message::ChangeExePath,
                    )
                    .padding(2)
                    .size(30),
                )
                .push(
                    Button::new(&mut self.exe_btn, Text::new("Select Location"))
                        .on_press(Message::SelectExe(String::new())),
                )
                .push({
                    let btn = Button::new(&mut self.patch_btn, Text::new("Patch Exe"));
                    if self.patched {
                        btn
                    } else {
                        btn.on_press(Message::SelectExe(self.exe_path.clone()))
                    }
                });

            let page = Column::new();
            let page = if self.os.to_string().as_str() == "Windows" {
                page.push(exe_path)
            } else {
                page
            };

            let option_tree = map_installation_options(&self.installation);

            let page = page
                .push(
                    // file option tree
                    Row::new().push(Column::new().push(option_tree)),
                )
                .push(
                    // image/text pane
                    Row::new(),
                );

            let error = Row::new().push(Text::new(format!("Error: {}", self.err.join("\n"))));
            page
                // error always last
                .push(error)
                .into()
        }
    }
}

fn map_installation_options(installation: &files::InstallerOption) -> Element<'static, Message> {
    let children: Vec<_> = installation
        .children
        .iter()
        .map(map_installation_options)
        .map(std::convert::Into::into)
        .collect();

    let heading = Checkbox::new(
        installation.checked(),
        installation.name.to_string(),
        Message::ToggledCheckbox,
    );

    Column::new()
        .push(heading)
        .push(Column::with_children(children).spacing(installation.depth * 20))
        .into()
}
