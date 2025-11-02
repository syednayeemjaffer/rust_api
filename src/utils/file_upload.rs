use std::fs;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn save_profile_image(bytes: Vec<u8>, original_name: &str) -> Result<String, String> {
    // Get current timestamp in seconds
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Failed to get current time")?
        .as_secs();

    // Build a filename using timestamp + original name
    let new_filename = format!("{}_{}", timestamp, original_name);
    let dir_path = "files/usersProfiles";

    // Create the directory if not exists
    fs::create_dir_all(dir_path).map_err(|_| "Failed to create upload directory")?;

    // Write the file
    let full_path = format!("{}/{}", dir_path, new_filename);
    let mut file = fs::File::create(&full_path).map_err(|_| "Failed to create image file")?;
    file.write_all(&bytes)
        .map_err(|_| "Failed to write image file")?;

    Ok(new_filename)
}
