use serde::{Deserialize, Serialize};
use warp::{Filter, http::Response};
use log::{info, warn, debug, error};
use anyhow::anyhow;
use percent_encoding;
use rust_embed::RustEmbed;
use std::path::{Path};
use os_info;

#[derive(RustEmbed)]
#[folder = "docs"]
struct DocAsset;
#[derive(RustEmbed)]
#[folder = "images"]
struct ImageAsset;

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

fn rust_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}


#[derive(Debug, Clone, Deserialize, Serialize)]
struct Configuration {
    title: String,
    #[serde(default = "rust_version")]
    rust_version: String,
    version: f64,
    web_server_port: u16,
    #[serde(default)]
    windows: bool
}

fn calculate_folders(strs: &mut Vec<String>) -> Vec<String> {
    let as_paths = strs.iter().map(|s| Path::new(s));
    let mut folders: Vec<String> = as_paths.map(|p| {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let full_path = p.to_string_lossy().to_string();
        full_path.replace(&name, "")
    }).collect();
    folders.sort();
    folders.dedup();
    folders.append(strs);
    folders.iter().filter(|f| f.len() > 0).map(|f| f.to_string()).collect::<Vec<String>>()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {    
    let windows = os_info::get().os_type().to_string().to_lowercase() == "windows";

    env_logger::init();
    let mut config: Configuration = serde_json::from_str(CONFIG)?;
    config.windows = windows;

    let file_list: Vec<String> = calculate_folders(InstallAsset::iter().map(|asset| asset.to_string()).collect::<Vec<String>>().as_mut());

    let uuid = uuid::Uuid::new_v4().to_hyphenated();

    for folder in file_list.iter().filter(|f| f.ends_with("/")) {
        std::fs::create_dir_all(format!("{}/{}", uuid, folder))?;
    };

    let folder_structure = [
            folder_structure(&format!("{}/", uuid))?
        ].to_vec()
    ;
    
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
        .boxed()
    ;

    let get_static = warp::get()
        .and(warp::path!("static" / String))
        .and_then(load_static)
        .boxed()
    ;

    let get_docs = warp::get()
        .and(warp::path!("docs" / String))
        .and_then(load_local_file)
        .boxed()
    ;

    let get_images = warp::get()
        .and(warp::path!("images" / String))
        .and_then(load_local_image)
        .boxed()
    ;

    let post_check_path = warp::post()
        .and(warp::path!("check_path"))
        .and(warp::body::json())
        .and_then(check_exe_location_windows)
        .boxed()
    ;

    let post_patch_exe = warp::post()
        .and(warp::path!("patch_exe"))
        .and(warp::body::json())
        .and_then(patch_exe_windows)
        .boxed()
    ;

    let any = warp::any()
        .and(warp::path::peek())
        .and(warp::method())        
        .boxed()
        .map(|r: warp::filters::path::Peek, m| {
            if r.as_str() != "" {   
                warn!("Method: {} - Route: {:#?}: defaulted to root", m, r);
            }
        })
        .map(move |_| warp::reply::html(index_html.clone())
    );

    let all_routes = warp::any()    
    .and(
        get_static
        .or(get_structure)
        .or(get_docs)
        .or(get_images)
        .or(post_check_path)
        .or(post_patch_exe)
        .or(any)
    );
    let port: u16 = config.clone().web_server_port;
   
    webbrowser::open(&format!("http://localhost:{}", port.to_owned()))?;

    warp::serve(all_routes).run(([0, 0, 0, 0], port.to_owned())).await;

    Ok(())
}

async fn check_exe(path: String) -> String {    
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
                    .expect("Unable to run powershell.exe.")
                ;
                let out = String::from_utf8_lossy(&cmd.stdout).to_string();
                let version = out.lines().collect::<Vec<&str>>()[3]
                    .trim()
                    [0..9]
                    .to_string()
                ;
                let acceptable_versions = vec!["1.1.638.0", "1.1.640.0", "1.1.641.0"];
                if acceptable_versions.contains(&version.as_str()) {
                    serde_json::json!({
                        "version": version.to_string(),
                        "valid" : true
                    })
                }
                else {
                    serde_json::json!({
                        "version": version.to_string(),
                        "valid" : false
                    })
                }
            }
            else {
                serde_json::json!({
                    "version": "Could not locate exe.".to_string(),
                    "valid" : false
                })
            }
        }
        Err(e) => {
            warn!("{}", e.to_string());
            serde_json::json!({
                "version": "Could not locate exe.".to_string(),
                "valid" : false
            })
        }
    }.to_string()
}

