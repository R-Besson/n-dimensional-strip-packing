//! Sorting functions used by [PackerInstance](crate::PackerInstance)

use crate::box3d::Box3D;
use std::cmp::{Ordering, Reverse};

pub struct Sorting {}

impl Sorting {
	pub fn descending_volume(a: &Box3D, b: &Box3D) -> Ordering {
		(b.volume(), b.size.z, b.size.x, b.size.y, Reverse(b.id))
			.cmp(&(a.volume(), a.size.z, a.size.x, a.size.y, Reverse(a.id)))
	}
	pub fn ascending_volume(a: &Box3D, b: &Box3D) -> Ordering {
		Self::descending_volume(b, a)
	}

	pub fn descending_width(a: &Box3D, b: &Box3D) -> Ordering {
		(b.size.x, b.volume(), b.size.z, b.size.y, Reverse(b.id))
			.cmp(&(a.size.x, a.volume(), a.size.z, a.size.y, Reverse(a.id)))
	}
	pub fn ascending_width(a: &Box3D, b: &Box3D) -> Ordering {
		Self::descending_width(b, a)
	}

	pub fn descending_height(a: &Box3D, b: &Box3D) -> Ordering {
		(b.size.y, b.volume(), b.size.z, b.size.x, Reverse(b.id))
			.cmp(&(a.size.y, a.volume(), a.size.z, a.size.x, Reverse(a.id)))
	}
	pub fn ascending_height(a: &Box3D, b: &Box3D) -> Ordering {
		Self::descending_height(b, a)
	}

	pub fn descending_length(a: &Box3D, b: &Box3D) -> Ordering {
		(b.size.z, b.volume(), b.size.x, b.size.y, Reverse(b.id))
			.cmp(&(a.size.z, a.volume(), a.size.x, a.size.y, Reverse(a.id)))
	}
	pub fn ascending_length(a: &Box3D, b: &Box3D) -> Ordering {
		Self::descending_length(b, a)
	}

	pub fn descending_id(a: &Box3D, b: &Box3D) -> Ordering {
		b.id.cmp(&a.id)
	}
	pub fn ascending_id(a: &Box3D, b: &Box3D) -> Ordering {
		Self::descending_id(b, a)
	}

	/// Gets the sorting function from a string slice
	pub fn get(name: &str) -> fn(&Box3D,&Box3D) -> Ordering
	{
		match name
		{
			"Descending Volume" => Sorting::descending_volume,
			"Ascending Volume" => Sorting::ascending_volume,

			"Descending Width" => Sorting::descending_width,
			"Ascending Width" => Sorting::ascending_width,

			"Descending Height" => Sorting::descending_height,
			"Ascending Height" => Sorting::ascending_height,

			"Descending Length" => Sorting::descending_length,
			"Ascending Length" => Sorting::ascending_length,

			"Descending Id" => Sorting::descending_id,
			"Ascending Id" => Sorting::ascending_id,

			_ => Sorting::descending_volume,
		}
	}
}