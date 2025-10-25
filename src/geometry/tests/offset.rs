use super::super::*;

pub type OffsetF = GenericOffset<f32>;

#[test]
fn test_offset_creation() {
    let offset = Offset::new(10, 20);
    assert_eq!(offset.dx(), 10);
    assert_eq!(offset.dy(), 20);
}

#[test]
fn test_negation() {
    let offset = Offset::new(5, -3);
    assert_eq!(-offset, Offset::new(-5, 3));
}

#[test]
fn test_scalar_multiplication() {
    let offset = Offset::new(2, 4);
    let result = offset * 1.8;
    assert_eq!(result, Offset::new(4, 7)); // Rounded

    let float_offset = OffsetF::new(1.5, 2.5);
    let float_result = float_offset * 2.5;
    assert_eq!(float_result, OffsetF::new(3.75, 6.25));
}

#[test]
fn test_from_point() {
    let point = Point::new(7, 8);
    let offset: Offset = point.into();
    assert_eq!(offset, Offset::new(7, 8));
}