async fn check_exe_location_windows(path: String) -> Result<impl warp::Reply> {
    let check = check_exe(path).await;
    Ok(check)
}
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ExeResp {
    version : String,
    valid : bool
}

async fn patch_exe_windows(path: String) -> Result<impl warp::Reply> {
    let resp: ExeResp = serde_json::from_str(&check_exe(path.clone()).await).unwrap();
    if resp.valid {
        let uuid = uuid::Uuid::new_v4().to_hyphenated().to_string()[0..8].to_string();
        std::fs::write(format!("{}-4gb_patch.exe", uuid), FOUR_GB).expect("Unable to write file.");
        
        let out = std::process::Command::new("powershell.exe")
            .arg(format!("./{}-4gb_patch.exe", uuid))    
            .arg(format!("'{}'", path))
            .output()
        ;
        std::fs::remove_file(format!("{}-4gb_patch.exe", uuid)).expect("Unable to remove file.");
        match out {
            Ok(_) => Ok(serde_json::json!({ "patched" : true }).to_string()),
            Err(_) => Ok(serde_json::json!({ "patched" : false }).to_string())
        }        
    }
    else {
        Ok(serde_json::json!({ "patched" : false }).to_string())
    }
}

async fn load_local_file(file_name: String) -> Result<impl warp::Reply> {
    let name = percent_encoding::percent_decode_str(&file_name).decode_utf8_lossy().to_string();
    let name = format!("{}.txt", name);
    
    info!("Loading {}", name);

    let f: Vec<u8> = match DocAsset::get(&name) {
        Some(file) => file.into(),
        None => {
            warn!("Unable to retrieve file: {}", &name);
            Vec::new()
        }
    };

    let str_f = String::from_utf8_lossy(f.as_ref()).to_string();
    Ok(str_f)
}
async fn load_local_image(file_name: String)  -> Result<impl warp::Reply> {
    let name = percent_encoding::percent_decode_str(&file_name).decode_utf8_lossy().to_string();

    info!("Loading {}", name);

    let f = match ImageAsset::get(&name) {
        Some(file) => Ok(file),
        None => Err(Error::Custom(format!("Unable to retrieve file: {}", &name)))
    }?;

    let bytes_f: Vec<u8> = f.into();
    Ok(bytes_f)
}

