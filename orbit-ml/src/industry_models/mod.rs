#[cfg(feature = "industry-healthcare")]
pub mod healthcare {
    pub struct HealthcareModels;
}

#[cfg(feature = "industry-fintech")]
pub mod fintech {
    pub struct FintechModels;
}

#[cfg(feature = "industry-adtech")]
pub mod adtech {
    pub struct AdtechModels;
}

#[cfg(feature = "industry-defense")]
pub mod defense {
    pub struct DefenseModels;
}

#[cfg(feature = "industry-logistics")]
pub mod logistics {
    pub struct LogisticsModels;
}

#[cfg(feature = "industry-banking")]
pub mod banking {
    pub struct BankingModels;
}

#[cfg(feature = "industry-insurance")]
pub mod insurance {
    pub struct InsuranceModels;
}
