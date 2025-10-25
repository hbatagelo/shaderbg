use super::super::*;

pub type PointF = GenericPoint<f32>;
pub type SizeI = GenericSize<i32>;
pub type SizeF = GenericSize<f32>;
pub type RectangleF = GenericRectangle<f32>;

#[test]
fn test_rectangle_creation() {
    let rect = Rectangle::new(Point::new(5, 10), SizeI::new(15, 20));
    assert_eq!(rect.left(), 5);
    assert_eq!(rect.top(), 10);
    assert_eq!(rect.right(), 20);
    assert_eq!(rect.bottom(), 30);
}

#[test]
fn test_rectangle_getters() {
    let rect = Rectangle::new(Point::new(10, 20), SizeI::new(30, 40));

    assert_eq!(rect.top_left(), Point::new(10, 20));
    assert_eq!(rect.size(), SizeI::new(30, 40));
    assert_eq!(rect.left(), 10);
    assert_eq!(rect.top(), 20);
    assert_eq!(rect.right(), 40);
    assert_eq!(rect.bottom(), 60);
    assert_eq!(rect.width(), 30);
    assert_eq!(rect.height(), 40);
}

#[test]
fn test_float_rectangle_getters() {
    let rect = RectangleF::new(PointF::new(5.5, 10.0), SizeF::new(20.0, 15.0));

    assert_eq!(rect.top_left(), PointF::new(5.5, 10.0));
    assert_eq!(rect.size(), SizeF::new(20.0, 15.0));
    assert_eq!(rect.left(), 5.5);
    assert_eq!(rect.top(), 10.0);
    assert_eq!(rect.right(), 25.5);
    assert_eq!(rect.bottom(), 25.0);
    assert_eq!(rect.width(), 20.0);
    assert_eq!(rect.height(), 15.0);
}

#[test]
fn test_offset_arithmetic() {
    let rect = Rectangle::new(Point::new(10, 20), SizeI::new(30, 40));
    let offset = Offset::new(5, -5);

    assert_eq!(
        rect + offset,
        Rectangle::new(Point::new(15, 15), SizeI::new(30, 40))
    );
    assert_eq!(
        rect - offset,
        Rectangle::new(Point::new(5, 25), SizeI::new(30, 40))
    );
}

#[test]
fn test_union() {
    let rect1 = Rectangle::new(Point::new(10, 10), SizeI::new(20, 30));
    let rect2 = Rectangle::new(Point::new(15, 25), SizeI::new(40, 20));
    let union = rect1.union(&rect2);

    assert_eq!(union.left(), 10);
    assert_eq!(union.top(), 10);
    assert_eq!(union.right(), 55);
    assert_eq!(union.bottom(), 45);
}

#[test]
fn test_float_union() {
    let rect1 = RectangleF::new(PointF::new(5.5, 10.0), SizeF::new(20.0, 15.0));
    let rect2 = RectangleF::new(PointF::new(10.0, 12.5), SizeF::new(25.0, 20.0));
    let union = rect1.union(&rect2);

    assert_eq!(union.left(), 5.5);
    assert_eq!(union.top(), 10.0);
    assert_eq!(union.right(), 35.0);
    assert_eq!(union.bottom(), 32.5);
}

#[test]
fn test_gdk_conversion() {
    let gdk_rect = gtk::gdk::Rectangle::new(10, 20, 30, 40);
    let rect: Rectangle = gdk_rect.into();

    assert_eq!(rect.left(), 10);
    assert_eq!(rect.top(), 20);
    assert_eq!(rect.width(), 30);
    assert_eq!(rect.height(), 40);
}
