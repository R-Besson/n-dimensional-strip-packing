//! 3-dimensional vector

#[derive(Eq, PartialEq, Default, Hash, Copy, Clone, Debug)]
pub struct Vector3D<T> {
	pub x: T,
	pub y: T,
	pub z: T,
}

impl<T> Vector3D<T> {
	pub fn new(x: T, y: T, z: T) -> Vector3D<T> {
		Vector3D::<T> {x, y, z}
	}
}