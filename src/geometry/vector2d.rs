// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

pub trait Vector2D<T> {
    fn x(&self) -> T;
    fn y(&self) -> T;

    fn from_components(x: T, y: T) -> Self;
}

#[macro_export]
macro_rules! impl_scalar_mul {
    ($($vector_type:ty, $scalar_type:ty, $round_flag:tt)+) => {
        $(
            impl Mul<$scalar_type> for $vector_type {
                type Output = $vector_type;
                fn mul(self, rhs: $scalar_type) -> Self::Output {
                    <$vector_type as Vector2D<_>>::from_components(
                        impl_scalar_mul!(@compute $round_flag, self.x(), $scalar_type, rhs),
                        impl_scalar_mul!(@compute $round_flag, self.y(), $scalar_type, rhs),
                    )
                }
            }
        )+
    };

    (@compute true, $field:expr, $scalar:ty, $rhs:expr) => {
        (($field as $scalar * $rhs).round() as _)
    };

    (@compute false, $field:expr, $scalar:ty, $rhs:expr) => {
        (($field as $scalar * $rhs) as _)
    };
}
