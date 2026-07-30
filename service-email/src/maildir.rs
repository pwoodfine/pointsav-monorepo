use std::fs;
use std::path::{Path, PathBuf};
use std::io::Write;
use uuid::Uuid;
use chrono::Utc;

pub struct MaildirVault {
    base_path: PathBuf,
}

impl MaildirVault {
    pub fn init<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let base = path.as_ref().to_path_buf();
        fs::create_dir_all(base.join("tmp"))?;
        fs::create_dir_all(base.join("new"))?;
        fs::create_dir_all(base.join("cur"))?;
        
        Ok(MaildirVault { base_path: base })
    }

    pub fn write_payload(&self, raw_data: &str) -> std::io::Result<()> {
        let timestamp = Utc::now().timestamp();
        let unique_id = Uuid::new_v4().to_string();
        let hostname = "node-imac-12.pointsav.local";
        
        let filename = format!("{}.{}_{}.{}", timestamp, unique_id, "V1", hostname);
        let tmp_path = self.base_path.join("tmp").join(&filename);
        let new_path = self.base_path.join("new").join(&filename);

        // Write to tmp first per Maildir specification
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(raw_data.as_bytes())?;
        file.sync_all()?;

        // Atomically move to new
        fs::rename(tmp_path, new_path)?;
        
        Ok(())
    }
}
