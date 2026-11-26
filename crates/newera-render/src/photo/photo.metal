#include <metal_stdlib>
#include <metal_raytracing>
using namespace metal;
using namespace raytracing;

// Match the packed Rust Vertex and the bridge's Params/Lamp/Pixel layouts.
struct Vertex {
    packed_float3 position, normal;
    packed_float4 color;
    packed_float2 uv;
    uint kind;
};
struct Lamp {
    float4 position_radius, intensity_angle, panel_u, panel_v;
};
struct Params {
    float4 eye, forward, right, up, sky, sun_direction, sun_color;
    uint width, height, first_row, rows, samples, bounces, opaque_triangles, light_count;
    float squeeze;
    uint coarse;
    uint2 padding;
};
struct Pixel {
    float4 color, albedo, normal_depth;
};
struct Hit {
    float t;
    uint triangle;
    float2 uv;
    bool found;
};
using Textures = texture2d_array<float>;

constexpr sampler linearRepeat(coord::normalized, address::repeat, filter::linear);
constexpr sampler nearestRepeat(coord::normalized, address::repeat, filter::nearest);

float luminance(float3 c) { return dot(c, float3(0.2126, 0.7152, 0.0722)); }

float random(thread ulong &state) {
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    return float((state * 0x2545F4914F6CDD1DUL) >> 40) / 16777216.0f;
}

float3 normalizedOrZero(float3 v) {
    float magnitude = length(v);
    return magnitude > 0.0f ? v / magnitude : float3(0);
}

float3 hemisphere(float3 normal, thread ulong &state) {
    float phi = 2.0f * M_PI_F * random(state);
    float r2 = random(state);
    float3 helper = abs(normal.x) > 0.9f ? float3(0,1,0) : float3(1,0,0);
    float3 tangent = normalize(cross(helper, normal));
    return normalize(tangent * (sqrt(r2) * cos(phi))
                     + cross(normal, tangent) * (sqrt(r2) * sin(phi))
                     + normal * sqrt(1.0f - r2));
}

float tent(float r) {
    r *= 2.0f;
    return r < 1.0f ? sqrt(r) - 1.0f : 1.0f - sqrt(2.0f - r);
}

float hash21(float2 xy) {
    float2 q = fract(xy * float2(123.34f, 456.21f));
    q += dot(q, q + 45.32f);
    return fract(q.x * q.y);
}

float noise(float2 xy) {
    float2 cell = floor(xy);
    float2 f = fract(xy);
    f = f * f * (3.0f - 2.0f * f);
    return mix(mix(hash21(cell), hash21(cell + float2(1,0)), f.x),
               mix(hash21(cell + float2(0,1)), hash21(cell + 1.0f), f.x), f.y);
}

float fbm(float2 xy) {
    float result = 0.0f;
    float scale = 0.5f;
    for (uint i = 0; i < 4; ++i) {
        result += scale * noise(xy);
        xy = xy * 2.03f + float2(17.1f, 9.2f);
        scale *= 0.5f;
    }
    return result;
}

float joint(float x, float width, float scale, float pixel) {
    float aa = max(pixel * scale, 1e-4f);
    return 1.0f - smoothstep(width - aa, width + aa, abs(fract(x + 0.5f) - 0.5f));
}

