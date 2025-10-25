use super::super::*;

pub type SizeI = GenericSize<i32>;
pub type SizeF = GenericSize<f32>;

#[test]
fn test_size_creation() {
    let size = Size::new(100, 200);
    assert_eq!(size.width(), 100);
    assert_eq!(size.height(), 200);
}

#[test]
fn test_clamping() {
    let mut size = SizeI::new(-10, 5);
    assert_eq!(size.width(), 0); // Clamped to 0
    assert_eq!(size.height(), 5);

    size.set_width(-5);
    assert_eq!(size.width(), 0);

    size.set_height(-5);
    assert_eq!(size.height(), 0);
}

#[test]
fn test_setters() {
    let mut size = Size::new(0, 0);

    size.set_width(12);
    assert_eq!(size.width(), 12);

    size.set_width(21);
    assert_eq!(size.width(), 21);
}

#[test]
fn test_default() {
    let size: Size = Default::default();
    assert_eq!(size, Size::new(0, 0));
}

#[test]
fn test_scalar_multiplication() {
    let size = Size::new(2, 4);
    let result = size * 1.8;
    assert_eq!(result, Size::new(4, 7)); // Rounded

    let float_size = SizeF::new(1.5, 2.5);
    let float_result = float_size * 2.5;
    assert_eq!(float_result, SizeF::new(3.75, 6.25));
}
