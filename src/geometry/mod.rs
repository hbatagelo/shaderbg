// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(test)]
mod tests {
    mod offset;
    mod point;
    mod rectangle;
    mod size;
}
mod offset;
mod point;
mod rectangle;
mod size;
mod vector2d;

use offset::GenericOffset;
use point::GenericPoint;
use rectangle::GenericRectangle;
use size::GenericSize;

pub type Offset = GenericOffset<i32>;
pub type Point = GenericPoint<i32>;
pub type Rectangle = GenericRectangle<i32>;
pub type Size = GenericSize<u32>;
pub type SizeI = GenericSize<i32>;
