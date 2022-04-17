#! /bin/bash
cargo build
mv target/debug/Network-Addon-Installer.exe ./Network-Addon-Installer.exe
export FILES_DIR=C:/Users/durfsurn/Documents/Network-Addon-Installer/installation
./Network-Addon-Installer.exe