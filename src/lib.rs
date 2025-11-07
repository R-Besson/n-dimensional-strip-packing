/*! Rust Crate for 3-dimensional packing of boxes optimally along x, y or z, or all three axis.
## Example
```
let my_boxes = vec![
    Box3D::from_xyz_whl(0, 0, 0, 100, 200, 300, 1, 0),
    Box3D::from_xyz_whl(0, 0, 0, 100, 200, 300, 2, 0),
    Box3D::from_xyz_whl(0, 0, 0, 100, 200, 300, 3, 0),
];

let mut my_instance = PackerInstance::new(
    my_boxes.clone(),            // Our boxes
    Vector3D::new(500, 0, 500),  // Our container size
    true,                        // No rotations
    (false, true, false),        // Minimize height only
    Sorting::descending_volume,  // Our initial sorting heuristic
);

// for _ in 0..3 {
//     match my_instance.pack_next() {
//         Err(error) => println!("Error: {}", error),
//         Ok(()) => {}
//     }
// }

match my_instance.pack_all() {
    Err(errors) => println!("Errors: {:#?}", errors),
    Ok(()) => {}
}

println!("{:#?}", my_instance.boxes());
```*/

pub mod box3d;
pub mod vector3d;
pub mod sorting;

use box3d::Box3D;
use vector3d::Vector3D;
use std::cmp::Ordering;

use std::collections::HashSet;
use std::hash::BuildHasherDefault;
use fnv::FnvHasher;

/// Hash Set using [fnv](https://crates.io/crates/fnv): an implementation of the [Fowler–Noll–Vo hash function](http://www.isthe.com/chongo/tech/comp/fnv/index.html).
/// ## Example
/// ```rust
/// let mut boxes = HashSetFnv::<Box3D>::default();
/// boxes.insert(Box3D::from_xyz_whl(0, 0, 0, 100, 200, 300, 1, 0));
/// ```
pub type HashSetFnv<V> = HashSet<V, BuildHasherDefault<FnvHasher>>;

/// # Minimize tuple
/// Dictates which axis (x,y,z) the packer tries to minimize
/// ## Example
/// ```rust
/// // x,    y,     z
/// (true, false, true)
/// ```
/// Will minimize the width (x) of the container,\
/// and the length (z) of the container\
/// However, the height (y) of the container will be specified via container_size in [PackerInstance::new()](PackerInstance::new)
pub type Minimize = (bool, bool, bool);

/**
## Example
```
let mut my_instance = PackerInstance::new(
    my_boxes.clone(), // Our boxes
    Vector3D::new(500, 0, 500), // Our container size
    true, // Allow rotations
    (false, true, false), // Minimize height only
    Sorting::descending_volume // Our initial sorting heuristic
);
```*/
#[derive(Default, Debug, Clone)]
pub struct PackerInstance {
	container_size: Vector3D::<u64>,
	do_rotations: bool,
	minimize: Minimize,

	boxes: Vec<Box3D>,
	holes: HashSetFnv<Box3D>,

	next_box_id: usize,
	next_hole_id: usize,
}

impl PackerInstance {
	pub fn container_size(&self) -> Vector3D<u64> {
		self.container_size
	}

	pub fn boxes(&self) -> &Vec<Box3D> {
		&self.boxes
	}

	pub fn holes(&self) -> &HashSetFnv<Box3D> {
		&self.holes
	}

	pub fn finished(&self) -> bool {
		self.next_box_id >= self.boxes.len()
	}

	pub fn next_box_id(&self) -> usize {
		self.next_box_id
	}

	pub fn next_hole_id(&self) -> usize {
		self.next_hole_id
	}

	/// Setup the packer
	pub fn new(mut boxes: Vec<Box3D>, container_size: Vector3D<u64>, do_rotations: bool, minimize: Minimize, sorting_func: fn(&Box3D,&Box3D) -> Ordering)
	-> PackerInstance
	{
		boxes.sort_by(sorting_func);

		let mut holes = HashSetFnv::<Box3D>::default();
		holes.insert(Box3D::from_xyz_whl(
			0, 0, 0,
			if minimize.0 {u64::MAX} else {container_size.x},
			if minimize.1 {u64::MAX} else {container_size.y},
			if minimize.2 {u64::MAX} else {container_size.z},
			0,
			0
		));

		PackerInstance {
			container_size,
			do_rotations,
			minimize,
			boxes,
			holes,
			next_box_id: 0,
			next_hole_id: 1
		}
	}

