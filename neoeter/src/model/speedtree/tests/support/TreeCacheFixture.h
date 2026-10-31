#pragma once

// Synthetic SpeedTree geometry-cache fixtures. The layout mirrors
// SpeedTreeLib/TreeCacheFormat.h; no game assets are involved.

#include <Neoeter/Model/SpeedTree/TreeCacheFormat.h>

#include <cstdint>
#include <cstring>
#include <string>
#include <vector>

namespace TreeCacheFixture
{

class Writer
{
public:
    template <typename T>
    void Raw(const T & value)
    {
        const unsigned char * bytes = reinterpret_cast<const unsigned char *>(&value);
        m_data.insert(m_data.end(), bytes, bytes + sizeof(T));
    }

    void U32(uint32_t value) { Raw(value); }
    void I32(int32_t value) { Raw(value); }
    void U16(uint16_t value) { Raw(value); }
    void U8(uint8_t value) { Raw(value); }
    void F32(float value) { Raw(value); }

    void Bytes(const std::string & value)
    {
        U32(static_cast<uint32_t>(value.size()));
        m_data.insert(m_data.end(), value.begin(), value.end());
    }

    const std::vector<unsigned char> & Data() const { return m_data; }

private:
    std::vector<unsigned char> m_data;
};

inline void WriteIndexedPart(Writer & w, uint32_t vertexCount, uint32_t stripCount,
                             uint16_t lod, float alphaTest,
                             const std::vector<uint16_t> & indices,
                             const std::vector<float> & coords,
                             const std::vector<uint32_t> & colors,
                             const std::vector<float> & texCoords0,
                             const std::vector<float> & texCoords1)
{
    w.U32(vertexCount);
    w.U32(stripCount);
    w.U16(lod);
    w.F32(alphaTest);
    w.U32(static_cast<uint32_t>(indices.size()));
    for (uint16_t index : indices)
        w.U16(index);
    for (float value : coords)
        w.F32(value);
    for (uint32_t color : colors)
        w.U32(color);
    for (float value : texCoords0)
        w.F32(value);
    for (float value : texCoords1)
        w.F32(value);
    for (uint32_t i = 0; i < vertexCount * 3; ++i)
        w.F32(0.0f); // normals
}

inline void WriteLeafSet(Writer & w, bool active, int32_t lod, float alphaTest,
                         uint32_t leafCount)
{
    w.U8(active ? 1 : 0);
    w.I32(lod);
    w.F32(alphaTest);
    w.U32(leafCount);
    for (uint32_t i = 0; i < leafCount * 3; ++i)
        w.F32(0.0f);
    for (uint32_t i = 0; i < leafCount; ++i)
        w.U32(0xFFFFFFFFu);
    for (uint32_t i = 0; i < leafCount; ++i)
    {
        for (int k = 0; k < 16; ++k)
            w.F32(0.0f);
        for (int k = 0; k < 8; ++k)
            w.F32(0.0f);
        w.U8(0);
        w.U8(0);
    }
}

inline void WriteBillboard(Writer & w, bool active, float alphaTest)
{
    w.U8(active ? 1 : 0);
    w.F32(alphaTest);
    for (int i = 0; i < 12; ++i)
        w.F32(0.0f);
    for (int i = 0; i < 8; ++i)
        w.F32(0.0f);
}

inline std::vector<unsigned char> MakeBranchesOnly(uint32_t hash, float height)
{
    Writer w;
    w.U32(TreeCache::kMagic);
    w.U32(hash);
    const float bbox[6] = {-10.0f, -20.0f, -30.0f, 10.0f, 20.0f, height};
    for (float value : bbox)
        w.F32(value);

    // One cylinder collision object.
    w.U32(1);
    w.I32(1); // CO_CYLINDER
    w.F32(1.0f);
    w.F32(2.0f);
    w.F32(3.0f);
    w.F32(0.5f);
    w.F32(height * 0.5f);
    w.F32(0.5f);

    const std::vector<uint16_t> indices = {0, 1, 2, 3};
    const std::vector<float> coords = {
        0.0f, 0.0f, 0.0f,
        1.0f, 0.0f, 0.0f,
        1.0f, 1.0f, 0.0f,
        0.0f, 1.0f, 0.0f,
    };
    const std::vector<uint32_t> colors(4, 0xFFFFFFFFu);
    const std::vector<float> tex0 = {0.0f, 0.0f, 1.0f, 0.0f, 1.0f, 1.0f, 0.0f, 1.0f};
    const std::vector<float> tex1 = tex0;

    WriteIndexedPart(w, 4, 1, 0, 0.25f, indices, coords, colors, tex0, tex1);
    WriteIndexedPart(w, 0, 0, 0, 0.0f, {}, {}, {}, {}, {});

    WriteLeafSet(w, false, 0, 0.0f, 0);
    WriteLeafSet(w, false, 0, 0.0f, 0);
    WriteBillboard(w, false, 0.0f);
    WriteBillboard(w, false, 0.0f);

    w.Bytes("SyntheticBark.dds");
    w.Bytes("SyntheticComposite.dds");
    w.Bytes("SyntheticShadow.dds");
    return w.Data();
}

// Well-formed header followed by a leaf count that promises far more data than
// the file contains. A reader must reject this before allocating.
inline std::vector<unsigned char> MakeHugeLeafCount(uint32_t hash)
{
    Writer w;
    w.U32(TreeCache::kMagic);
    w.U32(hash);
    const float bbox[6] = {-10.0f, -20.0f, -30.0f, 10.0f, 20.0f, 123.5f};
    for (float value : bbox)
        w.F32(value);
    w.U32(0); // collisions

    WriteIndexedPart(w, 0, 0, 0, 0.0f, {}, {}, {}, {}, {});
    WriteIndexedPart(w, 0, 0, 0, 0.0f, {}, {}, {}, {}, {});

    w.U8(1);
    w.I32(0);
    w.F32(1.0f);
    w.U32(0xFFFFFFFFu); // leaf count with no payload
    return w.Data();
}

} // namespace TreeCacheFixture
