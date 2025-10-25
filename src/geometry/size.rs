// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use num_traits::Zero;
use std::{fmt, ops::Mul};

use super::super::impl_scalar_mul;
use super::vector2d::Vector2D;

#[derive(Copy, Clone, PartialEq)]
pub struct GenericSize<T> {
    width: T,
    height: T,
}

impl<T> GenericSize<T>
where
    T: Copy + PartialOrd + Zero,
{
    pub fn new(width: T, height: T) -> Self {
        Self {
            width: Self::clamp(width),
            height: Self::clamp(height),
        }
    }

    pub fn width(&self) -> T {
        self.width
    }

    pub fn height(&self) -> T {
        self.height
    }

    pub fn set_width(&mut self, width: T) {
        self.width = Self::clamp(width);
    }

    pub fn set_height(&mut self, height: T) {
        self.height = Self::clamp(height);
    }

    fn clamp(value: T) -> T {
        if value < T::zero() {
            T::zero()
        } else {
            value
        }
    }
}

impl<T> Vector2D<T> for GenericSize<T>
where
    T: Copy + PartialOrd + Zero,
{
    fn x(&self) -> T {
        self.width()
    }
    fn y(&self) -> T {
        self.height()
    }

    fn from_components(x: T, y: T) -> Self {
        Self::new(x, y)
    }
}

impl<T> Default for GenericSize<T>
where
    T: Copy + PartialOrd + Zero,
{
    fn default() -> Self {
        Self::new(T::zero(), T::zero())
    }
}

impl_scalar_mul!(GenericSize<i32>, f32, true);
impl_scalar_mul!(GenericSize<u32>, f32, true);
impl_scalar_mul!(GenericSize<f32>, f32, false);

impl<T> fmt::Debug for GenericSize<T>
where
    T: Copy + PartialOrd + Zero + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Size")
            .field("w", &self.width())
            .field("h", &self.height())
            .finish()
    }
}
