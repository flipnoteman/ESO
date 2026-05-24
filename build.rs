// build.rs
use glob::glob;
use gltf;
#[allow(unused_imports)]
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("Could not create destination directory");
    for entry in fs::read_dir(src).expect("Could not read source directory") {
        let entry = entry.expect("Could not read entry");
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).expect("Could not copy file");
        }
    }
}

fn main() {
    let assets_dir = Path::new("./assets/"); //env::var_os("OUT_DIR").unwrap();
    let models_dir = Path::new(&assets_dir).join("models");
    let dest_dir = Path::new(&assets_dir).join("meshes");

    if !dest_dir.exists() {
        fs::create_dir_all(&dest_dir).expect("Could not create save directory");
    }

    if models_dir.is_dir() {
        for entry in fs::read_dir(models_dir).expect("Could not read path") {
            let entry = entry.expect("Could not read entry path");
            println!("cargo::warning={:?}", entry.path());

            for glb_path in glob(entry.path().join("**/*.glb").to_str().unwrap())
                .expect("Failed to read glob pattern")
            {
                let obj = glb_path.expect("Could not load object model path");

                // Obj file to mesh file name mapping
                let stem = obj.file_stem().unwrap().to_str().unwrap();
                let out_path = dest_dir.join(format!("{}.mesh", stem)); // .mesh file out

                let (document, buffers, images) = gltf::import(&obj).expect("Failed to load glb");

                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(&out_path)
                    .expect("Cannot open/create mesh file");

                // Write file headers
                file.write_all(b"MESH")
                    .expect("Could not write magic bytes");
                let mesh_count = document
                    .meshes()
                    .map(|m| m.primitives().count())
                    .sum::<usize>();
                file.write_all(&(mesh_count as u32).to_le_bytes())
                    .expect("Could not write mesh count");

                // if let Ok(materials) = materials {
                //     println!("cargo::warning=Materials: {}", materials.len());
                // } else {
                //     println!("cargo::warning=Materials: 0");
                // }
                //
                println!("cargo::warning=Meshes: {}", mesh_count);

                for mesh in document.meshes() {
                    for primitive in mesh.primitives() {
                        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                        let name = mesh.name().unwrap_or("unnamed");
                        println!("cargo::warning=   mesh: {}", name);

                        let positions: Vec<[f32; 3]> =
                            reader.read_positions().expect("No positions").collect();

                        let tex_coords: Vec<[f32; 2]> = reader
                            .read_tex_coords(0)
                            .expect("No tex coords")
                            .into_f32()
                            .collect();

                        let mut indices: Vec<u16> = reader
                            .read_indices()
                            .expect("No indices")
                            .into_u32()
                            .map(|i| i as u16)
                            .collect();

                        // glTF expects CCW winding, but our code expects CW, so we flip indices
                        for tri in indices.chunks_exact_mut(3) {
                            tri.swap(1, 2);
                        }

                        println!("cargo::warning=       vertices: {}", positions.len());
                        println!("cargo::warning=       texcoords: {}", tex_coords.len());
                        println!("cargo::warning=       indices: {}", indices.len());

                        let vertices: Vec<[f32; 5]> = positions
                            .iter()
                            .zip(tex_coords.iter())
                            .map(|(p, uv)| [uv[0], uv[1], p[0], p[1], p[2]])
                            .collect();

                        let vertex_bytes: &[u8] = bytemuck::cast_slice(&vertices);
                        let index_bytes: &[u8] = bytemuck::cast_slice(&indices);

                        file.write_all(&(name.len() as u32).to_le_bytes())
                            .expect("Could not write name length");
                        file.write_all(name.as_bytes())
                            .expect("Could not write name");
                        file.write_all(&(vertices.len() as u32).to_le_bytes())
                            .expect("Could not write vertex count");
                        file.write_all(&(indices.len() as u32).to_le_bytes())
                            .expect("Could not write index count");
                        file.write_all(&[0u8, 0u8, 0u8, 0u8]) // First byte prim type, 3 pad
                            .expect("Could not write primitive type");
                        file.write_all(vertex_bytes)
                            .expect("Could not write vertex data");
                        file.write_all(index_bytes)
                            .expect("Could not write index data");
                    }
                }
                println!("cargo::warning=Written {}", out_path.to_str().unwrap());
            }
        }
    }

    // Copy assets directory to build directory for ppsspp access
    let out_dir = env::var("OUT_DIR").unwrap();
    let build_assets = Path::new(&out_dir)
        .parent()
        .unwrap() // out
        .parent()
        .unwrap() // <crate>-<hash>
        .parent()
        .unwrap() // build
        .join("assets");

    println!("cargo::warning=Copying Assets to: {:?}", build_assets);

    copy_dir(assets_dir, &build_assets);

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=assets/models");
}
