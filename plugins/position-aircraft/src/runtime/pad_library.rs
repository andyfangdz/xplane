use std::fs;

use crate::pad::{parse_pad, safe_pad_filename, write_pad, Field, Form};

use super::state::PluginState;

impl PluginState {
    pub(in crate::runtime) fn refresh_pads(&mut self) {
        let old = self.pads.get(self.selected_index).cloned();
        let mut pads = Vec::new();
        match fs::read_dir(&self.pad_directory) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if !entry
                        .file_type()
                        .map(|kind| kind.is_file())
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.to_ascii_lowercase().ends_with(".pad") {
                        pads.push(name);
                    }
                }
                pads.sort_by_key(|name| name.to_ascii_lowercase());
            }
            Err(error) => {
                self.status = format!("Unable to read PAD directory: {error}");
            }
        }
        self.pads = pads;
        self.selected_index = old
            .as_ref()
            .and_then(|old_name| self.pads.iter().position(|name| name == old_name))
            .or_else(|| {
                self.pads
                    .iter()
                    .position(|name| name.eq_ignore_ascii_case("QuickFile.pad"))
            })
            .unwrap_or(0)
            .min(self.pads.len().saturating_sub(1));
        if !self.status.starts_with("Unable") {
            self.status = format!("Found {} PAD files", self.pads.len());
        }
    }

    pub(in crate::runtime) fn selected_name(&self) -> Option<&str> {
        self.pads.get(self.selected_index).map(String::as_str)
    }

    pub(in crate::runtime) fn select_pad(&mut self, index: usize) {
        if index < self.pads.len() {
            self.selected_index = index;
            self.status = format!("Selected {}", self.pads[index]);
        }
    }

    pub(in crate::runtime) fn load_file(&mut self, filename: &str) -> bool {
        match parse_pad(&self.pad_directory.join(filename)) {
            Ok(data) => {
                let save_name = filename
                    .strip_suffix(".pad")
                    .or_else(|| filename.strip_suffix(".PAD"))
                    .unwrap_or(filename);
                self.form = Form::from_data(&data, save_name);
                self.status = format!("Loaded {filename}");
                true
            }
            Err(error) => {
                self.status = error;
                false
            }
        }
    }

    pub(in crate::runtime) fn load_selected(&mut self, position: bool) {
        let Some(filename) = self.selected_name().map(str::to_owned) else {
            self.status = "No PAD file is selected".to_owned();
            return;
        };
        if self.load_file(&filename) && position {
            self.position_loaded();
        }
    }

    pub(in crate::runtime) fn select_relative(&mut self, delta: isize, position: bool) {
        if self.pads.is_empty() {
            self.refresh_pads();
        }
        if self.pads.is_empty() {
            self.status = "No PAD files found".to_owned();
            return;
        }
        self.selected_index =
            (self.selected_index as isize + delta).rem_euclid(self.pads.len() as isize) as usize;
        self.load_selected(position);
    }

    pub(in crate::runtime) fn quick_load(&mut self, position: bool) {
        if self.load_file("QuickFile.pad") && position {
            self.position_loaded();
        }
    }

    pub(in crate::runtime) fn quick_save(&mut self) {
        let data = self.capture_current();
        match write_pad(&self.pad_directory.join("QuickFile.pad"), &data) {
            Ok(()) => {
                self.refresh_pads();
                self.status = "Quick-saved current aircraft to QuickFile.pad".to_owned();
            }
            Err(error) => self.status = format!("Unable to write QuickFile.pad: {error}"),
        }
    }

    pub(in crate::runtime) fn save_named(&mut self) {
        let data = match self.form.to_data() {
            Ok(data) => data,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let Some(filename) = safe_pad_filename(self.form.value(Field::SaveName)) else {
            self.status = "Enter a PAD filename".to_owned();
            return;
        };
        match write_pad(&self.pad_directory.join(&filename), &data) {
            Ok(()) => {
                self.refresh_pads();
                if let Some(index) = self.pads.iter().position(|name| name == &filename) {
                    self.selected_index = index;
                }
                self.status = format!("Saved {filename}");
            }
            Err(error) => self.status = format!("Unable to write {filename}: {error}"),
        }
    }
}
