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
        
        // Boot Verification: Test physical write access
        let test_file = base.join("tmp").join(".totebox_io_test");
        fs::File::create(&test_file)?.sync_all()?;
        fs::remove_file(test_file)?;

        Ok(MaildirVault { base_path: base })
    }

    pub fn write_payload(&self, raw_data: &str) -> std::io::Result<u64> {
        let timestamp = Utc::now().timestamp();
        let unique_id = Uuid::new_v4().to_string();
        let filename = format!("{}.{}_V1_TOTEBOX", timestamp, unique_id);
        
        let tmp_path = self.base_path.join("tmp").join(&filename);
        let new_path = self.base_path.join("new").join(&filename);

        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(raw_data.as_bytes())?;
        file.sync_all()?;
        
        let metadata = fs::metadata(&tmp_path)?;
        let file_size = metadata.len();

        fs::rename(tmp_path, new_path)?;
        
        Ok(file_size)
    }
}
