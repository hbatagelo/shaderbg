// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use num_traits::Zero;
use std::{
    fmt,
    ops::{Mul, Neg},
};

use super::super::impl_scalar_mul;
use super::{vector2d::Vector2D, GenericPoint};

#[derive(Default, Copy, Clone, PartialEq)]
pub struct GenericOffset<T> {
    dx: T,
    dy: T,
}

impl<T> GenericOffset<T>
where
    T: Copy,
{
    pub fn new(dx: T, dy: T) -> Self {
        Self { dx, dy }
    }

    pub fn dx(&self) -> T {
        self.dx
    }

    pub fn dy(&self) -> T {
        self.dy
    }
}

impl<T> Vector2D<T> for GenericOffset<T>
where
    T: Copy + PartialOrd + Zero,
{
    fn x(&self) -> T {
        self.dx()
    }
    fn y(&self) -> T {
        self.dy()
    }

    fn from_components(x: T, y: T) -> Self {
        Self::new(x, y)
    }
}

impl<T> From<GenericPoint<T>> for GenericOffset<T>
where
    T: Copy,
{
    fn from(src: GenericPoint<T>) -> Self {
        Self::new(src.x(), src.y())
    }
}

impl<T> Neg for GenericOffset<T>
where
    T: Neg<Output = T> + Copy,
{
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.dx(), -self.dy())
    }
}

impl_scalar_mul!(GenericOffset<i32>, f32, true);
impl_scalar_mul!(GenericOffset<f32>, f32, false);

impl<T> fmt::Debug for GenericOffset<T>
where
    T: Copy + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Offset")
            .field("dx", &self.dx())
            .field("dy", &self.dy())
            .finish()
    }
}
