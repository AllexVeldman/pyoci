// Return the current datetime in UTC
#[cfg(not(test))]
pub fn now_utc() -> time::UtcDateTime {
    time::UtcDateTime::now()
}

#[cfg(test)]
pub use mocks::now_utc;

#[cfg(test)]
pub use mocks::set_timestamp;

// Get the unix time in nanoseconds
// u64 is good until the year 2554.
// if you crash here this project has exceeded my wildest expectations, congratulations to me!
pub fn time_unix_ns() -> u64 {
    now_utc()
        .unix_timestamp_nanos()
        .try_into()
        .expect("Could not fit the current unix timestamp into a u64")
}

#[cfg(test)]
mod mocks {
    use std::cell::Cell;

    thread_local! {
        // Tread-local timestamp that can be set during testing
        static TIMESTAMP: Cell<i64> = const { Cell::new(0) };
    }

    // Return the TIMESTAMP as a UtcDateTime
    // Use mocks.set_timestamp() to set the time to a fixed timestamp
    pub fn now_utc() -> time::UtcDateTime {
        TIMESTAMP
            .with(|timestamp| time::UtcDateTime::from_unix_timestamp(timestamp.get()))
            .expect("a valid timestamp")
    }

    pub fn set_timestamp(timestamp: i64) {
        TIMESTAMP.with(|ts| ts.set(timestamp));
    }
}
