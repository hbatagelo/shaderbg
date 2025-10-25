// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use num_traits::Zero;
use std::{
    fmt,
    ops::{Add, Sub},
};

use super::{GenericOffset, GenericPoint, GenericSize, Point};

#[derive(Default, Copy, Clone, PartialEq)]
pub struct GenericRectangle<T>
where
    T: Copy + PartialOrd + Zero,
{
    top_left: GenericPoint<T>,
    size: GenericSize<T>,
}

impl<T> GenericRectangle<T>
where
    T: Copy + PartialOrd + Zero,
{
    pub fn new(top_left: GenericPoint<T>, size: GenericSize<T>) -> Self {
        Self { top_left, size }
    }

    pub fn top_left(&self) -> GenericPoint<T> {
        self.top_left
    }

    pub fn size(&self) -> GenericSize<T> {
        self.size
    }

    pub fn left(&self) -> T {
        self.top_left.x()
    }

    pub fn top(&self) -> T {
        self.top_left.y()
    }

    pub fn right(&self) -> T {
        self.top_left.x() + self.size.width()
    }

    pub fn bottom(&self) -> T {
        self.top_left.y() + self.size.height()
    }

    pub fn width(&self) -> T {
        self.size.width()
    }

    pub fn height(&self) -> T {
        self.size.height()
    }
}

impl From<gtk::gdk::Rectangle> for GenericRectangle<i32> {
    fn from(src: gtk::gdk::Rectangle) -> Self {
        Self::new(
            Point::new(src.x(), src.y()),
            GenericSize::<i32>::new(src.width(), src.height()),
        )
    }
}

impl<T> Add<GenericOffset<T>> for GenericRectangle<T>
where
    T: Add<Output = T> + Copy + PartialOrd + Zero,
{
    type Output = Self;

    fn add(self, offset: GenericOffset<T>) -> Self::Output {
        Self::new(self.top_left() + offset, self.size())
    }
}

impl<T> Sub<GenericOffset<T>> for GenericRectangle<T>
where
    T: Sub<Output = T> + Copy + PartialOrd + Zero,
{
    type Output = Self;

    fn sub(self, offset: GenericOffset<T>) -> Self::Output {
        Self::new(self.top_left() - offset, self.size())
    }
}

impl<T> GenericRectangle<T>
where
    T: Copy + Add<Output = T> + Sub<Output = T> + MinMax + PartialOrd + Zero,
{
    pub fn union(&self, other: &Self) -> Self {
        let x1 = self.left().min(other.left());
        let y1 = self.top().min(other.top());

        let x2 = self.right().max(other.right());
        let y2 = self.bottom().max(other.bottom());

        let width = x2 - x1;
        let height = y2 - y1;

        Self::new(GenericPoint::new(x1, y1), GenericSize::new(width, height))
    }
}

pub trait MinMax {
    fn min(self, other: Self) -> Self;
    fn max(self, other: Self) -> Self;
}

impl MinMax for i32 {
    fn min(self, other: Self) -> Self {
        std::cmp::min(self, other)
    }
    fn max(self, other: Self) -> Self {
        std::cmp::max(self, other)
    }
}

impl MinMax for f32 {
    fn min(self, other: Self) -> Self {
        self.min(other)
    }
    fn max(self, other: Self) -> Self {
        self.max(other)
    }
}

impl<T> fmt::Debug for GenericRectangle<T>
where
    T: Copy + PartialOrd + Zero + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rectangle")
            .field("x", &self.left())
            .field("y", &self.top())
            .field("w", &self.size.width())
            .field("h", &self.size.height())
            .finish()
    }
}
