// ZenLink Liquid Glass layer.
//
// Optics adapted (re-implemented, not copied) from al1h3n/liquid-rust
// (MIT OR Apache-2.0) and shayann07/liquidglass (Apache-2.0): rounded-rect
// SDF, circular bevel profile, Snell refraction with a small RGB
// dispersion, specular rim from a top-left light with a weaker counter light.

cbuffer P : register(b0)
{
    float2 winSize;   // window, physical px
    float2 srcSize;   // backdrop texture (window + 2 * pad)
    float2 pad;       // backdrop margin around the window, px
    float  sigma;     // blur sigma in taps of the current pass
    float  dark;
    float4 dir;       // blur step in uv per tap (xy)
    float4 light;     // xy: light direction (towards the light)
    int    count;     // number of shapes
    float  lens;      // 0..1 refraction strength
    float  rimK;      // rim brightness
    float  scale;     // DPI scale
    float4 rc[8];     // centre xy, half size zw (window px)
    float4 rp[8];     // radius, bezel width, strength, unused
};

Texture2D src : register(t0);
SamplerState smp : register(s0);

struct VO { float4 pos : SV_Position; float2 uv : TEXCOORD0; };

VO vs(uint id : SV_VertexID)
{
    VO o;
    float2 t = float2((id << 1) & 2, id & 2);
    o.uv = t;
    o.pos = float4(t * float2(2, -2) + float2(-1, 1), 0, 1);
    return o;
}

// Separable Gaussian, one direction per pass.
float4 blur(VO i) : SV_Target
{
    float s = max(sigma, 0.01);
    int r = (int)min(ceil(s * 2.5), 24.0);
    float4 acc = 0;
    float ws = 0;
    for (int k = -r; k <= r; k++) {
        float w = exp(-(k * k) / (2 * s * s));
        acc += src.SampleLevel(smp, i.uv + dir.xy * k, 0) * w;
        ws += w;
    }
    return acc / ws;
}

float sdRR(float2 p, float2 h, float r)
{
    float2 q = abs(p) - h + r;
    return length(max(q, 0)) + min(max(q.x, q.y), 0) - r;
}

float2 sdNormal(float2 p, float2 h, float r)
{
    const float e = 0.5;
    float2 g = float2(sdRR(p + float2(e, 0), h, r) - sdRR(p - float2(e, 0), h, r),
                      sdRR(p + float2(0, e), h, r) - sdRR(p - float2(0, e), h, r));
    return g / max(length(g), 1e-5);
}

// Lateral shift of a ray through a surface of this slope (Snell).
float refractShift(float slope, float eta)
{
    float th = atan(slope);
    return tan(th - asin(sin(th) / eta));
}

float4 comp(VO i) : SV_Target
{
    float2 p = i.uv * winSize;
    float2 off = 0;
    float rim = 0;
    float cov = 1;
    for (int k = 0; k < count; k++) {
        float2 c = rc[k].xy;
        float2 h = rc[k].zw;
        float r = rp[k].x;
        float bz = max(rp[k].y, 1);
        float st = rp[k].z;
        float d = sdRR(p - c, h, r);
        if (k == 0) cov = saturate(0.5 - d);
        if (d < 0.5 && d > -bz) {
            // u: 0 at the rim, 1 where the bevel meets the flat top.
            float u = saturate(-d / bz);
            float a = 1 - u;
            float slope = min(a / sqrt(max(1 - a * a, 1e-3)), 6.0);
            float2 n = sdNormal(p - c, h, r);
            // The bevel pulls in the scene from just outside the edge.
            off += n * refractShift(slope, 1.5) * bz * 0.55 * st * lens;
            float edge = exp(min(d, 0) / (1.4 * scale));
            float l = max(dot(n, light.xy), 0) + 0.35 * max(dot(n, -light.xy), 0);
            rim = max(rim, edge * l * st);
        }
    }
    float2 uv = (p + pad + off) / srcSize;
    float2 dc = off * 0.06 / srcSize;
    float3 col = float3(src.Sample(smp, uv + dc).r, src.Sample(smp, uv).g, src.Sample(smp, uv - dc).b);
    col += rim * rimK;
    return float4(col * cov, cov);
}
