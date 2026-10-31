#pragma once

// Synthetic GR2 fixtures for Gr2Reader tests and fuzzers. Every byte is
// generated here; no game models are used.

#include <cstdint>
#include <cstring>
#include <string>
#include <vector>

namespace Gr2Fixture
{

inline const unsigned char * Magic()
{
    static const unsigned char kMagic[16] = {
        0xb8, 0x67, 0xb0, 0xca, 0xf8, 0x6d, 0xb1, 0x0f,
        0x84, 0x72, 0x8c, 0x7e, 0x5e, 0x19, 0x00, 0x1e
    };
    return kMagic;
}

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
    void U16(uint16_t value) { Raw(value); }
    void Bytes(const void * data, size_t size)
    {
        const unsigned char * bytes = static_cast<const unsigned char *>(data);
        m_data.insert(m_data.end(), bytes, bytes + size);
    }
    void PatchU32(size_t offset, uint32_t value)
    {
        std::memcpy(&m_data[offset], &value, sizeof(value));
    }
    void Resize(size_t size) { m_data.resize(size, 0); }
    const std::vector<unsigned char> & Data() const { return m_data; }
    std::vector<unsigned char> & Data() { return m_data; }

private:
    std::vector<unsigned char> m_data;
};

// Minimal but structurally valid GR2: one section, a `granny_file_info` root
// type whose only member is `Models`, and one model with a name but no skeleton
// or meshes. Exercises the header, section table, relocation table, type tree,
// root node, model array and model record paths.
inline std::vector<unsigned char> MakeMinimalModel()
{
    constexpr size_t kSectionTable = 68;
    constexpr size_t kPayloadOffset = 112;
    constexpr uint32_t kSecOffset = kSectionTable - 32; // relative to byte 32

    Writer w;
    w.Bytes(Magic(), 16);
    w.Resize(32); // version/metadata, unused by the reader

    w.U32(0);              // 32
    w.U32(0);              // 36
    w.U32(0);              // 40
    w.U32(kSecOffset);     // 44
    w.U32(1);              // 48: numSec
    w.U32(0);              // 52: rootTypeSec
    w.U32(0);              // 56: rootTypeOff (patched below)
    w.U32(0);              // 60: rootNodeSec
    w.U32(0);              // 64: rootNodeOff (patched below)

    // Section header (44 bytes): fmt, offset, compSize, uncompSize, unknown,
    // f16, f8, relocOffset, relocCount, reserved.
    const size_t sectionHeader = w.Data().size();
    w.Resize(sectionHeader + 44);

    // Payload, offsets are relative to the section start.
    constexpr uint32_t kTypeOff = 0;
    constexpr uint32_t kNodeOff = 64;
    constexpr uint32_t kModelArrayOff = 80;
    constexpr uint32_t kModelOff = 84;
    constexpr uint32_t kNameOff = 168;
    constexpr uint32_t kTypeNameOff = 184;
    constexpr size_t kPayloadSize = 200;

    w.Resize(kPayloadOffset + kPayloadSize);

    std::vector<unsigned char> & data = w.Data();

    auto put32 = [&data](size_t offset, uint32_t value) {
        std::memcpy(&data[offset], &value, sizeof(value));
    };

    // Type tree: member type 2 (`Models`), then GrannyEndMember.
    put32(kPayloadOffset + kTypeOff + 0, 2);
    put32(kPayloadOffset + kTypeOff + 4, kTypeNameOff); // patched through reloc

    // Root node: model count + pointer to the model pointer array.
    put32(kPayloadOffset + kNodeOff + 0, 1);
    put32(kPayloadOffset + kNodeOff + 4, kModelArrayOff);

    // One-element model pointer array.
    put32(kPayloadOffset + kModelArrayOff, kModelOff);

    // Model record: name pointer, no skeleton, zero transform, no meshes.
    put32(kPayloadOffset + kModelOff + 0, kNameOff);

    std::memcpy(&data[kPayloadOffset + kNameOff], "SyntheticModel",
                sizeof("SyntheticModel"));
    std::memcpy(&data[kPayloadOffset + kTypeNameOff], "Models", sizeof("Models"));

    // Relocation table: (fromOffset, toSection, toOffset) in the section.
    const size_t relocOffset = data.size();
    const uint32_t relocations[4][3] = {
        {kTypeOff + 4, 0, kTypeNameOff},
        {kNodeOff + 4, 0, kModelArrayOff},
        {kModelArrayOff, 0, kModelOff},
        {kModelOff, 0, kNameOff},
    };
    for (const auto & relocation : relocations)
    {
        w.U32(relocation[0]);
        w.U32(relocation[1]);
        w.U32(relocation[2]);
    }

    // Section header fields.
    put32(sectionHeader + 0, 0); // fmt: raw
    put32(sectionHeader + 4, kPayloadOffset);
    put32(sectionHeader + 8, kPayloadSize);
    put32(sectionHeader + 12, kPayloadSize);
    put32(sectionHeader + 16, 0);
    put32(sectionHeader + 20, 0);
    put32(sectionHeader + 24, 0);
    put32(sectionHeader + 28, static_cast<uint32_t>(relocOffset));
    put32(sectionHeader + 32, 4);

    // Root offsets.
    put32(56, kTypeOff);
    put32(64, kNodeOff);

    return data;
}

} // namespace Gr2Fixture
