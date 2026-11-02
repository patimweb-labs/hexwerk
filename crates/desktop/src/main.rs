// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Patrick Weppelmann

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    hexwerk_desktop::run();
}
