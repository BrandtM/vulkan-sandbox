#version 460

layout(location = 0) in vec2 position;
layout(set = 0, binding = 0) uniform Data {
    mat4 rotation;
} uni;

void main() {
    mat4 size;
    size[0] = vec4(0.5, 0.0, 0.0, 0.0);
    size[1] = vec4(0.0, 0.5, 0.0, 0.0);
    size[2] = vec4(0.0, 0.0, 1.0, 0.0);
    size[3] = vec4(0.0, 0.0, 0.0, 1.0);
    gl_Position = size * vec4(position, 0.0, 1.0) * uni.rotation;
}