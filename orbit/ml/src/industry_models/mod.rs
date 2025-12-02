//! Industry-specific machine learning models
//!
//! Organized into 15 major industry categories following standard taxonomy:
//!
//! 1. **Technology, Media & Internet** - Software, platforms, social media, gaming, entertainment
//! 2. **Finance, Banking & Insurance** - Banking, capital markets, insurance, fintech
//! 3. **Healthcare, Pharma & Life Sciences** - Hospitals, pharma, biotech, medical devices
//! 4. **Retail, E-Commerce & Consumer Goods** - Retail, e-commerce, CPG, fashion
//! 5. **Transportation, Logistics & Travel** - Logistics, airlines, ride-sharing, hospitality
//! 6. **Manufacturing, Industrial & Energy** - Manufacturing, robotics, oil & gas, utilities, clean energy
//! 7. **Agriculture, Food & Environment** - Agriculture, food, fisheries, climate
//! 8. **Construction, Real Estate & Smart Cities** - Construction, real estate, urban planning
//! 9. **Telecom, Networking & Hardware** - Telecom, semiconductors, cloud infrastructure
//! 10. **Education, Training & HR** - EdTech, corporate learning, recruiting
//! 11. **Government, Defense & Public Sector** - Public admin, defense, law enforcement
//! 12. **Legal, Compliance & Professional Services** - Legal, consulting, audit
//! 13. **Arts, Design & Creative Industries** - Advertising, design, publishing
//! 14. **Consumer Apps & Daily-Life Services** - Personal finance, health apps, home IoT
//! 15. **Cross-Cutting Horizontal** - Forecasting, recommendations, anomaly detection, CV, NLP

// Common infrastructure
/// Common traits and types shared across industry models
pub mod common;

// Re-export common types
pub use common::{
    IndustryModel, IndustryModelError, ModelConfig, ModelMetrics, ModelRegistry, Result,
    TrainingConfig,
};

// 15 Major Industry Categories
pub mod agriculture_food_environment;
pub mod arts_design_creative;
pub mod construction_realestate_smartcities;
pub mod consumer_apps_dailylife;
pub mod cross_cutting_horizontal;
pub mod education_training_hr;
pub mod finance_banking_insurance;
pub mod government_defense_publicsector;
pub mod healthcare_pharma_lifesciences;
pub mod legal_compliance_professional;
pub mod manufacturing_industrial_energy;
pub mod retail_ecommerce_consumer;
pub mod technology_media_internet;
pub mod telecom_networking_hardware;
pub mod transportation_logistics_travel;
