@group(0) @binding(0) var<storage, read> source: array<u32>;
@group(0) @binding(1) var<storage, read_write> destination: array<u32>;
@group(0) @binding(2) var<uniform> size: vec4<u32>;
fn weight(v: f32) -> f32 {
    let x = abs(v);
    if x <= 1.0 { return 1.5*x*x*x - 2.5*x*x + 1.0; }
    if x < 2.0 { return -0.5*x*x*x + 2.5*x*x - 4.0*x + 2.0; }
    return 0.0;
}
@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= size.x*2u || id.y >= size.y*2u { return; }
    let position = (vec2<f32>(id.xy)+vec2<f32>(0.5))/2.0-vec2<f32>(0.5);
    let base = vec2<i32>(floor(position));
    var color = vec3<f32>(0.0);
    for (var y = -1; y <= 2; y++) {
        for (var x = -1; x <= 2; x++) {
            let sample_position = clamp(base+vec2<i32>(x,y),vec2<i32>(0),vec2<i32>(size.xy)-vec2<i32>(1));
            let pixel_value = source[u32(sample_position.y)*size.x+u32(sample_position.x)];
            let rgb = vec3<f32>(f32(pixel_value & 255u),f32((pixel_value>>8u)&255u),f32((pixel_value>>16u)&255u));
            color += rgb*weight(position.x-f32(base.x+x))*weight(position.y-f32(base.y+y));
        }
    }
    let rgb = vec3<u32>(round(clamp(color,vec3<f32>(0.0),vec3<f32>(255.0))));
    destination[id.y*size.x*2u+id.x] = rgb.x | (rgb.y<<8u) | (rgb.z<<16u);
}
