use std::{io::Cursor, path::Path, str::FromStr};

use anyhow::anyhow;
use log::{info, warn};
use walkdir::WalkDir;

#[cfg(not(debug_assertions))]
const INSTALLATION_FILES: &[u8] = include_bytes!("../static/data.bin");

#[cfg(not(debug_assertions))]
fn get_install_asset_list(uuid: String, zip: &[u8]) -> anyhow::Result<Vec<String>> {
    std::fs::create_dir_all(&format!("C:/temp/{}", uuid))?;
    println!("{:#?}", uuid);

    let file = Cursor::new(zip);

    let mut zip = zip::ZipArchive::new(file)?;

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
    Ok(vec![WalkDir::new(&format!("C:/temp/{}", uuid))
        .into_iter()
        .map(|f| f.unwrap().path().to_string_lossy().to_string())
        .collect()])
}

#[cfg(not(debug_assertions))]
pub fn temp_files() -> anyhow::Result<InstallerOption> {
    println!("\nStarting to unzip files. Do not close this window.");
    let temp_folder_uuid = uuid::Uuid::new_v4().to_hyphenated().to_string()[0..8].to_string();
    let asset_iter = get_install_asset_list(temp_folder_uuid.clone(), INSTALLATION_FILES)?;
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

    folder_structure(&format!("{}/", uuid))
}
#[cfg(debug_assertions)]
pub fn temp_files() -> anyhow::Result<InstallerOption> {
    folder_structure("installation/")
}

#[cfg(not(debug_assertions))]
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

#[derive(Debug, Clone, PartialEq, Ord, PartialOrd, Eq)]
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
        } else if s.contains("=") {
            RadioCheck::RadioChecked
        } else if s.contains("-") {
            RadioCheck::Radio
        } else if s.ends_with("#") {
            RadioCheck::RadioFolder
        } else if s.contains("!") {
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

#[derive(Debug, Clone, PartialEq, Ord, PartialOrd, Eq)]
pub struct InstallerOption {
    pub name: String,
    original_name: String,
    location: String,
    radio_check: RadioCheck,
    pub children: Vec<InstallerOption>,
    pub depth: u16,
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

    pub fn checked(&self) -> bool {
        match self.radio_check {
            RadioCheck::Checked | RadioCheck::RadioChecked => true,
            _ => false,
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
