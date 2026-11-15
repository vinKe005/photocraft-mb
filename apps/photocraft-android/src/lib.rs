//! Native Android host. The editor and engine retain their unsafe-code prohibition.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
mod content;

#[cfg(target_os = "android")]
mod gallery;

#[cfg(any(target_os = "android", test))]
mod stream;

#[cfg(target_os = "android")]
mod storage;
