use anyhow::{Context, Result};
use rocksdb::{DBWithThreadMode, MultiThreaded, Options};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub struct SecondaryDb {
    db: DBWithThreadMode<MultiThreaded>,
    column_families: Vec<String>,
}

impl SecondaryDb {
    pub fn open(db_path: &Path, secondary_path: Option<&Path>) -> Result<Self> {
        let secondary_path = match secondary_path {
            Some(p) => p.to_path_buf(),
            None => Self::default_secondary_path(db_path),
        };

        // Create secondary directory if not exists
        std::fs::create_dir_all(&secondary_path)
            .with_context(|| format!("Failed to create secondary path: {:?}", secondary_path))?;

        // List existing column families
        let cf_names = DBWithThreadMode::<MultiThreaded>::list_cf(&Options::default(), db_path)
            .unwrap_or_else(|_| vec!["default".to_string()]);

        // Open as secondary
        let mut opts = Options::default();
        opts.create_if_missing(false);

        let cf_descriptors: Vec<_> = cf_names
            .iter()
            .map(|name| rocksdb::ColumnFamilyDescriptor::new(name, Options::default()))
            .collect();

        let db = DBWithThreadMode::<MultiThreaded>::open_cf_descriptors_as_secondary(
            &opts,
            db_path,
            &secondary_path,
            cf_descriptors,
        )
        .with_context(|| format!("Failed to open database as secondary: {:?}", db_path))?;

        Ok(Self {
            db,
            column_families: cf_names,
        })
    }

    fn default_secondary_path(db_path: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        db_path.hash(&mut hasher);
        let hash = hasher.finish();
        PathBuf::from(format!("/tmp/rocksdb-tui-{:x}", hash))
    }

    pub fn column_families(&self) -> &[String] {
        &self.column_families
    }

    pub fn try_catch_up_with_primary(&self) -> Result<()> {
        self.db
            .try_catch_up_with_primary()
            .context("Failed to catch up with primary")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get(&self, cf_name: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .with_context(|| format!("Column family not found: {}", cf_name))?;
        let value = self.db.get_cf(&cf, key)?;
        Ok(value)
    }

    pub fn estimate_num_keys(&self, cf_name: &str) -> Option<u64> {
        let cf = self.db.cf_handle(cf_name)?;
        self.db
            .property_int_value_cf(&cf, "rocksdb.estimate-num-keys")
            .ok()
            .flatten()
    }

    pub fn iter_keys(
        &self,
        cf_name: &str,
        start_key: Option<&[u8]>,
        limit: usize,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .with_context(|| format!("Column family not found: {}", cf_name))?;

        let iter = self.db.iterator_cf(&cf, rocksdb::IteratorMode::Start);

        let mut results = Vec::with_capacity(limit);
        let iter = iter.into_iter();

        // Skip to start_key if provided
        if let Some(start) = start_key {
            let iter_from = self.db.iterator_cf(
                &cf,
                rocksdb::IteratorMode::From(start, rocksdb::Direction::Forward),
            );
            let mut iter_from = iter_from.into_iter();
            // Skip the start_key itself
            iter_from.next();
            for item in iter_from.take(limit) {
                let (k, v) = item?;
                results.push((k.to_vec(), v.to_vec()));
            }
        } else {
            for item in iter.take(limit) {
                let (k, v) = item?;
                results.push((k.to_vec(), v.to_vec()));
            }
        }

        Ok(results)
    }

    pub fn iter_keys_with_prefix(
        &self,
        cf_name: &str,
        prefix: &[u8],
        start_key: Option<&[u8]>,
        limit: usize,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .with_context(|| format!("Column family not found: {}", cf_name))?;

        // Start from start_key if provided, otherwise from prefix
        let from_key = start_key.unwrap_or(prefix);
        let iter = self.db.iterator_cf(
            &cf,
            rocksdb::IteratorMode::From(from_key, rocksdb::Direction::Forward),
        );

        let mut results = Vec::with_capacity(limit);
        let mut iter = iter.into_iter();

        // Skip start_key itself if provided
        if start_key.is_some() {
            iter.next();
        }

        for item in iter.take(limit) {
            let (k, v) = item?;
            if !k.starts_with(prefix) {
                break;
            }
            results.push((k.to_vec(), v.to_vec()));
        }

        Ok(results)
    }
}
