## Purpose

This repo contains the laptop configuration system used by FRC team 766

## Overview

The directory `T766-ControlServer` contains:
- The admin UI for the system
- The control node server

## Usage

Run `make rpi_build` to build the control server for a raspberry pi. It will output a `target/dx/T766-ControlServer/release/web/` folder containing a binary and the static assets it requires!
