use serde::{Deserialize, Serialize};
use std::fs::File;
use warp::{Filter, http::Response};
use log::{info, warn, debug};
use std::fs;
use anyhow::anyhow;

type Result<T> = std::result::Result<T, warp::Rejection>;

const BULMA: &[u8] = include_bytes!("../static/bulma.css");
const JS: &[u8] = include_bytes!("../static/main.js");
const INDEX_TEMPLATE: &str = include_str!("../static/index.html.hbs");
const FAVICON_PNG: &[u8] = include_bytes!("../static/favicon.png");
const FAVICON_ICO: &[u8] = include_bytes!("../static/favicon.ico");

fn rust_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}


#[derive(Debug, Clone, Deserialize, Serialize)]
struct Configuration {
    title: String,
    #[serde(default = "rust_version")]
    rust_version: String,
    files_location: String,
    version: f64,
    web_server_port: u16
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config: Configuration = serde_json::from_reader(File::open("configuration.json")?)?;
    
    let folder_structure = [folder_structure(&config.files_location)?].to_vec();

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
        .or(any)
    );
    let port: u16 = config.clone().web_server_port;
   
    webbrowser::open(&format!("http://localhost:{}", port.to_owned()))?;

    warp::serve(all_routes).run(([0, 0, 0, 0], port.to_owned())).await;

    Ok(())
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
    let files = fs::read_dir(dir)?;
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