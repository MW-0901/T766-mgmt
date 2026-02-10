## Purpose

This repo contains the laptop configuration system used by FRC team 766

## Overview

The directory `T766-ControlServer` contains:
- The admin UI for the system
- The control node server

## Usage

Run `bash rpi_build.sh` from a unix-like machine to build the control server for a raspberry pi. It will output a `target/dx/T766-ControlServer/release/web/` folder containing a binary and the static assets it requires!

To build an installer for the windows client, please first run `cargo install cargo-packager --locked`. From there, run `cargo packager --release` to build an installer.