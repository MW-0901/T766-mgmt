## Purpose

This repo contains the laptop configuration system used by FRC team 766

## Overview

The directory `T766-ControlServer` contains:
- The admin UI for the system
- The control node server

## Usage

- You can use `nix run` to build and run the development server, or `nix run .#serve` for a proper release bundle for prod
- If you do not want to use nix, you can manually install the dioxus CLI and run `dx serve --release --port 8000` from the T766-ControlServer directory for a development environment.