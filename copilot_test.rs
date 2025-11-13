// GitHub Copilot Test File
// This file is designed to test GitHub Copilot functionality in Zed

use std::collections::HashMap;

/// A simple struct to test Copilot suggestions
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub age: u32,
}

impl User {
    /// Create a new user
    pub fn new(id: u64, name: String, email: String, age: u32) -> Self {
        Self {
            id,
            name,
            email,
            age,
        }
    }

    /// Check if user is adult
    pub fn is_adult(&self) -> bool {
        self.age >= 18
    }

    // TODO: Add a method to validate email format
    // Start typing here to test Copilot suggestions:
    // pub fn validate_email(&self) -> bool {

    // TODO: Add a method to get user's display name
    // pub fn display_name(&self) -> String {
}

/// A simple function to test Copilot code completion
/// Try typing the following patterns to test Copilot:
/// 1. "// Calculate the factorial of n"
/// 2. "// Sort a vector of numbers"
/// 3. "// Parse JSON from string"
pub fn test_copilot_suggestions() {
    // Start typing common programming patterns here
    let numbers = vec![3, 1, 4, 1, 5, 9, 2, 6];

    // Type: "let sorted = " and see if Copilot suggests sorting

    // Type: "// Create a HashMap" and see suggestions

    // Type: "for i in 0..10 {" and see loop suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_creation() {
        let user = User::new(
            1,
            "John Doe".to_string(),
            "john@example.com".to_string(),
            25,
        );
        assert_eq!(user.id, 1);
        assert_eq!(user.name, "John Doe");
        assert!(user.is_adult());
    }

    // Try adding more test functions here to test Copilot's test generation
}