	/// Do next packing iteration
	pub fn pack_next(&mut self) -> Result<(), String>
	{
		if self.next_box_id >= self.boxes.len() {
			return Err("No more boxes to pack".into());
		}

		let b = &mut self.boxes[self.next_box_id];
		if b.size.x == 0 || b.size.y == 0 || b.size.z == 0
		{
			self.next_box_id += 1;
			return Err(format!("Box {} has invalid size", b.id));
		}

		let (best_box, best_hole) = get_best_hole(b, &self.holes, self.do_rotations, self.minimize);
		if best_hole.is_none()
		{
			self.next_box_id += 1;
			return Err(format!("Could not find hole for Box {}", b.id));
		}
		let best_hole = best_hole.unwrap();

		// Place and rotate (switch sizes) box
		b.size = best_box.size;
		b.position = best_hole.position;

		// Update container size
		if self.minimize.0 {
			self.container_size.x = self.container_size.x.max(get_new_w(&best_hole, b));
		};
		if self.minimize.1 {
			self.container_size.y = self.container_size.y.max(get_new_h(&best_hole, b));
		};
		if self.minimize.2 {
			self.container_size.z = self.container_size.z.max(get_new_l(&best_hole, b));
		};

		// Update the holes
		self.next_hole_id = update_holes(b, &mut self.holes, self.next_hole_id);

		// Change for next box id
		self.next_box_id += 1;

		Ok(())
	}

	pub fn pack_all(&mut self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();

		while !self.finished() {
			if let Err(e) = self.pack_next() {
				errors.push(e);
			}
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
    }
}

fn cut(b: &Box3D, hole: &Box3D, holes: &mut HashSetFnv<Box3D>, next_hole_id: &mut usize)
{
	let mut new_holes = Vec::<Box3D>::default();

	let x2 = b.x2();
	let y2 = b.y2();
	let z2 = b.z2();

	let hx2 = hole.x2();
	let hy2 = hole.y2();
	let hz2 = hole.z2();

	if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{ }
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 1));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 2));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 3));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 4));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 5));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 6));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 7));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 8));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 9));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 10));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 11));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 12));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 13));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 14));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 15));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 16));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 17));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 18));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 19));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 20));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 21));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 22));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 23));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 24));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 25));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 26));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 27));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 28));
	}
	else if
		(b.position.x <= hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 29));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 30));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 31));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 32));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 33));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 34));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 35));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 36));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 37));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 38));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 39));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 40));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 41));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 42));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 43));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,y2-hole.position.y,hz2-z2, *next_hole_id, 44));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 45));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,z2-hole.position.z, *next_hole_id, 46));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 47));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 48));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 49));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 50));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 51));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 52));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 53));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 54));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 55));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,y2-hole.position.y,hz2-z2, *next_hole_id, 56));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 57));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,z2-hole.position.z, *next_hole_id, 58));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 59));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 60));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 61));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 62));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 63));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,z2-hole.position.z, *next_hole_id, 64));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 65));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 66));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 67));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 68));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 69));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 70));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 71));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 72));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,z2-hole.position.z, *next_hole_id, 73));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 74));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 75));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 76));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 77));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 78));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 79));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,z2-hole.position.z, *next_hole_id, 80));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 81));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,y2-hole.position.y,hz2-z2, *next_hole_id, 82));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 83));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,z2-hole.position.z, *next_hole_id, 84));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 85));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 86));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 87));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 88));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 89));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 90));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 91));
	}
	else if
		(b.position.x <= hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 92));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 93));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,z2-hole.position.z, *next_hole_id, 94));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 95));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 96));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,y2-hole.position.y,hz2-z2, *next_hole_id, 97));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 98));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,z2-hole.position.z, *next_hole_id, 99));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 100));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 101));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 102));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 103));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 104));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 105));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 106));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 107));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 108));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 109));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 110));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 111));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 112));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 113));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 114));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 115));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 116));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 117));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 118));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 119));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 120));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 121));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 122));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 123));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 124));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 125));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 126));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 127));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 128));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 129));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 130));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 131));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 132));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 133));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 134));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 135));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 136));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 137));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 138));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 139));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 140));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 141));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 142));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 143));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 144));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 145));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 146));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 147));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 148));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 149));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 150));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 151));
	}
	else if
		(b.position.x > hole.position.x && x2 >= hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 152));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 153));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 154));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 155));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 156));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 157));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 158));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 159));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 160));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 161));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 162));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 163));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 164));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 165));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 166));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 167));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 168));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 169));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 170));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 171));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 172));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 173));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 174));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 175));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 176));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,y2-hole.position.y,hz2-z2, *next_hole_id, 177));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 178));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,z2-hole.position.z, *next_hole_id, 179));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 180));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 181));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 182));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 183));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 184));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 185));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 186));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y <= hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 187));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 188));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 189));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 190));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 191));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,y2-hole.position.y,hz2-z2, *next_hole_id, 192));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 193));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,z2-hole.position.z, *next_hole_id, 194));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 195));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 196));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 197));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 198));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 199));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 200));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 201));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 202));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,z2-hole.position.z, *next_hole_id, 203));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 204));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 205));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 206));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 207));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 208));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 209));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 210));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 >= hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 211));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 212));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 213));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 214));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,z2-hole.position.z, *next_hole_id, 215));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 216));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 217));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 218));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 219));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 220));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 221));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z <= hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 222));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 223));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 224));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,z2-hole.position.z, *next_hole_id, 225));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 226));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,y2-hole.position.y,hz2-z2, *next_hole_id, 227));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 228));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,z2-hole.position.z, *next_hole_id, 229));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 230));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 231));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 >= hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 232));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 233));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 234));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 235));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 236));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 237));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 238));
	}
	else if
		(b.position.x > hole.position.x && x2 < hx2) &&
		(b.position.y > hole.position.y && y2 < hy2) &&
		(b.position.z > hole.position.z && z2 < hz2)
	{
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 239));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,b.position.x-hole.position.x,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 240));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,hy2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 241));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,hz2-hole.position.z, *next_hole_id, 242));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,y2-hole.position.y,b.position.z-hole.position.z, *next_hole_id, 243));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,hole.position.z,hx2-hole.position.x,b.position.y-hole.position.y,z2-hole.position.z, *next_hole_id, 244));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,hy2-hole.position.y,hz2-z2, *next_hole_id, 245));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,hole.position.y,z2,hx2-hole.position.x,y2-hole.position.y,hz2-z2, *next_hole_id, 246));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,hz2-hole.position.z, *next_hole_id, 247));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(hole.position.x,y2,hole.position.z,hx2-hole.position.x,hy2-y2,z2-hole.position.z, *next_hole_id, 248));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,hz2-hole.position.z, *next_hole_id, 249));
		*next_hole_id += 1; new_holes.push(Box3D::from_xyz_whl(x2,hole.position.y,hole.position.z,hx2-x2,hy2-hole.position.y,z2-hole.position.z, *next_hole_id, 250));
	}

	for new_hole in new_holes {
		holes.insert(new_hole);
	}
}

