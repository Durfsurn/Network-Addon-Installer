#! /usr/bin/bash
wsl.exe "/mnt/c/Users/durfsurn/Documents/Network-Addon-Installer/elm_compile.sh"
cargo build
Copy-Item "target/debug/network-addon-installer.exe" -Destination "."
./network-addon-installer.exe