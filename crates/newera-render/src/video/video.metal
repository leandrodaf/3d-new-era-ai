// Compiled with the shared vertex layout, material patterns and samplers.
struct RasterVertex {
    float4 position [[position]];
    float4 color;
    float3 normal;
    float2 uv;
    uint kind [[flat]];
};

vertex RasterVertex video_vertex(uint index [[vertex_id]], device const Vertex *vertices [[buffer(0)]],
                                 constant float4x4 &view [[buffer(1)]]) {
    Vertex v = vertices[index];
    return {view * float4(float3(v.position),1), float4(v.color), float3(v.normal), float2(v.uv), v.kind};
}

struct RasterOutput {
    float4 color [[color(0)]];
    float4 normal_distance [[color(1)]];
};

RasterOutput shadeVideo(RasterVertex in, Textures textures) {
    uint kind = in.kind & 0x007fffffu;
    float2 uv = in.uv * float2(1,-1);
    if ((in.kind & 0x80000000u) && kind >= 100) {
        float cutoff = float((in.kind >> 23) & 255u) / 255.0f;
        if (textures.sample(nearestRepeat, uv, kind - 100).a < cutoff) discard_fragment();
    }
    float3 normal = normalizedOrZero(in.normal);
    float3 detail = kind >= 100 ? textures.sample(linearRepeat, uv, kind - 100).rgb
                               : float3(pattern(kind, in.uv, length(dfdx(in.uv))));
    float ambient = 0.86f + 0.14f * (0.5f * normal.y + 0.5f);
    float diffuse = max(0.0f, dot(normal, normalize(float3(0.4f,1.0f,0.3f))));
    float3 rgb = min(in.color.rgb * detail * (ambient + diffuse * 0.22f), 1.0f);
    return {float4(rgb,saturate(in.color.a)),float4(normal,1.0f / in.position.w)};
}

fragment RasterOutput video_opaque(RasterVertex in [[stage_in]], Textures textures [[texture(0)]]) {
    return shadeVideo(in, textures);
}
fragment float4 video_glass(RasterVertex in [[stage_in]], Textures textures [[texture(0)]]) {
    return shadeVideo(in, textures).color;
}

bool videoEdge(float4 surface, float4 neighbor) {
    if ((surface.w > 0.0f) != (neighbor.w > 0.0f)) return true;
    if (surface.w <= 0.0f) return false;
    return abs(surface.w - neighbor.w) > 0.03f * min(surface.w,neighbor.w)
        || dot(surface.xyz,neighbor.xyz) < 0.8f;
}

kernel void video_outline(texture2d<float,access::read> color [[texture(0)]],
                          texture2d<float,access::read> normals [[texture(1)]],
                          texture2d<float,access::write> outlined [[texture(2)]],
                          uint2 p [[thread_position_in_grid]]) {
    float4 surface = normals.read(p);
    bool edge = (p.x + 2 < color.get_width() && videoEdge(surface,normals.read(p + uint2(2,0))))
             || (p.y + 2 < color.get_height() && videoEdge(surface,normals.read(p + uint2(0,2))));
    float3 rgb = color.read(p).rgb;
    outlined.write(float4(edge ? mix(rgb,float3(0.22f),0.55f) : rgb,1.0f),p);
}

kernel void video_resolve(texture2d<float,access::read> color [[texture(0)]],
                          device uchar4 *output [[buffer(0)]],
                          uint2 p [[thread_position_in_grid]]) {
    uint2 source = p * 2;
    float3 rgb = (color.read(source).rgb + color.read(source + uint2(1,0)).rgb
                + color.read(source + uint2(0,1)).rgb + color.read(source + uint2(1,1)).rgb) * 0.25f;
    output[p.y * (color.get_width()/2) + p.x] = uchar4(uchar3(round(saturate(rgb)*255.0f)),255);
}
