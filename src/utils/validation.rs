use regex::Regex;

pub struct Validator;

impl Validator {
    /// Validate email format
    pub fn validate_email(email: &str) -> Result<(), String> {
        if email.is_empty() {
            return Err("Email is required".to_string());
        }

        if email.len() > 250 {
            return Err("Email is too long".to_string());
        }

        let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();

        if !email_regex.is_match(email) {
            return Err("Email is invalid".to_string());
        }

        Ok(())
    }

    /// Validate password
    pub fn validate_password(password: &str) -> Result<(), String> {
        if password.is_empty() {
            return Err("Password is required".to_string());
        }

        // Check length
        if password.len() < 6 || password.len() > 20 {
            return Err("Password must be 6-20 characters".to_string());
        }

        // Check for spaces
        if password.contains(' ') {
            return Err("Password must not contain spaces".to_string());
        }

        // Check for uppercase
        if !password.chars().any(|c| c.is_uppercase()) {
            return Err("Password must have at least 1 uppercase letter".to_string());
        }

        // Check for lowercase
        if !password.chars().any(|c| c.is_lowercase()) {
            return Err("Password must have at least 1 lowercase letter".to_string());
        }

        // Check for digit
        if !password.chars().any(|c| c.is_numeric()) {
            return Err("Password must have at least 1 number".to_string());
        }

        // Check for special character
        let special_chars = "!@#$%^&*()_+-=[]{}; ':\"\\|,.<>/?";
        if !password.chars().any(|c| special_chars.contains(c)) {
            return Err("Password must have at least 1 special character".to_string());
        }

        Ok(())
    }

    /// Validate firstname
    pub fn validate_firstname(name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("First name is required".to_string());
        }

        if name.len() < 3 || name.len() > 30 {
            return Err("Firstname must be 3-30 characters".to_string());
        }

        let name_regex = Regex::new(r"^[A-Za-z]+$").unwrap();
        if !name_regex.is_match(name) {
            return Err("Firstname must contain only letters".to_string());
        }

        Ok(())
    }

    /// Validate lastname
    pub fn validate_lastname(name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("Last name is required".to_string());
        }

        if name.len() < 1 || name.len() > 20 {
            return Err("Lastname must be 1-20 characters".to_string());
        }

        let name_regex = Regex::new(r"^[A-Za-z]+$").unwrap();
        if !name_regex.is_match(name) {
            return Err("Lastname must contain only letters".to_string());
        }

        Ok(())
    }

    /// Validate phone number
    pub fn validate_phone(ph: &str) -> Result<(), String> {
        if ph.is_empty() {
            return Err("Phone is required".to_string());
        }

        if ph.len() < 10 || ph.len() > 15 {
            return Err("Phone must be 10-15 digits".to_string());
        }

        let phone_regex = Regex::new(r"^[0-9]+$").unwrap();
        if !phone_regex.is_match(ph) {
            return Err("Phone must contain only numbers".to_string());
        }

        Ok(())
    }

    /// Validate image file type from filename
    pub fn validate_image_type(filename: &str) -> Result<(), String> {
        let allowed_extensions = ["jpg", "jpeg", "png", "webp"];

        let extension = filename.split('.').last().unwrap_or("").to_lowercase();

        if !allowed_extensions.contains(&extension.as_str()) {
            return Err("Image type must be jpeg, jpg,webp or png ".to_string());
        }

        Ok(())
    }

    
    pub fn validate_post_name(name: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("Name is required".into());
        }
        if name.len() < 2 || name.len() > 100 {
            return Err("Name length must be between 2 and 100 characters".into());
        }
        Ok(())
    }

    pub fn validate_post_description(desc: &str) -> Result<(), String> {
        if desc.trim().is_empty() {
            return Err("Description is required".into());
        }
        if desc.len() < 3 || desc.len() > 500 {
            return Err("Description length must be between 3 and 500 characters".into());
        }
        Ok(())
    }

    pub fn validate_post_images(filenames: &[String]) -> Result<(), String> {
        if filenames.is_empty() {
            return Err("At least one image is required".into());
        }
        for filename in filenames {
            Self::validate_image_type(filename)?;
        }
        Ok(())
    }


    
}


