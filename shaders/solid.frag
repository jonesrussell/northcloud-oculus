#version 450

// Renders a distinct solid color per eye:
//   Left eye (view 0) = dark blue
//   Right eye (view 1) = dark red

layout(location = 0) in flat uint in_view_index;
layout(location = 0) out vec4 out_color;

void main() {
    if (in_view_index == 0u) {
        out_color = vec4(0.05, 0.05, 0.3, 1.0); // Left eye: dark blue
    } else {
        out_color = vec4(0.3, 0.05, 0.05, 1.0); // Right eye: dark red
    }
}
