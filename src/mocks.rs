use std::cell::Cell;

thread_local! {
    static TIMESTAMP: Cell<i64> = const { Cell::new(0) };
}
/// Mock of OffsetDateTime::now_utc
/// Use set_timestamp to manipulate now_utc
pub struct OffsetDateTime {}

impl OffsetDateTime {
    pub fn now_utc() -> time::OffsetDateTime {
        TIMESTAMP
            .with(|timestamp| time::OffsetDateTime::from_unix_timestamp(timestamp.get()))
            .expect("a valid timestamp")
    }
}

pub fn set_timestamp(timestamp: i64) {
    TIMESTAMP.with(|ts| ts.set(timestamp));
}
