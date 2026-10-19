//! Parsing and data model shared between levels.
//!
//! Later levels reuse the input format and data model of earlier ones. When something recurs,
//! move it here (a struct with a `parse` function, a geometry helper, ...) and use it from every
//! level as `crate::model::...` instead of copying code between level files.
#![allow(dead_code)]
