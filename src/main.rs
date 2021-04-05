use anyhow::anyhow;
use log::LevelFilter;
use log::{debug, error, info, warn};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use os_info;
use percent_encoding;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{env, str::FromStr};
use walkdir::WalkDir;
use warp::{http::Response, Filter};

#[cfg(target_pointer_width = "32")]
use std::io::{Cursor, Read};

#[cfg(target_pointer_width = "64")]
#[derive(RustEmbed)]
#[folder = "installation/"]
struct InstallAsset;

type Result<T> = std::result::Result<T, warp::Rejection>;

const BULMA: &[u8] = include_bytes!("../static/bulma.css");
const JS: &[u8] = include_bytes!("../static/main.js");
const INDEX_TEMPLATE: &str = include_str!("../static/index.html.hbs");
const FAVICON_PNG: &[u8] = include_bytes!("../static/favicon.png");
const FAVICON_ICO: &[u8] = include_bytes!("../static/favicon.ico");
const CONFIG: &str = include_str!("../configuration.json");
const FOUR_GB: &[u8] = include_bytes!("../static/4gb_patch.exe");
const CLEANUP: &str = include_str!("../static/cleanup.txt");
#[cfg(target_pointer_width = "32")]
const WIN32_BIT_7Z: &[u8] = include_bytes!("../installation.zip");

fn rust_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Configuration {
    title: String,
    #[serde(default = "rust_version")]
    rust_version: String,
    nam_version: String,
    web_server_port: u16,
    #[serde(default)]
    windows: String,
}

fn calculate_folders(strs: &mut Vec<String>) -> Vec<String> {
    let as_paths = strs.iter().map(|s| Path::new(s));
    let mut folders: Vec<String> = as_paths
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let full_path = p.to_string_lossy().to_string();
            full_path.replace(&name, "")
        })
        .collect();
    folders.sort();
    folders.dedup();
    folders.append(strs);
    folders
        .iter()
        .filter(|f| f.len() > 0)
        .map(|f| f.to_string())
        .collect::<Vec<String>>()
}

// State Checkers to prevent spoofed http calls from causing a mangled installation, including making sure windows sc4 is patched
static mut CHECKED_EXE: bool = false;
static mut PATCHED_EXE: bool = false;

// List of cleaned files
static mut CLEANED_FILE_COUNT: usize = 0;
static mut CLEANED_FILE_MAX: usize = 0;
static mut CLEANED_FILE_LIST: Vec<String> = Vec::new();
// List of installed files
static mut INSTALLED_FILE_COUNT: usize = 0;
static mut INSTALLED_FILE_MAX: usize = 0;
static mut INSTALLED_FILE_LIST: Vec<String> = Vec::new();

#[derive(Clone, Debug)]
struct InstallAssetList {
    list: Vec<String>,
}
impl InstallAssetList {
    fn get_file(&self, f: &str) -> std::option::Option<std::borrow::Cow<'static, [u8]>> {
        InstallAsset::get(f)
    }

    fn to_vec(self) -> Vec<String> {
        self.list
    }

    fn filter_images(self) -> Self {
        InstallAssetList {
            list: self
                .list
                .iter()
                .filter(|f| f.contains(".jpg") || f.contains(".png"))
                .map(|f| f.to_owned())
                .collect(),
        }
    }
    fn filter_docs(self) -> Self {
        InstallAssetList {
            list: self
                .list
                .iter()
                .filter(|f| f.contains(".txt"))
                .map(|f| f.to_owned())
                .collect(),
        }
    }
}
#[cfg(target_pointer_width = "64")]
async fn get_install_asset_list() -> anyhow::Result<InstallAssetList> {
    Ok(InstallAssetList {
        list: WalkDir::new("installation/")
            .into_iter()
            .map(|f| f.unwrap().path().to_string_lossy().to_string())
            .collect(),
    })
}

