//! 3-dimensional box
use crate::vector3d::*;
use crate::HashSetFnv;

/// The structure of a 3-dimensional box
#[derive(Eq, PartialEq, Default, Hash, Copy, Clone, Debug)]
pub struct Box3D {
	/// Corresponds to `x`,`y`,`z` properties
	pub position: Vector3D::<u64>,
	/// Corresponds to `w`,`h`,`l` properties
	pub size: Vector3D::<u64>,
	/// ID for each box
	pub id: usize,
	/// Keeps a trace of how the box evolved (only for debugging)
	pub origin: u16,
}

impl Box3D {

	/// Create Box3D from `position`, and `size`
	pub fn from_position_size(position: Vector3D::<u64>, size: Vector3D::<u64>, id: usize, origin: u16) -> Box3D
	{
		Box3D { position, size, id, origin }
	}

	/// Create Box3D from `xyz`, and `whl`
	pub fn from_xyz_whl(x: u64, y: u64, z: u64, w: u64, h: u64, l: u64, id: usize, origin: u16) -> Box3D
	{
		let position = Vector3D::<u64>::new(x,y,z);
		let size = Vector3D::<u64>::new(w,h,l);
		Box3D { position, size, id, origin }
	}

	/// Volume of the Box
	pub fn volume(&self) -> u64
	{
		self.size.x * self.size.y * self.size.z
	}

	/// Box is smaller than other
	pub fn fits_in(&self, other: &Box3D) -> bool
	{
		self.size.x <= other.size.x && self.size.y <= other.size.y && self.size.z <= other.size.z
	}

	/// Box is inside other in terms of size AND position
	pub fn is_in(&self, other: &Box3D) -> bool 
	{   // Check if shape IS in 'other' shape using
		// a comparison of positions
		self.position.x >= other.position.x &&       // BOTTOM corner of shape is inside 'other'
		self.position.y >= other.position.y &&       //
		self.position.z >= other.position.z &&       //

		self.x2() <= other.x2() &&     // TOP corner of shape is inside 'other'
		self.y2() <= other.y2() &&     //
		self.z2() <= other.z2()        //
	}

	pub fn x2(&self) -> u64
	{
		self.position.x + self.size.x
	}
	pub fn y2(&self) -> u64
	{
		self.position.y + self.size.y
	}
	pub fn z2(&self) -> u64 
	{
		self.position.z + self.size.z
	}

	/// Box strictly (doesn't only touch) intersects other
	pub fn intersects(&self, other: &Box3D) -> bool
	{
		range_overlap(self.position.x, self.x2(), other.position.x, other.x2()) &&
		range_overlap(self.position.y, self.y2(), other.position.y, other.y2()) &&
		range_overlap(self.position.z, self.z2(), other.position.z, other.z2())
	}

	/// Returns whether Box is covered among other Boxes
	pub fn is_covered_among(&self, boxes: &HashSetFnv<Box3D>) -> bool
	{
		boxes.iter().any(|other| self.is_in(other))
	}

	/// Gets the new 6 boxes with sizes adjusted according to rotation
	pub fn get_rotations(&self) -> Vec<Box3D>
	{
		vec![
			Box3D::from_position_size(self.position, Vector3D::<u64>::new(self.size.x, self.size.y, self.size.z), self.id, self.origin), // w,h,l
			Box3D::from_position_size(self.position, Vector3D::<u64>::new(self.size.y, self.size.x, self.size.z), self.id, self.origin), // h,w,l
			Box3D::from_position_size(self.position, Vector3D::<u64>::new(self.size.z, self.size.y, self.size.x), self.id, self.origin), // l,h,w
			Box3D::from_position_size(self.position, Vector3D::<u64>::new(self.size.x, self.size.z, self.size.y), self.id, self.origin), // w,l,h
			Box3D::from_position_size(self.position, Vector3D::<u64>::new(self.size.z, self.size.x, self.size.y), self.id, self.origin), // l,w,h
			Box3D::from_position_size(self.position, Vector3D::<u64>::new(self.size.y, self.size.z, self.size.x), self.id, self.origin)  // h,l,w
		]
	}
}

/// Checks if two 1D ranges `[amin, amax)` and `[bmin, bmax)` strictly overlap.
fn range_overlap<T: std::cmp::PartialOrd>(amin: T, amax: T, bmin: T, bmax: T) -> bool
{
	amax > bmin && bmax > amin
}