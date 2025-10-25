// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use drm::{
    control::{connector::State, Device as ControlDevice},
    Device,
};
use std::{error::Error, fs::*, os::fd::*};

#[derive(Debug)]
struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl Device for Card {}
impl ControlDevice for Card {}

impl Card {
    fn try_open(path: &str) -> Result<Self, Box<dyn Error>> {
        let mut options = OpenOptions::new();
        options.read(true);
        options.write(false);
        let file = options.open(path)?;
        Ok(Card(file))
    }
}

pub fn has_connected_drm_output() -> Result<bool, Box<dyn Error>> {
    let mut any_card_accessible = false;

    const MAX_DRM_CARDS: u32 = 4;

    for card_num in 0..MAX_DRM_CARDS {
        let card_path = format!("/dev/dri/card{card_num}");

        match Card::try_open(&card_path) {
            Ok(card) => {
                any_card_accessible = true;
                let resource_handles = card.resource_handles()?;

                for conn in resource_handles.connectors() {
                    let info = card.get_connector(*conn, false)?;
                    if info.state() == State::Connected && info.current_encoder().is_some() {
                        return Ok(true);
                    }
                }
            }
            Err(_) => continue,
        }
    }

    if !any_card_accessible {
        Err("No DRM cards accessible".into())
    } else {
        Ok(false)
    }
}
