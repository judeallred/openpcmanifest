//! `OpenPCManifest`: an open-source Speccy alternative.
//!
//! The crate is organised as a pipeline:
//!
//! 1. [`collect`] gathers a raw [`collect::Snapshot`] (Windows only).
//! 2. [`smbios`], [`smart`] and [`cpu`] decode the binary parts.
//! 3. [`build`] turns the snapshot into a [`model::Report`] tree.
//! 4. [`render`] writes the tree as text, Markdown, `BBCode`, JSON or HTML,
//!    and [`diff`] compares two reports.
//!
//! Everything after collection is pure and runs on any platform.

pub mod base64;
pub mod build;
pub mod cli;
pub mod collect;
pub mod cpu;
pub mod diff;
#[cfg(feature = "gui")]
pub mod gui;
pub mod json;
pub mod model;
pub mod render;
pub mod smart;
pub mod smbios;
pub mod util;

/// The product name.
pub const NAME: &str = "OpenPCManifest";

/// The crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The project home page.
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// The user guide.
pub const WIKI_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/wiki");

/// Where to report bugs and request features.
pub const ISSUES_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new/choose");

/// The name and full build identifier, such as
/// `OpenPCManifest 0.1.0-nightly.20261115.42+1a2b3c4`. CI sets
/// `OPM_BUILD` for nightly builds; local builds show the crate version.
#[must_use]
pub fn build_string() -> String {
    match option_env!("OPM_BUILD") {
        Some(b) if !b.trim().is_empty() => format!("{NAME} {}", b.trim()),
        _ => format!("{NAME} {VERSION}"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn build_string_names_the_product() {
        assert!(super::build_string().starts_with("OpenPCManifest "));
        assert!(super::ISSUES_URL.ends_with("/issues/new/choose"));
    }
}
