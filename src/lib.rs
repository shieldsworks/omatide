//! omatide: tides and currents for Omahoy, predicted from NOAA's harmonic
//! constants from scratch, with a model of San Francisco Bay — the tide
//! at the boat, and the stream through the Gate, Raccoon Strait, Red Rock
//! and the rest of the bay's narrows.

pub mod astro;
pub mod bay;
pub mod cache;
pub mod config;
pub mod constituent;
pub mod coops;
pub mod engine;
pub mod fetch;
pub mod http;
pub mod json;
pub mod keel;
pub mod predict;
pub mod station;
pub mod subordinate;
pub mod tides;
pub mod time;
