use std::fs;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn save_multiple_images(files: Vec<(Vec<u8>, String)>) -> Result<Vec<String>, String> {
    let dir = "files/userPost";
    fs::create_dir_all(dir).map_err(|_| "Failed to create directory")?;

    let mut saved_names = Vec::new();
    for (bytes, original_name) in files {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "Failed to get unix time")?
            .as_secs();
        let fname = format!("{}_{}", timestamp, original_name);
        let path = format!("{}/{}", dir, fname);

        let mut f = fs::File::create(&path).map_err(|_| "Failed to create image file")?;
        f.write_all(&bytes).map_err(|_| "Failed to write image data")?;

        saved_names.push(fname);
    }

    Ok(saved_names)
}