// Mirrors the procedural finishes in patterns.rs at the photo filter width.
float pattern(uint kind, float2 uv, float pixel = 0.002f) {
    float u = uv.x, v = uv.y;
    if (kind == 1 || kind == 12) {
        float row = floor(v);
        float x = u + hash21(float2(row, kind == 1 ? 3.0f : 5.0f)) * (kind == 1 ? 5.0f : 3.0f);
        float tone = 0.8f + (kind == 1 ? 0.02f : 0.0f) + 0.3f * hash21(float2(floor(x), row));
        if (kind == 12) {
            float grain = fbm(float2(x * 3.0f, v * 10.0f));
            return tone * (0.85f + 0.15f * grain) * (1.0f - 0.7f * max(joint(v,0.06f, 1.0f, pixel), joint(x,0.003f, 1.0f, pixel)));
        }
        float grain = fbm(float2(x * 2.0f, v * 24.0f));
        float rings = 0.5f + 0.5f * sin((v * 18.0f + grain * 6.0f) * M_PI_F);
        return tone * (0.8f + 0.12f * grain + 0.1f * rings)
             * (1.0f - 0.45f * max(joint(v,0.02f, 1.0f, pixel), joint(x,0.004f, 1.0f, pixel)));
    }
    if (kind == 2) {
        float2 cell = floor(uv * 2.0f), f = fract(uv * 2.0f);
        float flip = fract((cell.x + cell.y) * 0.5f) * 2.0f;
        float along = mix(f.x, f.y, flip), across = mix(f.y, f.x, flip) * 3.0f;
        float tone = 0.8f + 0.32f * hash21(cell * 7.0f + floor(across));
        float grain = noise(float2(along * 20.0f, across * 2.0f + cell.x * 3.1f));
        float seams = max(max(joint(u * 2.0f,0.012f,2, pixel), joint(v * 2.0f,0.012f,2, pixel)), joint(across,0.03f,6, pixel));
        return tone * (0.9f + 0.1f * grain) * (1.0f - 0.45f * seams);
    }
    if (kind == 3) {
        float tone = 0.95f + 0.08f * hash21(floor(uv));
        return mix(tone * (0.97f + 0.05f * noise(uv * 40.0f)), 0.72f, max(joint(u,0.006f, 1.0f, pixel), joint(v,0.006f, 1.0f, pixel)));
    }
    if (kind == 4 || kind == 5 || kind == 13) {
        float row = floor(v), x = u + fract(row * 0.5f);
        float lines = max(joint(v,kind == 4 ? 0.025f : 0.07f, 1.0f, pixel), joint(x,kind == 4 ? 0.012f : 0.02f, 1.0f, pixel));
        float hash = hash21(float2(floor(x),row));
        if (kind == 4) return mix(0.97f + 0.04f * hash, 0.75f, lines);
        float grain = fbm(uv * float2(12,4));
        if (kind == 5) return mix((0.75f + 0.4f * hash) * (0.88f + 0.16f * grain), 1.25f, lines);
        return mix((0.94f + 0.08f * hash) * (0.95f + 0.06f * grain), 0.8f, lines);
    }
    if (kind == 6) {
        float2 cell = floor(uv * 3.0f), f = fract(uv * 3.0f), winner = 0;
        float first = 8.0f, second = 8.0f;
        for (int y = -1; y <= 1; ++y) {
            for (int x = -1; x <= 1; ++x) {
                float2 offset = float2(x,y), id = cell + offset;
                float2 seed = offset + float2(hash21(id), hash21(id + 19.7f)) * 0.8f + 0.1f;
                float distance = length(seed - f);
                if (distance < first) {
                    second = first;
                    first = distance;
                    winner = id;
                } else {
                    second = min(second, distance);
                }
            }
        }
        float aa = max(pixel * 3.0f, 1e-4f);
        float gap = 1.0f - smoothstep(0.04f - aa, 0.04f + aa, second - first);
        return mix(0.75f + 0.4f * hash21(winner) + 0.1f * fbm(uv * 10.0f), 0.45f, gap);
    }
    if (kind == 7) return 0.86f + 0.16f * fbm(uv * 3.0f) + 0.06f * (noise(uv * 90.0f) - 0.5f);
    if (kind == 8) {
        float vein = abs(sin((u * 1.3f + v * 0.7f) * 3.0f + fbm(uv * 2.5f) * 5.0f));
        return (0.8f + 0.22f * pow(vein, 0.35f)) * (1.0f - 0.25f * max(joint(u,0.002f, 1.0f, pixel),joint(v,0.002f, 1.0f, pixel)));
    }
    if (kind == 9) return 0.88f + 0.14f * noise(uv * 160.0f) + 0.06f * fbm(uv * 6.0f);
    if (kind == 10) return 0.8f + 0.25f * fbm(uv * 4.0f) + 0.08f * (noise(uv * 60.0f) - 0.5f);
    if (kind == 11) {
        float warp = fbm(uv * 3.0f) * 4.0f;
        float ripple = abs(sin(u * 9.0f + warp) * cos(v * 7.0f - warp));
        return 0.88f + 0.2f * ripple * ripple * ripple + 0.05f * fbm(uv * 8.0f);
    }
    return 1.0f;
}