async fn load_static(file_name: String) -> Result<impl warp::Reply> {
    match file_name.as_str() {
        "main.js" =>  {
            match tokio::fs::read("static/main.js").await {
                Ok(f) => Response::builder().body(f).map_err(|e| Error::Http(e).into()),
                Err(_) => {
                    warn!("Couldn't retrieve static/main.js");
                    Response::builder().body(JS.to_owned())
                    .map_err(|e| Error::Http(e).into())
                }
            }
        },
        "bulma.css" =>  {
            Response::builder()
                .body(BULMA.to_owned())
                .map_err(|e| Error::Http(e).into())
        },
        "bulma.css.map" =>  {
            Response::builder()
                .body(BULMA.to_owned())
                .map_err(|e| Error::Http(e).into())
        },
        "favicon.ico" =>  {
            Response::builder()
                .body(FAVICON_ICO.to_owned())
                .map_err(|e| Error::Http(e).into())
        },
        "favicon.png" =>  {
            Response::builder()
                .body(FAVICON_PNG.to_owned())
                .map_err(|e| Error::Http(e).into())
        },
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
            warp::reply::with_status(warp::reply::html(format!("Bad Request: {}.", err.to_string())), warp::http::StatusCode::BAD_REQUEST)
        }
        None => {
            debug!("{:#?}", error);
            warn!("Request Rejection: Bad Request.");
            warp::reply::with_status(warp::reply::html("Bad Request: Unknown Error.".to_string()), warp::http::StatusCode::BAD_REQUEST)
        }
    };
    Ok(error_msg)
}
#[derive(Debug, Clone, Serialize)]
enum RadioCheck {
    Radio,
    RadioChecked,
    RadioFolder,
    Checked,
    Unchecked,
    Locked,
    ParentLocked
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
            _ => Err(anyhow!("Invalid RadioCheck string: {}", name))
        }
    }

    fn determine(s: &str) -> Self {
        if s.contains("~") {
            RadioCheck::Locked
        }
        else if s.contains("^") {
            RadioCheck::ParentLocked
        }
        else if s.contains("+")  {
            RadioCheck::Radio
        }
        else if s.contains("=") {
            RadioCheck::RadioChecked
        }
        else if s.ends_with("#") {
            RadioCheck::RadioFolder
        }
        else if s.ends_with("*") {
            RadioCheck::Checked
        }
        else {
            RadioCheck::Unchecked
        }
    }
}
#[derive(Debug, Clone, Serialize)]
struct InstallerOption {
    name: String,
    radio_check: RadioCheck,
    children: Vec<InstallerOption>,
    depth: u16,
    parent: String
}
impl InstallerOption {
    fn new<N>(name: N, radio_check: RadioCheck) -> anyhow::Result<Self> where N: Into<String>{
        let name: String = name.into()
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
        ;

        Ok(InstallerOption {
            name,
            radio_check,
            children: Vec::new(),
            depth: 0,
            parent: "".into()
        })
    }
    fn push_children(&self, children: &mut Vec<InstallerOption>) -> Self {
        children.append(self.children.clone().as_mut());
        InstallerOption {
            name: self.name.clone(),
            radio_check: self.radio_check.clone(),
            children: children.to_vec(),
            depth: self.depth.clone(),
            parent: self.parent.clone()
        }
    }
}

fn folder_structure(dir: &str) -> anyhow::Result<InstallerOption> {
    let options = InstallerOption::new("Network Addon Mod", RadioCheck::new("Locked")?)?;
    
    Ok(options.push_children(parse_folder(dir, 0, "top")?.as_mut()))
}

fn parse_folder(dir: &str, parent_depth: u16, parent_name: &str) -> anyhow::Result<Vec<InstallerOption>> {
    let files = std::fs::read_dir(dir)?;
    let mut options = Vec::new();

    for entry in files.into_iter() {
        match entry {
            Ok(e) => {
                // println!("{:#?}", &e);
                let f_n = &e.file_name().to_str().unwrap().to_owned();

                let local_res = 
                    if e.metadata()?.is_dir() {
                        let mut local_option = InstallerOption::new(f_n, RadioCheck::determine(&f_n))?;
                        local_option.depth = parent_depth + 1;
                        local_option.parent = parent_name.into();
                        let mut children = parse_folder(&e.path().to_str().unwrap(), parent_depth + 1, &format!("{}/{}", parent_name, &local_option.name))?;
                        local_option.push_children(children.as_mut())
                    }
                    else {
                        // InstallerOption::new(f_n, RadioCheck::determine(&f_n))?
                        continue
                    }
                ;
                options.push(local_res);
            },
            Err(e) => {
                warn!("Error reading file: {}", e);
                continue
            }
        }
    };
    // println!("{:#?}", &options);
    Ok(options)
}