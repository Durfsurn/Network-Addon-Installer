#! /usr/bin/bash
cargo build --release
cp target/release/network-addon-installer.exe .
