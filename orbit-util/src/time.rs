use chrono::{DateTime, Duration, Utc};
use std::time::SystemTime;

/// Time utilities for working with timestamps and durations
pub struct TimeUtils;

impl TimeUtils {
    /// Get current UTC timestamp
    pub fn now() -> DateTime<Utc> {
        Utc::now()
    }

    /// Convert SystemTime to DateTime<Utc>
    pub fn from_system_time(system_time: SystemTime) -> DateTime<Utc> {
        system_time.into()
    }

    /// Get timestamp in milliseconds since Unix epoch
    pub fn timestamp_millis() -> i64 {
        Utc::now().timestamp_millis()
    }

    /// Create a Duration from milliseconds
    pub fn duration_from_millis(millis: i64) -> Duration {
        Duration::milliseconds(millis)
    }

    /// Create a Duration from seconds
    pub fn duration_from_secs(secs: i64) -> Duration {
        Duration::seconds(secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now() {
        let now1 = TimeUtils::now();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let now2 = TimeUtils::now();
        assert!(now2 > now1);
    }

    #[test]
    fn test_timestamp_millis() {
        let ts = TimeUtils::timestamp_millis();
        assert!(ts > 0);
    }

    #[test]
    fn test_duration_from_millis() {
        let duration = TimeUtils::duration_from_millis(5000);
        assert_eq!(duration.num_seconds(), 5);
    }
}