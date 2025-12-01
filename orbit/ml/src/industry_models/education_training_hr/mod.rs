//! Education, Training & HR industry models
//!
//! Comprehensive ML models for education and human resources including:
//! - Education Tech & Universities
//! - Corporate Learning & L&D
//! - HR, Recruiting & Talent Management

pub mod corporate_learning;
pub mod education_tech;
pub mod hr_recruiting;

// Re-export commonly used types
pub use corporate_learning::*;
pub use education_tech::*;
pub use hr_recruiting::*;