float2 textureUV(device const Vertex *vertices, uint3 ids, float2 bary) {
    return float2(vertices[ids.x].uv) * (1.0f - bary.x - bary.y)
         + float2(vertices[ids.y].uv) * bary.x + float2(vertices[ids.z].uv) * bary.y;
}

uint3 triangleIndices(device const uint *indices, uint primitive) {
    device const uint *tri = indices + 3 * primitive;
    return uint3(tri[0], tri[1], tri[2]);
}

Hit intersectScene(primitive_acceleration_structure scene, device const Vertex *vertices,
                   device const uint *indices, Textures textures, float3 origin, float3 dir, float limit) {
    ray beam(origin, dir, 0.0001f, limit);
    intersector<triangle_data> trace;
    trace.assume_geometry_type(geometry_type::triangle);
    while (beam.min_distance < limit) {
        auto hit = trace.intersect(beam, scene);
        if (hit.type != intersection_type::triangle || !(hit.distance < limit)) break;
        uint3 ids = triangleIndices(indices, hit.primitive_id);
        uint kind = vertices[ids.x].kind;
        uint layer = kind & 0x007fffffu;
        if ((kind & 0x80000000u) && layer >= 100) {
            float2 uv = textureUV(vertices, ids, hit.triangle_barycentric_coord);
            float cutoff = float((kind >> 23) & 255u) / 255.0f;
            float alpha = textures.sample(nearestRepeat, uv * float2(1,-1), layer - 100).a;
            if (alpha < cutoff) {
                // Hit distances are positive finite floats. Advance by one ULP
                // without crossing a distinct surface just behind the cutout.
                beam.min_distance = as_type<float>(as_type<uint>(hit.distance) + 1u);
                continue;
            }
        }
        return {hit.distance, hit.primitive_id, hit.triangle_barycentric_coord, true};
    }
    return {limit, 0, float2(0), false};
}

float transmission(primitive_acceleration_structure scene, device const Vertex *vertices,
                   device const uint *indices, Textures textures, float3 origin, float3 dir,
                   float reach, uint opaque) {
    float through = 1.0f;
    for (uint i = 0; i < 4 && reach > 0.0f; ++i) {
        Hit hit = intersectScene(scene, vertices, indices, textures, origin, dir, reach);
        if (!hit.found) break;
        if (hit.triangle < opaque) return 0.0f;
        float alpha = vertices[indices[hit.triangle * 3]].color.w;
        through *= 1.0f - saturate(alpha) * 0.85f;
        origin += dir * (hit.t + 0.001f);
        reach -= hit.t + 0.001f;
    }
    return through;
}

float3 toward(Lamp lamp, float3 dir) {
    if (lamp.panel_u.w > 0.0f) {
        return lamp.intensity_angle.rgb * max(0.0f, dot(normalizedOrZero(cross(lamp.panel_u.xyz, lamp.panel_v.xyz)), dir));
    }
    if (lamp.intensity_angle.w <= 0.0f) return lamp.intensity_angle.rgb;
    if (dir.y >= 0.0f) return float3(0);
    float theta = acos(min(-dir.y, 1.0f)) / lamp.intensity_angle.w;
    return lamp.intensity_angle.rgb * exp(-M_LN2_F * theta * theta);
}

float seen(Lamp lamp, float3 origin, float3 dir, thread float3 &glow) {
    float3 center = lamp.position_radius.xyz;
    if (lamp.panel_u.w > 0.0f) {
        float3 u = lamp.panel_u.xyz, v = lamp.panel_v.xyz;
        float3 normal = normalizedOrZero(cross(u,v));
        float facing = dot(normal, dir);
        if (facing >= -1e-8f) return INFINITY;
        float t = dot(normal, center - origin) / facing;
        float3 offset = origin + dir * t - center;
        if (t <= 0.0f || abs(dot(offset,u)) > dot(u,u) || abs(dot(offset,v)) > dot(v,v)) return INFINITY;
        glow = lamp.intensity_angle.rgb;
        return t;
    }
    float3 oc = origin - center;
    float radius2 = lamp.position_radius.w * lamp.position_radius.w;
    float b = dot(oc, dir), disc = b * b - dot(oc,oc) + radius2;
    if (disc < 0.0f) return INFINITY;
    float t = -b - sqrt(disc);
    if (t <= 0.0f) return INFINITY;
    glow = toward(lamp,-dir) / max(M_PI_F * radius2, 1e-6f);
    return t;
}

