# Erofs Rust Userspace Library

This Repository aims to rewrite the original Extended Read-only FileSystem Userspace Library and Tools using Rust. It comes with total rewrite of implementation logic with the causion of just a little unsafe code to make sure its compatibility with on-disk data layouts.Much or the `erofs-sys` will be embedded into the Linux kernel to rewrite the logic of the original C implementation.

Notes: The MSRV for this repository is always kept in sync with the Rust For Linux MSRV for compatibility issues.
