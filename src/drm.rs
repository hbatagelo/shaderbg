// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! DRM/KMS output detection utilities.
//!
//! Provides minimal helpers for querying `/dev/dri/card*` devices to
//! determine whether a connected display output is available.
//!
//! Used during startup to decide whether DRM-backed rendering can be
//! initialized or if fallback monitor detection should be used.

use drm::{
    control::{connector::State, Device as ControlDevice},
    Device,
};
use std::{error::Error, fs::*, os::fd::*};

/// Thin wrapper around a DRM device node (`/dev/dri/card*`).
///
/// This type exists solely to attach the `drm-rs` traits to a `File`
/// descriptor so DRM queries can be performed.
///
/// The file is opened read-only since only enumeration is required.
#[derive(Debug)]
struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

/// Enables basic DRM device queries (resource enumeration).
impl Device for Card {}

/// Enables DRM control queries such as connectors and encoders.
impl ControlDevice for Card {}

impl Card {
    /// Attempts to open a DRM card device.
    ///
    /// Example paths:
    /// - `/dev/dri/card0`
    /// - `/dev/dri/card1`
    ///
    /// Fails if the device does not exist or permission is denied.
    fn try_open(path: &str) -> Result<Self, Box<dyn Error>> {
        let mut options = OpenOptions::new();
        options.read(true);
        options.write(false);
        let file = options.open(path)?;
        Ok(Card(file))
    }
}

/// Detects whether at least one DRM display output is currently active.
///
/// A connector is considered usable when:
/// - it reports `Connected` state (physical display attached), and
/// - an encoder is assigned (display pipeline configured).
///
/// Returns:
/// - `Ok(true)` if at least one active output was detected
/// - `Ok(false)` if the DRM devices are accessible but with no connected displays
/// - `Err(_)` if no DRM devices could be accessed
///
/// This function is used to determine whether a DRM/KMS rendering backend can be initialized.
pub fn has_connected_drm_output() -> Result<bool, Box<dyn Error>> {
    let mut any_card_accessible = false;

    // Maximum number of DRM card devices probed
    const MAX_DRM_CARDS: u32 = 4;

    for card_num in 0..MAX_DRM_CARDS {
        let card_path = format!("/dev/dri/card{card_num}");

        match Card::try_open(&card_path) {
            Ok(card) => {
                any_card_accessible = true;
                let resource_handles = card.resource_handles()?;

                for conn in resource_handles.connectors() {
                    let info = card.get_connector(*conn, false)?;
                    // Check if:
                    // 1. The connector has something physically connected (cable plugged in)
                    // 2. The connector has an encoder assigned (ready for display output)
                    if info.state() == State::Connected && info.current_encoder().is_some() {
                        return Ok(true);
                    }
                }
            }
            // Ignore missing devices or permission failures
            Err(_) => continue,
        }
    }

    if !any_card_accessible {
        Err("No DRM cards accessible".into())
    } else {
        // Cards were readable, but no connected outputs exist
        Ok(false)
    }
}
