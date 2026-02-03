use crate::config::{Config, KeyFormat, ValueFormat};
use crate::db::SecondaryDb;
use crate::parser::{parse_key, parse_value};
use anyhow::Result;

const PAGE_SIZE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Focus {
    ColumnFamilies,
    Keys,
    Value,
}

pub struct App {
    pub db: SecondaryDb,
    pub config: Config,
    pub focus: Focus,
    pub cf_index: usize,
    pub key_index: usize,
    pub keys: Vec<(Vec<u8>, Vec<u8>)>,
    pub has_more_keys: bool,
    pub search_input: String,
    pub search_active: bool,
    pub search_prefix: Option<Vec<u8>>,
    pub should_quit: bool,
}

impl App {
    pub fn new(db: SecondaryDb, config: Config) -> Result<Self> {
        let mut app = Self {
            db,
            config,
            focus: Focus::ColumnFamilies,
            cf_index: 0,
            key_index: 0,
            keys: Vec::new(),
            has_more_keys: false,
            search_input: String::new(),
            search_active: false,
            search_prefix: None,
            should_quit: false,
        };
        app.load_keys()?;
        Ok(app)
    }

    pub fn current_cf(&self) -> Option<&str> {
        self.db
            .column_families()
            .get(self.cf_index)
            .map(|s| s.as_str())
    }

    pub fn key_format(&self) -> KeyFormat {
        self.current_cf()
            .and_then(|cf| self.config.get_cf_config(cf))
            .map(|c| c.key_format.clone())
            .unwrap_or_default()
    }

    pub fn value_format(&self) -> ValueFormat {
        self.current_cf()
            .and_then(|cf| self.config.get_cf_config(cf))
            .map(|c| c.value_format.clone())
            .unwrap_or_default()
    }

    pub fn load_keys(&mut self) -> Result<()> {
        if let Some(cf) = self.current_cf() {
            let cf = cf.to_string();
            self.keys = if let Some(ref prefix) = self.search_prefix {
                self.db.iter_keys_with_prefix(&cf, prefix, PAGE_SIZE)?
            } else {
                self.db.iter_keys(&cf, None, PAGE_SIZE)?
            };
            self.has_more_keys = self.keys.len() == PAGE_SIZE && self.search_prefix.is_none();
            self.key_index = 0;
        }
        Ok(())
    }

    pub fn load_more_keys(&mut self) -> Result<()> {
        if !self.has_more_keys {
            return Ok(());
        }
        if let Some(cf) = self.current_cf() {
            let cf = cf.to_string();
            if let Some((last_key, _)) = self.keys.last() {
                let more = self.db.iter_keys(&cf, Some(last_key), PAGE_SIZE)?;
                self.has_more_keys = more.len() == PAGE_SIZE;
                self.keys.extend(more);
            }
        }
        Ok(())
    }

    pub fn execute_search(&mut self) -> Result<()> {
        if self.search_input.is_empty() {
            self.clear_search()?;
        } else {
            self.search_prefix = Some(self.search_input.as_bytes().to_vec());
            self.search_active = false;
            self.load_keys()?;
        }
        Ok(())
    }

    pub fn clear_search(&mut self) -> Result<()> {
        self.search_prefix = None;
        self.search_input.clear();
        self.search_active = false;
        self.load_keys()?;
        Ok(())
    }

    pub fn estimate_keys(&self) -> Option<u64> {
        self.current_cf()
            .and_then(|cf| self.db.estimate_num_keys(cf))
    }

    pub fn formatted_keys(&self) -> Vec<String> {
        let format = self.key_format();
        self.keys
            .iter()
            .map(|(k, _)| parse_key(k, &format))
            .collect()
    }

    pub fn current_value(&self) -> Option<(String, bool)> {
        self.keys.get(self.key_index).map(|(_, v)| {
            let result = parse_value(v, &self.value_format());
            (result.content, result.success)
        })
    }

    pub fn next_cf(&mut self) -> Result<()> {
        let len = self.db.column_families().len();
        if len > 0 {
            self.cf_index = (self.cf_index + 1) % len;
            self.load_keys()?;
        }
        Ok(())
    }

    pub fn prev_cf(&mut self) -> Result<()> {
        let len = self.db.column_families().len();
        if len > 0 {
            self.cf_index = if self.cf_index == 0 {
                len - 1
            } else {
                self.cf_index - 1
            };
            self.load_keys()?;
        }
        Ok(())
    }

    pub fn next_key(&mut self) -> Result<()> {
        if self.key_index + 1 < self.keys.len() {
            self.key_index += 1;
        } else if self.has_more_keys {
            self.load_more_keys()?;
            if self.key_index + 1 < self.keys.len() {
                self.key_index += 1;
            }
        }
        Ok(())
    }

    pub fn prev_key(&mut self) {
        if self.key_index > 0 {
            self.key_index -= 1;
        }
    }

    pub fn first_key(&mut self) {
        self.key_index = 0;
    }

    pub fn last_key(&mut self) -> Result<()> {
        // Load all remaining keys
        while self.has_more_keys {
            self.load_more_keys()?;
        }
        if !self.keys.is_empty() {
            self.key_index = self.keys.len() - 1;
        }
        Ok(())
    }

    pub fn next_focus(&mut self) {
        self.focus = match self.focus {
            Focus::ColumnFamilies => Focus::Keys,
            Focus::Keys => Focus::Value,
            Focus::Value => Focus::ColumnFamilies,
        };
    }

    pub fn prev_focus(&mut self) {
        self.focus = match self.focus {
            Focus::ColumnFamilies => Focus::Value,
            Focus::Keys => Focus::ColumnFamilies,
            Focus::Value => Focus::Keys,
        };
    }
}