float lampWeight(Lamp lamp, float3 at, float3 normal) {
    bool isPanel = any(lamp.panel_u.xyz != 0.0f) || any(lamp.panel_v.xyz != 0.0f);
    if (!isPanel && dot(normal,lamp.position_radius.xyz - at) <= 0.0f) return 0.0f;
    float area = isPanel ? lamp.panel_u.w : 1.0f;
    float3 offset = lamp.position_radius.xyz - at;
    return luminance(lamp.intensity_angle.rgb) * area / max(dot(offset,offset), 0.04f);
}

float3 radiance(primitive_acceleration_structure scene, device const Vertex *vertices,
                device const uint *indices, device const Lamp *lamps, Textures textures,
                constant Params &p, float3 origin, float3 dir, thread ulong &rng,
                thread Pixel &first) {
    float3 throughput(1), total(0);
    uint depth = 0, crossed = 0;
    while (true) {
        Hit hit = intersectScene(scene, vertices, indices, textures, origin, dir, 1e4f);
        if (depth == 0) {
            float nearest = INFINITY;
            float3 lightGlow = 0;
            for (uint i = 0; i < p.light_count; ++i) {
                float3 glow;
                float t = seen(lamps[i], origin, dir, glow);
                if (t < nearest) { nearest = t; lightGlow = glow; }
            }
            if (nearest <= hit.t + 0.02f || (!hit.found && isfinite(nearest))) {
                first.albedo = 1.0f;
                first.normal_depth = float4(-dir, nearest);
                return total + throughput * lightGlow;
            }
        }
        if (!hit.found) {
            if (first.normal_depth.w == 0) {
                first.albedo = 1.0f;
                first.normal_depth = float4(-dir, 1e4f);
            }
            float3 sunDisc = depth == 0 && dot(dir,p.sun_direction.xyz) > 0.9995f ? p.sun_color.rgb * 0.02f : float3(0);
            return total + throughput * (p.sky.rgb + sunDisc);
        }
        uint3 ids = triangleIndices(indices, hit.triangle);
        Vertex a = vertices[ids.x], b = vertices[ids.y], c = vertices[ids.z];
        float2 bary = hit.uv;
        float wa = 1.0f - bary.x - bary.y;
        float3 point = origin + dir * hit.t;
        float3 normal = normalizedOrZero(float3(a.normal) * wa + float3(b.normal) * bary.x + float3(c.normal) * bary.y);
        if (all(normal == 0.0f)) normal = normalizedOrZero(cross(float3(b.position) - float3(a.position), float3(c.position) - float3(a.position)));
        if (dot(normal,dir) > 0.0f) normal = -normal;
        float4 color = float4(a.color) * wa + float4(b.color) * bary.x + float4(c.color) * bary.y;
        uint kind = a.kind & 0x007fffffu;
        float2 uv = textureUV(vertices, ids, bary);
        float3 detail = kind >= 100 ? textures.sample(linearRepeat, uv * float2(1,-1), kind - 100).rgb
                                   : float3(pattern(kind, uv));
        float3 base = min(color.rgb * pow(detail, float3(2.2f)), 0.95f);
        float alpha = saturate(color.a);
        if (hit.triangle >= p.opaque_triangles && crossed < 6) {
            if (alpha <= 0.5f) {
                float fresnel = 0.04f + 0.96f * pow(1.0f - abs(dot(normal, -dir)), 5.0f);
                float3 reflected = reflect(dir,normal);
                if (!intersectScene(scene, vertices, indices, textures, point + reflected * 0.001f, reflected, 1e4f).found) {
                    total += throughput * p.sky.rgb * fresnel;
                }
                origin = point + dir * 0.001f;
                throughput *= mix(float3(1), base, 0.15f + alpha * 0.3f) * (1.0f - fresnel);
                ++crossed;
                continue;
            }
            if (random(rng) > alpha * 0.6f) {
                origin = point + dir * 0.001f;
                throughput *= mix(float3(1), base, 0.15f);
                ++crossed;
                continue;
            }
        }
        if (first.normal_depth.w == 0) {
            first.albedo = float4(base, 0);
            first.normal_depth = float4(normal, hit.t);
        }
        float3 at = point + normal * 0.001f;
        float sunCos = dot(normal, p.sun_direction.xyz);
        if (sunCos > 0.0f) {
            float visible = transmission(scene,vertices,indices,textures,at,p.sun_direction.xyz,1e4f,p.opaque_triangles);
            total += throughput * base * p.sun_color.rgb * (sunCos * visible / M_PI_F);
        }

        float weightSum = 0.0f;
        for (uint i = 0; i < p.light_count; ++i) weightSum += lampWeight(lamps[i],at,normal);
        if (weightSum > 0.0f) {
            float target = random(rng) * weightSum;
            float chosenWeight = 0.0f;
            uint pick = p.light_count - 1;
            for (uint i = 0; i < p.light_count; ++i) {
                chosenWeight = lampWeight(lamps[i],at,normal);
                if (target < chosenWeight) { pick = i; break; }
                target -= chosenWeight;
            }
            Lamp lamp = lamps[pick];
            float3 source = lamp.position_radius.xyz;
            float area = 1.0f;
            if (lamp.panel_u.w > 0.0f) {
                float u = random(rng), v = random(rng);
                source += lamp.panel_u.xyz * (u * 2.0f - 1.0f) + lamp.panel_v.xyz * (v * 2.0f - 1.0f);
                area = lamp.panel_u.w;
            } else {
                source += hemisphere(float3(0,1,0), rng) * lamp.position_radius.w;
            }
            float3 offset = source - at;
            float distance = length(offset);
            float3 l = offset / max(distance, 1e-4f);
            float cosLight = dot(normal,l);
            if (cosLight > 0.0f) {
                float reach = max(0.0f, distance - max(lamp.position_radius.w,lamp.panel_v.w));
                float visible = transmission(scene,vertices,indices,textures,at,l,reach,p.opaque_triangles);
                total += throughput * base * toward(lamp,-l) * (cosLight * area * visible /
                    max(chosenWeight / weightSum, 1e-6f) / max(distance * distance, 0.04f) / M_PI_F);
            }
        }
        if (depth >= p.bounces) return total;
        throughput *= base;
        if (depth >= 2) {
            float survive = clamp(max(max(throughput.x,throughput.y),throughput.z), 0.05f, 0.95f);
            if (random(rng) > survive) return total;
            throughput /= survive;
        }
        origin = at;
        dir = hemisphere(normal,rng);
        ++depth;
    }
}

