struct Header {
    char magic[4];
    u32 mesh_count;
};

struct Vertex {
    float u,v,x,y,z;
};

struct Mesh {
    u32 name_length;
    char name[name_length];
    u32 vertex_count;
    u32 index_count;
    u8 primitive_type;
    Vertex vertex_data[vertex_count];
    u16 index_data[index_count];
};

struct MeshFile {
    Header header;
    Mesh meshes[header.mesh_count];
};

MeshFile file @ 0x00;

struct Position {
    padding[8];
    float x,y,z;
};

fn to_u32(auto v) {
    return u32(v.data);  
};

struct Index32 {
    u16 data;
} [[transform("to_u32")]];

u32 indices[file.meshes[0].index_count];

for (auto i = 0, i < file.meshes[0].index_count, i += 1) {
    indices[i] = u32(file.meshes[0].index_data[i]);
}

struct VisualizableMesh {
    Position positions[file.meshes[0].vertex_count];
    Index32 indices[file.meshes[0].index_count];
}[[hex::visualize("3d", positions, indices)]];

std::print("Indices: {}", file.meshes[0].index_count);

VisualizableMesh vm @ addressof(file.meshes[0].vertex_data[0]);