#[cfg(target_pointer_width = "32")]
async fn get_install_asset_list() -> anyhow::Result<InstallAssetList> {
    let uuid = uuid::Uuid::new_v4().to_hyphenated().to_string()[0..8].to_string();
    std::fs::create_dir_all(&format!("C:/temp/{}", uuid))?;

    let reader = Cursor::new(WIN32_BIT_7Z.to_vec());

    let mut zip = zip::ZipArchive::new(reader)?;

    for index in 0..zip.len() {
        let mut file = zip.by_index(index)?;
        let name = file.name();
        if name.ends_with('/') {
            std::fs::create_dir_all(format!("C:/temp/{}/{}", uuid, &file.name()))?;
        } else {
            let mut output = std::fs::File::create(&format!("C:/temp/{}/{}", uuid, &file.name()))?;
            std::io::copy(&mut file, &mut output)?;
        }
    }
    InstallAssetList {
        list: WalkDir::new(&format!("C:/temp/{}", uuid))
            .into_iter()
            .map(|f| f.unwrap().path().to_string_lossy().to_string())
            .collect(),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut term = term::stdout().unwrap();
    term.fg(term::color::RED).unwrap();
    println!("DO NOT CLOSE THIS WINDOW!");
    term.fg(term::color::MAGENTA).unwrap();
    println!("IT IS ESSENTIAL FOR NAM INSTALLATION!");
    term.fg(term::color::RED).unwrap();
    println!("DO NOT CLOSE THIS WINDOW!");
    term.fg(term::color::MAGENTA).unwrap();
    println!("IT IS ESSENTIAL FOR NAM INSTALLATION!");

    let mut config: Configuration = serde_json::from_str(CONFIG)?;

    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
        .build(format!("NAM Installer v{}.log", &config.nam_version))?;

    let b_config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;

    log4rs::init_config(b_config)?;
    let os_name = os_info::get().os_type().to_string().to_lowercase();
    let os_bit = os_info::get().bitness().to_string();
    let windows = if os_info::get().os_type().to_string().to_lowercase() == "windows" {
        format!("{} {}", os_name, os_bit)
    } else {
        os_name
    };

    config.windows = windows;

    println!("\nStarting to unzip files. Do not close this window.");
    let asset_iter = get_install_asset_list().await?;
    println!("Finished unzipping files. Do not close this window.\n");

    let file_list: Vec<String> = calculate_folders(&mut asset_iter.clone().to_vec());

    let uuid = uuid::Uuid::new_v4().to_hyphenated().to_string()[0..8].to_string();

    for folder in file_list
        .iter()
        .filter(|f| match std::path::PathBuf::from_str(f) {
            Ok(a) => a.is_dir(),
            Err(e) => {
                info!("{}", e.to_string());
                false
            }
        })
    {
        std::fs::create_dir_all(format!("{}/{}", uuid, folder))?;
    }

    let folder_structure = [folder_structure(&format!("{}/", uuid))?].to_vec();

    let arc_folder_structure = std::sync::Arc::new(folder_structure.clone());

    let arc_asset_list: std::sync::Arc<InstallAssetList> = std::sync::Arc::new(asset_iter.clone());

    let arc_docs_list: std::sync::Arc<InstallAssetList> =
        std::sync::Arc::new(asset_iter.clone().filter_docs());
    let arc_images_list: std::sync::Arc<InstallAssetList> =
        std::sync::Arc::new(asset_iter.clone().filter_images());

    std::fs::remove_dir_all(format!("{}", uuid))?;

    info!("{:#?}", config);
    info!("App version: {:#?}", config.rust_version);

    let index_html = {
        let handlebars = handlebars::Handlebars::new();
        handlebars.render_template(&INDEX_TEMPLATE, &config)?
    };

    let get_structure = warp::get()
        .and(warp::path("structure"))
        .map(move || warp::reply::json(&folder_structure))
        .boxed();

    let get_static = warp::get()
        .and(warp::path!("static" / String))
        .and_then(load_static)
        .boxed();

    let get_docs = warp::get()
        .and(warp::path!("docs" / String))
        .map(move |path: String| (path.clone(), arc_docs_list.clone()))
        .and_then(load_local_file)
        .boxed();

    let get_install_status = warp::get()
        .and(warp::path!("install_status"))
        .and_then(load_install_status)
        .boxed();

    let get_plugins_location = warp::get()
        .and(warp::path!("plugins"))
        .and_then(find_plugins)
        .boxed();

    let get_select_exe = warp::get()
        .and(warp::path!("select_exe"))
        .and_then(select_exe)
        .boxed();

    let get_select_plugins = warp::get()
        .and(warp::path!("select_plugins"))
        .and_then(select_plugins)
        .boxed();

    let get_images = warp::get()
        .and(warp::path!("images" / String))
        .map(move |path: String| (path.clone(), arc_images_list.clone()))
        .and_then(load_local_image)
        .boxed();

    let post_check_path = warp::post()
        .and(warp::path!("check_path"))
        .and(warp::body::json())
        .and_then(check_exe_location_windows)
        .boxed();

    let post_patch_exe = warp::post()
        .and(warp::path!("patch_exe"))
        .and(warp::body::json())
        .and_then(patch_exe_windows)
        .boxed();

    let post_install_list = warp::post()
        .and(warp::path!("install_list"))
        .and(warp::body::json())
        .map(move |json: InstallConfig| {
            (
                json.clone(),
                arc_folder_structure.clone(),
                arc_asset_list.clone(),
            )
        })
        .and_then(install_nam)
        .boxed();

    let any = warp::any()
        .and(warp::path::peek())
        .and(warp::method())
        .boxed()
        .map(|r: warp::filters::path::Peek, m| {
            if r.as_str() != "" {
                warn!("Method: {} - Route: {:#?}: defaulted to root", m, r);
            }
        })
        .map(move |_| warp::reply::html(index_html.clone()));

    let all_routes = warp::any().and(
        get_static
            .or(get_structure)
            .or(get_install_status)
            .or(get_docs)
            .or(get_images)
            .or(get_plugins_location)
            .or(get_select_exe)
            .or(get_select_plugins)
            .or(post_check_path)
            .or(post_patch_exe)
            .or(post_install_list)
            .or(any),
    );
    let port: u16 = config.clone().web_server_port;

    if !cfg!(debug_assertions) {
        webbrowser::open(&format!("http://127.0.0.1:{}", port.to_owned()))?;
    };

    warp::serve(all_routes)
        .run(([0, 0, 0, 0], port.to_owned()))
        .await;

    Ok(())
}

async fn select_plugins() -> Result<impl warp::Reply> {
    let def_path = get_def_plugins().await?;
    let selected_path = select_folder_dialog(Some(def_path.as_str())).await?;

    Ok(selected_path)
}

async fn select_exe() -> Result<impl warp::Reply> {
    let def_path = get_def_home().await?;
    let selected_path = select_file_dialog(Some(def_path.as_str())).await?;

    Ok(selected_path)
}

fn check_exe(path: String) -> String {
    let file = std::fs::metadata(&path);
    match file {
        Ok(f) => {
            if f.is_file() {
                let cmd = std::process::Command::new("powershell.exe")
                    .arg("ItemPropertyValue")
                    .arg(format!("'{}'", &path))
                    .arg("-Name")
                    .arg("VersionInfo")
                    .output()
                    .expect("Unable to run powershell.exe.");
                let out = String::from_utf8_lossy(&cmd.stdout).to_string();
                let version = out.lines().collect::<Vec<&str>>()[3].trim()[0..9].to_string();
                let acceptable_versions = vec!["1.1.638.0", "1.1.640.0", "1.1.641.0"];
                if acceptable_versions.contains(&version.as_str()) {
                    unsafe {
                        CHECKED_EXE = true;
                    };
                    serde_json::json!({
                        "version": version.to_string(),
                        "valid" : true,
                        "path" : path
                    })
                } else {
                    serde_json::json!({
                        "version": version.to_string(),
                        "valid" : false,
                        "path" : ""
                    })
                }
            } else {
                serde_json::json!({
                    "version": "Could not locate exe.".to_string(),
                    "valid" : false,
                    "path" : ""
                })
            }
        }
        Err(e) => {
            warn!("{}", e.to_string());
            serde_json::json!({
                "version": "Could not locate exe.".to_string(),
                "valid" : false,
                "path" : ""
            })
        }
    }
    .to_string()
}

async fn check_exe_location_windows(path: String) -> Result<impl warp::Reply> {
    let check = check_exe(path);
    Ok(check)
}
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ExeResp {
    version: String,
    valid: bool,
}

fn flatten_installer_options(
    options: std::sync::Arc<Vec<InstallerOption>>,
) -> Vec<InstallerOption> {
    let mut output: Vec<Vec<InstallerOption>> = Vec::new();
    for opt in options.iter() {
        let mut no_child = opt.clone();
        no_child.children = Vec::new();
        output.push([no_child].to_vec());
        let children_arc = std::sync::Arc::new(opt.children.clone());
        output.push(flatten_installer_options(children_arc));
    }
    let mut output = output.concat();
    // output.sort_by(|a, b| a.name.cmp(&b.name));
    output.sort_unstable();
    output.dedup();
    output
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct InstallConfig {
    files_to_install: Vec<String>,
    location: String,
}
async fn install_nam(
    (install_config, options, asset_iter): (
        InstallConfig,
        std::sync::Arc<Vec<InstallerOption>>,
        std::sync::Arc<InstallAssetList>,
    ),
) -> Result<impl warp::Reply> {
    unsafe {
        CLEANED_FILE_COUNT = 0;
        CLEANED_FILE_MAX = 0;
        CLEANED_FILE_LIST = Vec::new();
        INSTALLED_FILE_COUNT = 0;
        INSTALLED_FILE_MAX = 0;
        INSTALLED_FILE_LIST = Vec::new();
    };

    if !&install_config.location.ends_with("Plugins") {
        Err(
            Error::Custom("Install Location must end in a folder called `Plugins`".to_string())
                .into(),
        )
    } else {
        let options = flatten_installer_options(options);
        std::thread::spawn(move || {
            // Clean Out old files (Cleanitol)
            let files_to_move = CLEANUP
                .lines()
                .map(|f| f.to_owned())
                .collect::<Vec<String>>();
            let plugins_dir = walkdir::WalkDir::new(&install_config.location);
            std::fs::create_dir(&install_config.location.replace("Plugins", "Plugins_bak"))
                .unwrap_or_else(|e| warn!("Unable to create plugins_bak dir: {}", e.to_string()));
            let max_clean = files_to_move.len();

            for (count, file) in plugins_dir.into_iter().enumerate() {
                unsafe {
                    CLEANED_FILE_COUNT = count;
                    CLEANED_FILE_MAX = max_clean;
                    let mut nl = CLEANED_FILE_LIST.clone();
                    let f_n = file
                        .as_ref()
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .to_string();
                    nl.push(f_n);
                    CLEANED_FILE_LIST = nl;
                };
                match &file {
                    Ok(f) => {
                        let f_n = f.file_name().to_string_lossy().to_string();

                        if files_to_move.contains(&f_n) {
                            let possible_dir = &f
                                .path()
                                .to_string_lossy()
                                .to_string()
                                .replace(
                                    &format!(
                                        "/{}",
                                        &f.path()
                                            .file_name()
                                            .unwrap()
                                            .to_string_lossy()
                                            .to_string()
                                    ),
                                    "",
                                )
                                .replace("Plugins", "Plugins_bak");

                            std::fs::create_dir_all(possible_dir).unwrap_or_else(|e| {
                                warn!(
                                    "Unable to create dir in plugins_bak because: {}.",
                                    e.to_string()
                                )
                            });

                            match std::fs::rename(
                                f.path(),
                                f.path()
                                    .to_string_lossy()
                                    .to_string()
                                    .replace("Plugins", "Plugins_bak"),
                            ) {
                                Ok(_) => info!("Successfully moved file: {} to plugins_bak", &f_n),
                                Err(e) => warn!(
                                    "Unable to move file: {}, to plugins_bak: {}",
                                    &f_n,
                                    e.to_string()
                                ),
                            };
                        } else {
                            continue;
                        }
                    }
                    Err(e) => {
                        warn!("{}", e.to_string());
                        continue;
                    }
                }
            }

            // Retrieve the files from the binary
            let mut chosen_options: Vec<InstallerOption> = Vec::new();
            for file in install_config.files_to_install {
                let opt: Vec<InstallerOption> = options
                    .iter()
                    .filter(|o| format!("{}/{}", o.parent, o.name.clone()) == file)
                    .map(|o| o.to_owned())
                    .collect();
                if opt.len() > 0 {
                    chosen_options.push(opt[0].clone())
                } else {
                    continue;
                }
            }
            let files_to_install = chosen_options
                .iter()
                .filter(|o| o.children.len() == 0)
                .map(|o| format!("{}/{}", o.location.clone(), o.original_name.clone()))
                .collect::<Vec<String>>();

            let file_list: Vec<String> = asset_iter
                .list
                .iter()
                .filter(|f| f.contains(".dat"))
                .map(|f| f.to_owned())
                .collect();

            let max_install = files_to_install.len();
            for (count, file_name) in files_to_install.iter().enumerate() {
                let file_name = file_name.replace("installation/", "");
                // println!("File Name: {:#?}", file_name);

                let filtered_file_list: Vec<&String> = file_list
                    .iter()
                    .filter(|f| f.contains(&file_name))
                    .collect();

                for file in filtered_file_list {
                    let file_data = asset_iter.list.iter().find(|f| f == &file);
                    match file_data {
                        Some(f) => {
                            info!("Retrieved file: {}", file);
                            let splits = file.split("/").collect::<Vec<&str>>();
                            let folder = format!(
                                "{}/{}",
                                &install_config.location,
                                prettify_folder_name(splits[..splits.len() - 1].join("/"))
                            );
                            let file_location = format!(
                                "{}/{}/{}",
                                install_config.location,
                                prettify_folder_name(splits[..splits.len() - 1].join("/")),
                                prettify_folder_name(splits[splits.len() - 1..].concat())
                            );

                            std::fs::create_dir_all(folder).unwrap_or_else(|e| {
                                warn!("Couldn't create install directories: {}", e.to_string())
                            });

                            match std::fs::write(&file_location, f) {
                                Ok(_) => {
                                    info!("Successfully wrote file: {}", &file_location);
                                }
                                Err(e) => {
                                    warn!(
                                        "Couldn't write file: {} because {}",
                                        &file_location,
                                        e.to_string()
                                    );
                                    continue;
                                }
                            };
                        }
                        None => {
                            warn!("Couldn't retrieve file: {}", file);
                            continue;
                        }
                    };
                }
                unsafe {
                    let count = count + 1;
                    INSTALLED_FILE_COUNT = count;
                    INSTALLED_FILE_MAX = max_install;
                    let mut nl = INSTALLED_FILE_LIST.clone();
                    nl.push(prettify_folder_name(file_name));
                    INSTALLED_FILE_LIST = nl;
                };
            }
        });

        Ok(serde_json::json!(
            { "cleaning_count" : 0.0
            , "cleaning_max" : 0.0
            , "installed_count" : 0.0
            , "installed_max" : 0.0
            , "files_cleaned" : []
            , "files_copied" : []
            }
        )
        .to_string())
    }
}

async fn patch_exe_windows(path: String) -> Result<impl warp::Reply> {
    let resp: ExeResp = serde_json::from_str(&check_exe(path.clone())).unwrap();
    if resp.valid {
        let uuid = uuid::Uuid::new_v4().to_hyphenated().to_string()[0..8].to_string();
        let home = get_def_home().await?;
        let file_path = format!("{}/Downloads/{}-4gb_patch.exe", home, uuid);
        std::fs::write(&file_path, FOUR_GB).expect("Unable to write file.");

        let out = std::process::Command::new("powershell.exe")
            .arg(&file_path)
            .arg(format!("'{}'", path))
            .output();
        match out {
            Ok(_) => {
                unsafe {
                    PATCHED_EXE = true;
                };
            }
            Err(e) => {
                info!("Couldn't run the 4gb patch exe: {}", e.to_string());
            }
        };
        let out = std::process::Command::new("powershell.exe")
            .arg("Remove-Item")
            .arg(&file_path)
            .output();
        match out {
            Ok(_) => Ok(serde_json::json!({ "patched" : true }).to_string()),
            Err(e) => {
                info!("Couldn't remove the 4gb patch exe: {}", e.to_string());
                Ok(serde_json::json!({ "patched" : false }).to_string())
            }
        }
    } else {
        Ok(serde_json::json!({ "patched" : false }).to_string())
    }
}

async fn select_folder_dialog(def_path: Option<&str>) -> Result<String> {
    let result = nfd::open_pick_folder(def_path).unwrap();

    match result {
        nfd::Response::Okay(folder) => Ok(folder),
        _ => Ok("".to_string()),
    }
}
async fn select_file_dialog(def_path: Option<&str>) -> Result<String> {
    let dialog = nfd::open_file_dialog(Some("exe"), def_path).unwrap();

    let dialog_res = match dialog {
        nfd::Response::Okay(folder) => folder,
        _ => "".to_string(),
    };
    let check_res = check_exe(dialog_res);
    Ok(check_res)
}

async fn get_def_home() -> Result<String> {
    let user_dir = directories::UserDirs::new().unwrap();
    Ok(user_dir.home_dir().to_string_lossy().to_string())
}

async fn get_def_plugins() -> Result<String> {
    let user_dir = directories::UserDirs::new().unwrap();
    let home_dir = user_dir.home_dir().to_string_lossy().to_string();

    match os_info::get().os_type().to_string().to_lowercase().as_str() {
        "windows" => Ok(format!("{}\\Documents\\SimCity 4\\Plugins", home_dir)),
        "macos" => Ok(format!("{}/Documents/SimCity 4/Plugins", home_dir)),
        _ => Ok(format!("{}/Documents/SimCity 4/Plugins", home_dir)),
    }
}

async fn find_plugins() -> Result<impl warp::Reply> {
    get_def_plugins().await
}

async fn load_install_status() -> Result<impl warp::Reply> {
    unsafe {
        Ok(serde_json::json!(
            { "cleaning_count" : CLEANED_FILE_COUNT
            , "cleaning_max" : CLEANED_FILE_MAX
            , "installed_count" : INSTALLED_FILE_COUNT
            , "installed_max" : INSTALLED_FILE_MAX
            , "files_cleaned" : CLEANED_FILE_LIST
            , "files_copied" : INSTALLED_FILE_LIST
            }
        )
        .to_string())
    }
}

async fn load_local_file(
    (file_name, asset_list): (String, std::sync::Arc<InstallAssetList>),
) -> Result<impl warp::Reply> {
    let name = percent_encoding::percent_decode_str(&file_name)
        .decode_utf8_lossy()
        .to_string();
    let folder = name.replace("installation/", "");

    if folder == "installation" || folder == "/Network Addon Mod" {
        Ok(match asset_list.get_file("Main.txt") {
            Some(a) => String::from_utf8_lossy(&a).to_string(),
            _ => "".to_string(),
        })
    } else {
        let list = asset_list.clone();
        let files: Vec<&String> = list
            .list
            .iter()
            .filter(|f| f.clone().contains(&folder))
            .filter(|f| {
                let s = f.clone();
                let idx = s.rfind(&folder).unwrap();
                let count = s[idx..].matches("\\").count();
                count < 2
            })
            .filter(|f| f.contains("txt"))
            .collect();

        let mut texts = Vec::new();
        for file in files {
            let file = file.replace("installation/", "").replace("\\", "/");

            texts.push(match asset_list.get_file(&file) {
                Some(a) => String::from_utf8_lossy(&a).to_string(),
                _ => "".to_string(),
            })
        }
        Ok(texts.join("\n"))
    }
}
async fn load_local_image(
    (file_name, asset_list): (String, std::sync::Arc<InstallAssetList>),
) -> Result<impl warp::Reply> {
    let name = percent_encoding::percent_decode_str(&file_name)
        .decode_utf8_lossy()
        .to_string();
    let folder = name.replace("installation/", "");

    if folder == "installation" || folder == "/Network Addon Mod" {
        let resp = asset_list
            .get_file("Network Addon Mod.png")
            .unwrap()
            .to_vec();
        Ok(resp)
    } else {
        let list = asset_list.clone();
        let files: Vec<&String> = list
            .list
            .iter()
            .filter(|f| f.clone().contains(&folder))
            .filter(|f| {
                let s = f.clone();
                let idx = s.rfind(&folder).unwrap();
                let count = s[idx..].matches("\\").count();
                count < 2
            })
            .filter(|f| f.contains("png"))
            .collect();

        let mut images = Vec::new();
        for file in files {
            let file = file.replace("installation/", "").replace("\\", "/");

            images.push(
                match asset_list.get_file(&file) {
                    Some(img) => img,
                    None => asset_list.get_file("Network Addon Mod.png").unwrap(),
                }
                .to_vec(),
            )
        }
        let resp = &match images.get(0) {
            Some(a) => a.to_owned(),
            _ => asset_list
                .get_file("Network Addon Mod.png")
                .unwrap()
                .to_vec(),
        };
        Ok(resp.clone())
    }
}

async fn load_static(file_name: String) -> Result<impl warp::Reply> {
    match file_name.as_str() {
        "main.js" => match tokio::fs::read("static/main.js").await {
            Ok(f) => Response::builder()
                .body(f)
                .map_err(|e| Error::Http(e).into()),
            Err(_) => {
                warn!("Couldn't retrieve static/main.js");
                Response::builder()
                    .body(JS.to_owned())
                    .map_err(|e| Error::Http(e).into())
            }
        },
        "bulma.css" => Response::builder()
            .body(BULMA.to_owned())
            .map_err(|e| Error::Http(e).into()),
        "bulma.css.map" => Response::builder()
            .body(BULMA.to_owned())
            .map_err(|e| Error::Http(e).into()),
        "favicon.ico" => Response::builder()
            .body(FAVICON_ICO.to_owned())
            .map_err(|e| Error::Http(e).into()),
        "favicon.png" => Response::builder()
            .body(FAVICON_PNG.to_owned())
            .map_err(|e| Error::Http(e).into()),
        _ => {
            warn!("File: {}", file_name);
            Err(Error::NotFound.into())
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Http Error: {0}")]
    Http(#[from] warp::http::Error),
    #[error("Not Found Error")]
    NotFound,
    #[error("Error: {0}")]
    Custom(String),
    #[error("Forbidden")]
    Forbidden,
    #[error("IO: {0}")]
    IO(#[from] std::io::Error),
}
impl warp::reject::Reject for Error {}

impl std::convert::From<Error> for warp::reject::Rejection {
    fn from(e: Error) -> warp::reject::Rejection {
        warn!("{:#?}", e);
        warp::reject::custom(e)
    }
}

pub async fn handle_rejection(error: warp::reject::Rejection) -> Result<impl warp::Reply> {
    let error_msg = match error.find::<Error>() {
        Some(err) => {
            warn!("Request Rejection: {:#?}", err.to_string());
            warp::reply::with_status(
                warp::reply::html(format!("Bad Request: {}.", err.to_string())),
                warp::http::StatusCode::BAD_REQUEST,
            )
        }
        None => {
            debug!("{:#?}", error);
            warn!("Request Rejection: Bad Request.");
            warp::reply::with_status(
                warp::reply::html("Bad Request: Unknown Error.".to_string()),
                warp::http::StatusCode::BAD_REQUEST,
            )
        }
    };
    Ok(error_msg)
}
#[derive(Debug, Clone, Serialize, PartialEq, Ord, PartialOrd, Eq)]
enum RadioCheck {
    Radio,
    RadioChecked,
    RadioFolder,
    Checked,
    Unchecked,
    Locked,
    ParentLocked,
}
impl RadioCheck {
    fn new(name: &str) -> anyhow::Result<Self> {
        match name {
            "Radio" => Ok(RadioCheck::Radio),
            "RadioChecked" => Ok(RadioCheck::RadioChecked),
            "RadioFolder" => Ok(RadioCheck::RadioFolder),
            "Checked" => Ok(RadioCheck::Checked),
            "Unchecked" => Ok(RadioCheck::Unchecked),
            "Locked" => Ok(RadioCheck::Locked),
            "ParentLocked" => Ok(RadioCheck::ParentLocked),
            _ => Err(anyhow!("Invalid RadioCheck string: {}", name)),
        }
    }

    fn determine(s: &str) -> Self {
        if s.contains("~") {
            RadioCheck::Locked
        } else if s.contains("^") {
            RadioCheck::ParentLocked
        } else if s.contains("+") {
            RadioCheck::Radio
        } else if s.contains("=") {
            RadioCheck::RadioChecked
        } else if s.ends_with("#") {
            RadioCheck::RadioFolder
        } else if s.ends_with("!") {
            RadioCheck::Unchecked
        } else {
            RadioCheck::Checked
        }
    }
}

fn prettify_folder_name(s: String) -> String {
    s.replace("$1", "")
        .replace("$2", "")
        .replace("$3", "")
        .replace("$4", "")
        .replace("$5", "")
        .replace("$6", "")
        .replace("$7", "")
        .replace("$8", "")
        .replace("$9", "")
        .replace("^", "")
        .replace("+", "")
        .replace("=", "")
        .replace("#", "")
        .replace("!", "")
        .replace("~", "")
        .replace("*", "")
}

#[derive(Debug, Clone, Serialize, PartialEq, Ord, PartialOrd, Eq)]
struct InstallerOption {
    name: String,
    original_name: String,
    location: String,
    radio_check: RadioCheck,
    children: Vec<InstallerOption>,
    depth: u16,
    parent: String,
}
impl InstallerOption {
    fn new(original_name: String, radio_check: RadioCheck) -> anyhow::Result<Self> {
        let name: String = original_name
            .replace("$1", "")
            .replace("$2", "")
            .replace("$3", "")
            .replace("$4", "")
            .replace("$5", "")
            .replace("$6", "")
            .replace("$7", "")
            .replace("$8", "")
            .replace("$9", "")
            .replace("^", "")
            .replace("+", "")
            .replace("=", "")
            .replace("#", "")
            .replace("!", "")
            .replace("~", "")
            .replace("*", "");

        Ok(InstallerOption {
            name,
            original_name,
            location: "".into(),
            radio_check,
            children: Vec::new(),
            depth: 0,
            parent: "".into(),
        })
    }
    fn push_children(&self, children: &mut Vec<InstallerOption>) -> Self {
        children.append(self.children.clone().as_mut());
        InstallerOption {
            name: self.name.clone(),
            original_name: self.original_name.clone(),
            location: self.location.clone(),
            radio_check: self.radio_check.clone(),
            children: children.to_vec(),
            depth: self.depth.clone(),
            parent: self.parent.clone(),
        }
    }
}

fn folder_structure(dir: &str) -> anyhow::Result<InstallerOption> {
    let options =
        InstallerOption::new("Network Addon Mod".to_string(), RadioCheck::new("Locked")?)?;

    Ok(options.push_children(parse_folder(dir, 0, "top", "installation")?.as_mut()))
}

fn parse_folder(
    dir: &str,
    parent_depth: u16,
    parent_name: &str,
    original_parent_name: &str,
) -> anyhow::Result<Vec<InstallerOption>> {
    let files = std::fs::read_dir(dir)?;
    let mut options = Vec::new();

    for entry in files.into_iter() {
        match entry {
            Ok(e) => {
                // println!("{:#?}", &e);
                let f_n = &e.file_name().to_str().unwrap().to_owned();

                let local_res = if e.metadata()?.is_dir() {
                    let mut local_option =
                        InstallerOption::new(f_n.to_string(), RadioCheck::determine(&f_n))?;
                    local_option.depth = parent_depth + 1;
                    local_option.parent = parent_name.into();
                    local_option.location = original_parent_name.into();
                    let mut children = parse_folder(
                        &e.path().to_str().unwrap(),
                        parent_depth + 1,
                        &format!("{}/{}", parent_name, &local_option.name),
                        &format!("{}/{}", original_parent_name, &local_option.original_name),
                    )?;
                    local_option.push_children(children.as_mut())
                } else {
                    // InstallerOption::new(f_n, RadioCheck::determine(&f_n))?
                    continue;
                };
                options.push(local_res);
            }
            Err(e) => {
                warn!("Error reading file: {}", e);
                continue;
            }
        }
    }
    // println!("{:#?}", &options);
    Ok(options)
}
