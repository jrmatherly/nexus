//! AI21 Labs model family support for AWS Bedrock.
//!
//! This module provides support for AI21's Jamba family of models through AWS Bedrock.

pub mod input;
pub mod output;

pub(crate) use input::JambaRequest;
pub(crate) use output::{AI21StreamEvent, JambaResponse};
