#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_color;

layout(push_constant) uniform PushConstants {
    mat4 view_proj[2];
} pc;

layout(location = 0) out vec3 frag_color;

void main() {
    gl_Position = pc.view_proj[gl_ViewIndex] * vec4(in_position, 1.0);
    frag_color = in_color;
}