kernel void photo(primitive_acceleration_structure scene [[buffer(0)]],
                  device const Vertex *vertices [[buffer(1)]],
                  device const uint *indices [[buffer(2)]],
                  device const Lamp *lamps [[buffer(3)]],
                  constant Params &p [[buffer(4)]],
                  device Pixel *output [[buffer(5)]],
                  Textures textures [[texture(0)]],
                  uint id [[thread_position_in_grid]]) {
    uint x = id % p.width, y = p.first_row + id / p.width;
    ulong rng = (((ulong(y) << 32) | ulong(x)) * 0x9E3779B97F4A7C15UL) | 1UL;
    Pixel sum = {};
    for (uint i = 0; i < p.samples; ++i) {
        float jx = p.coarse ? 0.5f : 0.5f + tent(random(rng));
        float jy = p.coarse ? 0.5f : 0.5f + tent(random(rng));
        float sx = (2.0f * (float(x) + jx) / float(p.width) - 1.0f) * p.right.w;
        float sy = (1.0f - 2.0f * (float(y) + jy) / float(p.height)) * p.up.w;
        float3 dir = normalize(p.forward.xyz + p.right.xyz * sx + p.up.xyz * sy);
        Pixel first = {};
        float3 value = radiance(scene,vertices,indices,lamps,textures,p,p.eye.xyz,dir,rng,first);
        if (all(isfinite(value))) {
            value = min(value, 20.0f);
            sum.color.rgb += value / (1.0f + luminance(value) * p.squeeze);
        }
        sum.albedo += first.albedo;
        sum.normal_depth += first.normal_depth;
    }
    float n = float(p.samples);
    float3 squeezed = sum.color.rgb / n;
    sum.color.rgb = squeezed / max(1.0f - luminance(squeezed) * p.squeeze, 0.05f);
    sum.albedo /= n;
    sum.normal_depth = float4(normalizedOrZero(sum.normal_depth.xyz), sum.normal_depth.w / n);
    output[id] = sum;
}
