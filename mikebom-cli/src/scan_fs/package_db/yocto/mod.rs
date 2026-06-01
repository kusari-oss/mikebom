//! Yocto / OpenEmbedded source-tree readers (milestone 107).
//!
//! Sub-modules:
//! - `context` — sysroot-vs-rootfs detection (US3, FR-005a)
//! - `manifest` — `<image>.manifest` reader (US2) — added by Phase 4
//! - `recipe` — `.bb` filename walker (US4) — added by Phase 5
//!
//! This phase (US1+US3+US5 bundled PR) introduces only `context` —
//! the opkg installed-DB reader at `package_db/opkg.rs` consumes the
//! `ScanContext` value to decide lifecycle-scope tagging.

pub(crate) mod context;
