# packer-3d
> Rust Crate for 3-dimensional packing of boxes optimally along x, y or z, or all three axis.
> Includes an N-dimensional Constructive Solid Geometry cut generator script.

Example of what you can make. Rendered using <u>Bevy data-drive game engine</u>.
<img src="./img/example.png" alt="Example of what you can expect" width="400"/><br>

## Quickstart
Run this in your rust project to add packer-3d to your dependencies.
```
cargo add packer-3d
```

## [Documentation](https://docs.rs/packer-3d/latest/packer_3d/)

## Example
After adding the dependency, you can test it with this example:
```rust
use packer_3d::{box3d::Box3D, sorting::Sorting, vector3d::Vector3D, PackerInstance};

fn main() {
    let mut my_boxes = vec![
        Box3D::from_xyz_whl(0, 0, 0, 100, 200, 300, 1, 0),
        Box3D::from_xyz_whl(0, 0, 0, 100, 200, 300, 2, 0),
        Box3D::from_xyz_whl(0, 0, 0, 100, 200, 300, 3, 0),
    ];

    let mut my_instance = PackerInstance::new(
        &mut my_boxes,               // Our boxes
        Vector3D::new(500, 0, 500),  // Our container size
        true,                        // No rotations
        (false, true, false),        // Minimize height only
        &Sorting::descending_volume, // Our initial sorting heuristic
    );

    for _ in 0..3 {
        match my_instance.pack_next() {
            Err(error) => println!("Error: {}", error),
            Ok(()) => {}
        }
    }

    println!("{:#?}", my_instance.boxes());
}
```
Which should output:
```
[
	Box3D {
		position: Vector3D {
			x: 0,
			y: 0,
			z: 0,
		},
		size: Vector3D {
			x: 300,
			y: 100,
			z: 200,
		},
		id: 1,
		origin: 0,
	},
	Box3D {
		position: Vector3D {
			x: 300,
			y: 0,
			z: 0,
		},
		size: Vector3D {
			x: 200,
			y: 100,
			z: 300,
		},
		id: 2,
		origin: 0,
	},
	Box3D {
		position: Vector3D {
			x: 0,
			y: 0,
			z: 200,
		},
		size: Vector3D {
			x: 300,
			y: 100,
			z: 200,
		},
		id: 3,
		origin: 0,
	},
]
```