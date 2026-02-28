#version 450
#extension GL_EXT_multiview : require

// Fullscreen triangle from vertex ID — no vertex buffer needed.
// Vertex 0: (-1, -1), Vertex 1: (3, -1), Vertex 2: (-1, 3)
// This covers the entire clip space with a single oversized triangle.

layout(location = 0) out flat uint out_view_index;

void main() {
    vec2 pos = vec2(
        float((gl_VertexIndex << 1) & 2) * 2.0 - 1.0,
        float(gl_VertexIndex & 2) * 2.0 - 1.0
    );
    gl_Position = vec4(pos, 0.0, 1.0);
    out_view_index = gl_ViewIndex;
}