fn update_holes(b: &mut Box3D, holes: &mut HashSetFnv<Box3D>, mut next_hole_id: usize)
-> usize
{
	// Cut holes
	let mut new_holes = HashSetFnv::<Box3D>::default();

	for hole in holes.iter() {
		// If the Current Box3D overlaps with a hole, we break the hole into new holes
		if b.intersects(hole) {
			cut(b, hole, &mut new_holes, &mut next_hole_id);
		} else if !hole.is_covered_among(&new_holes) {
			new_holes.insert(*hole);
		}
	}

	*holes = new_holes;

	next_hole_id
}

fn get_new_w(hole: &Box3D, b: &Box3D) -> u64 {
	hole.position.x + b.size.x
}
fn get_new_h(hole: &Box3D, b: &Box3D) -> u64 {
	hole.position.y + b.size.y
}
fn get_new_l(hole: &Box3D, b: &Box3D) -> u64 {
	hole.position.z + b.size.z
}

fn is_better_hole(b: &Box3D, hole: &Box3D, best_hole: Option<Box3D>, min_w: u64, min_h: u64, min_l: u64, minimize: Minimize) -> bool
{
	if !b.fits_in(hole) { return false; }

	let Some(best_hole) = best_hole else { return true; };

	let hole_perfect = b.size.x == hole.size.x && b.size.y == hole.size.y && b.size.z == hole.size.z;
	let best_hole_perfect = b.size.x == best_hole.size.x && b.size.y == best_hole.size.y && b.size.z == best_hole.size.z;

	// Best hole is a perfect fit ...
	if best_hole_perfect && !hole_perfect { return false; }

	// Hole is a perfect fit !
	if hole_perfect && !best_hole_perfect { return true; }

	let current_w = if minimize.0 { get_new_w(hole, b) } else { 0 };
	let current_h = if minimize.1 { get_new_h(hole, b) } else { 0 };
	let current_l = if minimize.2 { get_new_l(hole, b) } else { 0 };

	match minimize {
		// Minimize Width
		(true, false, false) => {
			(current_w, hole.position.x, hole.position.y, hole.position.z, hole.size.x, hole.size.y, hole.size.z, hole.id) <
			(min_w, best_hole.position.x, best_hole.position.y, best_hole.position.z, best_hole.size.x, best_hole.size.y, best_hole.size.z, best_hole.id)
		}
		// Minimize Height
		(false, true, false) => {
			(current_h, hole.position.y, hole.position.x, hole.position.z, hole.size.y, hole.size.x, hole.size.z, hole.id) <
			(min_h, best_hole.position.y, best_hole.position.x, best_hole.position.z, best_hole.size.y, best_hole.size.x, best_hole.size.z, best_hole.id)
		}
		// Minimize Length
		(false, false, true) => {
			(current_l, hole.position.z, hole.position.x, hole.position.y, hole.size.z, hole.size.x, hole.size.y, hole.id) <
			(min_l, best_hole.position.z, best_hole.position.x, best_hole.position.y, best_hole.size.z, best_hole.size.x, best_hole.size.y, best_hole.id)
		}
		// Minimize Width & Height
		(true, true, false) => {
			(current_w.max(current_h), hole.position.x.max(hole.position.y), hole.position.z, hole.size.x.max(hole.size.y), hole.size.z, hole.id) <
			(min_w.max(min_h), best_hole.position.x.max(best_hole.position.y), best_hole.position.z, best_hole.size.x.max(best_hole.size.y), best_hole.size.z, best_hole.id)
		}
		// Minimize Width & Length
		(true, false, true) => {
			(current_w.max(current_l), hole.position.x.max(hole.position.z), hole.position.y, hole.size.x.max(hole.size.z), hole.size.y, hole.id) <
			(min_w.max(min_l), best_hole.position.x.max(best_hole.position.z), best_hole.position.y, best_hole.size.x.max(best_hole.size.z), best_hole.size.y, best_hole.id)
		}
		// Minimize Height & Length
		(false, true, true) => {
			(current_h.max(current_l), hole.position.y.max(hole.position.z), hole.position.x, hole.size.y.max(hole.size.z), hole.size.x, hole.id) <
			(min_h.max(min_l), best_hole.position.y.max(best_hole.position.z), best_hole.position.x, best_hole.size.y.max(best_hole.size.z), best_hole.size.x, best_hole.id)
		}
		// Minimize all three
		(true, true, true) => {
			(current_w.max(current_h).max(current_l), hole.position.x.max(hole.position.y).max(hole.position.z), hole.size.x.max(hole.size.y).max(hole.size.z), hole.id) <
			(min_w.max(min_h).max(min_l), best_hole.position.x.max(best_hole.position.y).max(best_hole.position.z), best_hole.size.x.max(best_hole.size.y).max(best_hole.size.z), best_hole.id)
		}
		// No minimization, use a default Bottom-Left-Front heuristic
		(false, false, false) => {
			(hole.position.y, hole.position.x, hole.position.z, hole.id) <
			(best_hole.position.y, best_hole.position.x, best_hole.position.z, best_hole.id)
		}
	}
}

fn get_best_hole(b: &Box3D, holes: &HashSetFnv<Box3D>, do_rotations: bool, minimize: Minimize) -> (Box3D, Option<Box3D>)
{
	let mut new_box = *b;
	let mut best_hole: Option<Box3D> = None;

	let mut min_w = u64::MAX;
	let mut min_h = u64::MAX;
	let mut min_l = u64::MAX;

	let rotations_to_check = if do_rotations { b.get_rotations() } else { vec![*b] };

	for hole in holes.iter() {
		for rotation in &rotations_to_check {
			if !rotation.fits_in(hole) {
				continue;
			}

			if is_better_hole(rotation, hole, best_hole, min_w, min_h, min_l, minimize) {
				new_box = *rotation;
				min_w = get_new_w(hole, rotation);
				min_h = get_new_h(hole, rotation);
				min_l = get_new_l(hole, rotation);
				best_hole = Some(*hole);
			}
		}
	}

	(new_box, best_hole)
}