//! GOD format subset of iliazeus/iso2god-rs, commit
//! 76207d7d7f89f551c7a5769560c0073e9ffe50e9. See LICENSE.
//! Harbor owns source validation, orchestration and output lifecycle.
pub mod god;
pub mod executable {
    #[derive(Clone, Debug)]
    pub struct TitleExecutionInfo {
        pub media_id: u32,
        pub version: u32,
        pub base_version: u32,
        pub title_id: u32,
        pub platform: u8,
        pub executable_type: u8,
        pub disc_number: u8,
        pub disc_count: u8,
    }
}
