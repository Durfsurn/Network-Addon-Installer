use std::{ffi::OsString, fs, path};

use serde::{Deserialize, Serialize};

fn main() -> anyhow::Result<()> {
    let installation = InstallerFiles::new("..\\installation");
    // dbg!(installation);
    fs::write(
        "../installation.json",
        serde_json::to_string(&installation)?,
    )?;

    Ok(())
}

fn walkdir<P>(d: P) -> Vec<InstallerFiles>
where
    P: AsRef<path::Path>,
{
    let dir = fs::read_dir(&d).unwrap_or_else(|e| {
        panic!(
            "Could not read dir {} because {}",
            d.as_ref().to_string_lossy(),
            e
        )
    });
    dir.filter(|item| item.as_ref().is_ok())
        .map(|item| {
            let item = item.unwrap_or_else(|e| panic!("Could not read items because {}", e));
            let p = item.path().as_os_str().to_string_lossy().to_string();

            if item.path().is_dir() {
                InstallerFiles {
                    path: p.clone(),
                    name: prettify_folder_name(&p),
                    children: walkdir(item.path()),
                    radio_check: RadioCheck::determine(&p),
                }
            } else {
                InstallerFiles {
                    path: p.clone(),
                    name: prettify_folder_name(&p),
                    children: Vec::new(),
                    radio_check: RadioCheck::determine(&p),
                }
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct InstallerFiles {
    path: String,
    name: String,
    children: Vec<InstallerFiles>,
    radio_check: RadioCheck,
}
impl InstallerFiles {
    fn new<P>(d: P) -> Self
    where
        P: AsRef<path::Path>,
        OsString: From<P>,
    {
        InstallerFiles {
            path: d.as_ref().as_os_str().to_string_lossy().to_string(),
            name: prettify_folder_name(
                d.as_ref()
                    .as_os_str()
                    .to_string_lossy()
                    .to_string()
                    .as_str(),
            ),
            children: walkdir(d),
            radio_check: RadioCheck::ParentLocked,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Ord, PartialOrd, Eq)]
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
            _ => Err(anyhow::anyhow!("Invalid RadioCheck string: {}", name)),
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
fn prettify_folder_name(s: &str) -> String {
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
