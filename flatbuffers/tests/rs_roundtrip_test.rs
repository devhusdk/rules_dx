//! Builds a Shape with a Point from the generated Rust crate and reads it back.

use shapes_rs::dx::api::{root_as_shape, Shape, ShapeArgs};
use shapes_rs::dx::base::Point;

#[test]
fn shape_round_trips_through_the_generated_crate() {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("circle");
    let origin = Point::new(1.5, -2.0);
    let shape = Shape::create(
        &mut builder,
        &ShapeArgs {
            name: Some(name),
            origin: Some(&origin),
        },
    );
    builder.finish(shape, None);

    let parsed = root_as_shape(builder.finished_data()).expect("verifier accepts the buffer");
    assert_eq!(parsed.name(), Some("circle"));
    let point = parsed.origin().expect("origin");
    assert_eq!(point.x(), 1.5);
    assert_eq!(point.y(), -2.0);
}
