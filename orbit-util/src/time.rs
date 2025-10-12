use chrono::{DateTime, Duration, Utc};
use std::time::SystemTime;

/// Time utilities for working with timestamps and durations
pub struct TimeUtils;

impl TimeUtils {
    /// Get current UTC timestamp
    pub fn now() -> DateTime<Utc> {
        Utc::now()
    }

    /// Convert SystemTime to `DateTime<Utc>`
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
    use std::time::UNIX_EPOCH;

    #[test]
    fn test_now() {
        let now1 = TimeUtils::now();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let now2 = TimeUtils::now();
        assert!(now2 > now1);

        // Ensure it's roughly current time (within last minute)
        let very_recent = Utc::now() - Duration::minutes(1);
        assert!(now1 > very_recent);
        assert!(now2 > very_recent);
    }

    #[test]
    fn test_timestamp_millis() {
        let ts1 = TimeUtils::timestamp_millis();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let ts2 = TimeUtils::timestamp_millis();

        assert!(ts1 > 0);
        assert!(ts2 >= ts1); // Should be same or later

        // Should be reasonable current timestamp (after 2020)
        let year_2020_millis = 1_577_836_800_000i64; // Jan 1, 2020
        assert!(ts1 > year_2020_millis);
    }

    #[test]
    fn test_duration_from_millis() {
        let duration = TimeUtils::duration_from_millis(5000);
        assert_eq!(duration.num_seconds(), 5);
        assert_eq!(duration.num_milliseconds(), 5000);

        // Test edge cases
        let zero_duration = TimeUtils::duration_from_millis(0);
        assert_eq!(zero_duration.num_milliseconds(), 0);

        let negative_duration = TimeUtils::duration_from_millis(-1000);
        assert_eq!(negative_duration.num_seconds(), -1);
        assert_eq!(negative_duration.num_milliseconds(), -1000);

        // Test large values
        let large_duration = TimeUtils::duration_from_millis(86400000); // 24 hours
        assert_eq!(large_duration.num_days(), 1);
    }

    #[test]
    fn test_duration_from_secs() {
        let duration = TimeUtils::duration_from_secs(60);
        assert_eq!(duration.num_minutes(), 1);
        assert_eq!(duration.num_seconds(), 60);

        // Test edge cases
        let zero_duration = TimeUtils::duration_from_secs(0);
        assert_eq!(zero_duration.num_seconds(), 0);

        let negative_duration = TimeUtils::duration_from_secs(-30);
        assert_eq!(negative_duration.num_seconds(), -30);

        // Test large values
        let hour_duration = TimeUtils::duration_from_secs(3600);
        assert_eq!(hour_duration.num_hours(), 1);

        let day_duration = TimeUtils::duration_from_secs(86400);
        assert_eq!(day_duration.num_days(), 1);
    }

    #[test]
    fn test_from_system_time() {
        let system_now = SystemTime::now();
        let dt = TimeUtils::from_system_time(system_now);

        // Should be recent
        let now = TimeUtils::now();
        let diff = now - dt;
        assert!(diff.num_seconds().abs() < 2); // Within 2 seconds

        // Test Unix epoch
        let epoch_dt = TimeUtils::from_system_time(UNIX_EPOCH);
        assert_eq!(epoch_dt.timestamp(), 0);

        // Test future time
        let future_system_time = SystemTime::now() + std::time::Duration::from_secs(3600);
        let future_dt = TimeUtils::from_system_time(future_system_time);
        assert!(future_dt > now);
    }

    #[test]
    fn test_timestamp_consistency() {
        let dt = TimeUtils::now();
        let ts_millis = dt.timestamp_millis();
        let util_ts_millis = TimeUtils::timestamp_millis();

        // Should be within a few milliseconds of each other
        let diff = (ts_millis - util_ts_millis).abs();
        assert!(diff < 1000, "Timestamp difference too large: {} ms", diff);
    }

    #[test]
    fn test_duration_operations() {
        let d1 = TimeUtils::duration_from_secs(30);
        let d2 = TimeUtils::duration_from_millis(15000); // 15 seconds

        assert_eq!(d1.num_seconds(), 30);
        assert_eq!(d2.num_seconds(), 15);

        // Test arithmetic (chrono::Duration supports these operations)
        let sum = d1 + d2;
        assert_eq!(sum.num_seconds(), 45);

        let diff = d1 - d2;
        assert_eq!(diff.num_seconds(), 15);
    }

    #[test]
    fn test_datetime_arithmetic() {
        let base_time = TimeUtils::now();
        let duration = TimeUtils::duration_from_secs(3600); // 1 hour

        let future_time = base_time + duration;
        let past_time = base_time - duration;

        assert!(future_time > base_time);
        assert!(past_time < base_time);

        let time_diff = future_time - past_time;
        assert_eq!(time_diff.num_hours(), 2); // 2 hours apart
    }

    #[test]
    fn test_precision() {
        // Test millisecond precision
        let d1 = TimeUtils::duration_from_millis(1500); // 1.5 seconds
        assert_eq!(d1.num_milliseconds(), 1500);
        assert_eq!(d1.num_seconds(), 1); // Truncated to whole seconds

        // Test that we can represent fractional seconds
        let base = TimeUtils::now();
        let with_millis = base + TimeUtils::duration_from_millis(500);
        let diff = with_millis - base;
        assert_eq!(diff.num_milliseconds(), 500);
    }

    #[test]
    fn test_large_timestamps() {
        // Test that we can handle large timestamp values without overflow
        let large_millis = TimeUtils::duration_from_millis(i64::MAX / 1000);
        assert!(large_millis.num_seconds() > 0);

        let large_secs = TimeUtils::duration_from_secs(i64::MAX / 1000000);
        assert!(large_secs.num_days() > 0);
    }
}
