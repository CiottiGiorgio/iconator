use const_format::formatcp;

pub const CACHE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;  // One week
pub const IMMUTABLE: &str = formatcp!("public, max-age={MAX_AGE_SECS}, immutable");

pub const FILE_ENDPOINT: &str = "/file";
pub const FOLDER_ENDPOINT: &str = "/folder";
pub const SVG_ENDPOINT: &str = "/svg/{id}";
