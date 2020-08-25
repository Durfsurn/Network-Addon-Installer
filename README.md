# Network Addon Installer

A Rust based application for installing the Network Addon Mod.

Installer source files must be in the `installation/` folder.
- Use a `~` **contained** in the folder name for locked parent folders, such as $1~1_Core
- Use a `^` **contained** in the folder name for locked child folders, such as ^Locale Files
- Use a `+` **contained** in the folder name for an unchecked radio button option
- Use a `=` **contained** in the folder name for a *checked* radio button option
- Use a `#` **contained** in the folder name for a folder that *only* contains radio buttons
- Use a `!` **contained** in the folder name for a non-default option

Doc files must be in the `docs/` folder, with the file name the same as the feature its for:
- e.g. for feature `z_NAM Controller_LHD`, create a file called `z_NAM Controller_LHD.txt`

Images are the same, and must be in `.png` format:
- e.g. for feature `z_NAM Controller_LHD`, create a file called `z_NAM Controller_LHD.png`

## Compilation

Assuming you have Rust installed, run `cargo build --release` from the root folder. Your output binary will be found in `target/release/` called `network-addon-installer`.

## TODO
- [x] Fix strange radio button issues (deselecting children, strange behaviour interacting with other radio buttons)
- [x] Send selection back to Rust and prompt install
- [ ] Test on Windows, Linux, Mac
- [ ] Potentially recode a Controller Compiler? (stretch goal)

Many thanks to NT core for the freeware 4GB Patch!
