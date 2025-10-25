use super::super::*;

pub type PointF = GenericPoint<f32>;

#[test]
fn test_point_creation() {
    let point = Point::new(15, 25);
    assert_eq!(point.x(), 15);
    assert_eq!(point.y(), 25);
}

#[test]
fn test_offset_arithmetic() {
    let point = Point::new(10, 20);
    let offset = Offset::new(5, -5);

    assert_eq!(point + offset, Point::new(15, 15));
    assert_eq!(point - offset, Point::new(5, 25));
}

#[test]
fn test_scalar_multiplication() {
    let point = Point::new(2, 4);
    let result = point * 1.8;
    assert_eq!(result, Point::new(4, 7)); // Rounded

    let float_point = PointF::new(1.5, 2.5);
    let float_result = float_point * 2.5;
    assert_eq!(float_result, PointF::new(3.75, 6.25));
}
