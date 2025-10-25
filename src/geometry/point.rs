// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fmt,
    ops::{Add, Mul, Sub},
};

use super::super::impl_scalar_mul;
use super::{offset::GenericOffset, vector2d::Vector2D};

#[derive(Default, Copy, Clone, PartialEq)]
pub struct GenericPoint<T> {
    x: T,
    y: T,
}

impl<T> GenericPoint<T>
where
    T: Copy,
{
    pub fn new(x: T, y: T) -> Self {
        Self { x, y }
    }

    pub fn x(&self) -> T {
        self.x
    }

    pub fn y(&self) -> T {
        self.y
    }
}

impl<T> Vector2D<T> for GenericPoint<T>
where
    T: Copy,
{
    fn x(&self) -> T {
        self.x()
    }
    fn y(&self) -> T {
        self.y()
    }

    fn from_components(x: T, y: T) -> Self {
        Self::new(x, y)
    }
}

impl<T> Add<GenericOffset<T>> for GenericPoint<T>
where
    T: Add<Output = T> + Copy,
{
    type Output = Self;

    fn add(self, offset: GenericOffset<T>) -> Self::Output {
        Self::new(self.x() + offset.dx(), self.y() + offset.dy())
    }
}

impl<T> Sub<GenericOffset<T>> for GenericPoint<T>
where
    T: Sub<Output = T> + Copy,
{
    type Output = Self;

    fn sub(self, offset: GenericOffset<T>) -> Self::Output {
        Self::new(self.x() - offset.dx(), self.y() - offset.dy())
    }
}

impl_scalar_mul!(GenericPoint<i32>, f32, true);
impl_scalar_mul!(GenericPoint<f32>, f32, false);

impl<T> fmt::Debug for GenericPoint<T>
where
    T: Copy + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Point")
            .field("x", &self.x())
            .field("y", &self.y())
            .finish()
    }
}
