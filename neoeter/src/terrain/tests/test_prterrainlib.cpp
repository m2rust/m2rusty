#include <Neoeter/Terrain/StdAfx.h>
#include <Neoeter/Terrain/TerrainType.h>
#include <Neoeter/Terrain/Terrain.h>
#include <Neoeter/Terrain/TextureSet.h>

#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Render/ResourceManager.h>

#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <vector>

namespace {

void test_float_conversion_macros()
{
    // Test PR_FLOAT_TO_INT
    int outInt = 0;
    PR_FLOAT_TO_INT(0.0f, outInt);
    assert(outInt == 0);

    PR_FLOAT_TO_INT(3.14f, outInt);
    assert(outInt == 3);

    PR_FLOAT_TO_INT(3.99f, outInt);
    assert(outInt == 3);

    PR_FLOAT_TO_INT(-2.3f, outInt);
    assert(outInt == -3);

    PR_FLOAT_TO_INT(-2.0f, outInt);
    assert(outInt == -2);

    // Test PR_FLOAT_TO_FIXED (16.16 fixed-point)
    long outFixed = 0;
    PR_FLOAT_TO_FIXED(1.0f, outFixed);
    assert(outFixed == 65536);

    PR_FLOAT_TO_FIXED(0.5f, outFixed);
    assert(outFixed == 32768);

    PR_FLOAT_TO_FIXED(0.0f, outFixed);
    assert(outFixed == 0);

    // Test PR_FLOAT_ADD_TO_INT
    long accum = 10;
    PR_FLOAT_ADD_TO_INT(4.8f, accum);
    assert(accum == 14);

    PR_FLOAT_ADD_TO_INT(-2.2f, accum);
    assert(accum == 11);

    std::printf("[PASS] test_float_conversion_macros\n");
}

void test_terrain_constants_and_types()
{
    assert(CTerrainImpl::XSIZE == 128);
    assert(CTerrainImpl::YSIZE == 128);
    assert(CTerrainImpl::HEIGHTMAP_RAW_XSIZE == 131);
    assert(CTerrainImpl::HEIGHTMAP_RAW_YSIZE == 131);
    assert(CTerrainImpl::ATTRMAP_XSIZE == 256);
    assert(CTerrainImpl::ATTRMAP_YSIZE == 256);
    assert(CTerrainImpl::WATERMAP_XSIZE == 128);
    assert(CTerrainImpl::WATERMAP_YSIZE == 128);
    assert(CTerrainImpl::SHADOWMAP_XSIZE == 256);
    assert(CTerrainImpl::SHADOWMAP_YSIZE == 256);
    assert(CTerrainImpl::PATCH_XSIZE == 16);
    assert(CTerrainImpl::PATCH_XCOUNT == 8);
    assert(CTerrainImpl::CELLSCALE == 200);
    assert(CTerrainImpl::TERRAIN_XSIZE == 25600);

    assert(CTerrainImpl::ATTRIBUTE_BLOCK == 1);
    assert(CTerrainImpl::ATTRIBUTE_WATER == 2);
    assert(CTerrainImpl::ATTRIBUTE_BANPK == 4);

    TERRAIN_VBUFFER vbuf = {};
    vbuf.VertexSize = sizeof(float) * 5;
    vbuf.NumIndices = 6;
    assert(vbuf.VertexSize == 20);
    assert(vbuf.NumIndices == 6);

    PR_MATERIAL mat = {};
    std::strncpy(mat.name, "grass", sizeof(mat.name) - 1);
    mat.diff_r = 0.5f;
    mat.diff_g = 0.8f;
    mat.diff_b = 0.2f;
    mat.diff_a = 1.0f;
    assert(std::strcmp(mat.name, "grass") == 0);
    assert(mat.diff_g == 0.8f);

    std::printf("[PASS] test_terrain_constants_and_types\n");
}

class TestTerrain : public CTerrainImpl
{
public:
    using CTerrainImpl::LoadHeightMap;
    using CTerrainImpl::LoadAttrMap;
    using CTerrainImpl::RAW_LoadTileMap;
    using CTerrainImpl::LoadWaterMapFile;
    using CTerrainImpl::m_awRawHeightMap;
    using CTerrainImpl::m_awShadowMap;
    using CTerrainImpl::m_abyAttrMap;
    using CTerrainImpl::m_abyWaterMap;
    using CTerrainImpl::m_byNumWater;
    using CTerrainImpl::m_lWaterHeight;
    using CTerrainImpl::GetHeightMapValue;
};

void test_terrain_impl_lifecycle_and_shadowmap()
{
    TestTerrain terrain;
    assert(terrain.GetHeightMapValue(0, 0) == 0);

    // Out-of-bounds shadow color check
    DWORD oobColor1 = terrain.GetShadowMapColor(-10.0f, 100.0f);
    assert(oobColor1 == 0xFFFFFFFF);
    DWORD oobColor2 = terrain.GetShadowMapColor(30000.0f, 100.0f);
    assert(oobColor2 == 0xFFFFFFFF);

    // Initialize shadow map pixel at (10, 10)
    // 16-bit 5-6-5 RGB format: e.g. R=31, G=63, B=31 -> white
    // Or R=31 (0x1F), G=0, B=0 -> 0x001F in 5-bit low channel
    terrain.m_awShadowMap[10 * CTerrainImpl::SHADOWMAP_XSIZE + 10] = 0x001F;
    float coordX = (10.0f / (CTerrainImpl::SHADOWMAP_XSIZE - 1)) * CTerrainImpl::TERRAIN_XSIZE;
    float coordY = (10.0f / (CTerrainImpl::SHADOWMAP_YSIZE - 1)) * CTerrainImpl::TERRAIN_XSIZE;
    DWORD color = terrain.GetShadowMapColor(coordX, coordY);
    // Alpha must be 0xFF
    assert((color & 0xFF000000) == 0xFF000000);

    std::printf("[PASS] test_terrain_impl_lifecycle_and_shadowmap\n");
}

void test_terrain_impl_file_loaders()
{
    TestTerrain terrain;

    // 1. Test HeightMap loading via VFS
    const char* heightmapPath = "/tmp/test_heightmap.raw";
    std::vector<WORD> mockHeight(CTerrainImpl::HEIGHTMAP_RAW_XSIZE * CTerrainImpl::HEIGHTMAP_RAW_YSIZE, 420);
    // Set distinctive value at grid (5, 5) -> raw index (6 * HEIGHTMAP_RAW_XSIZE + 6)
    mockHeight[6 * CTerrainImpl::HEIGHTMAP_RAW_XSIZE + 6] = 1337;

    {
        std::ofstream out(heightmapPath, std::ios::binary);
        out.write(reinterpret_cast<const char*>(mockHeight.data()), mockHeight.size() * sizeof(WORD));
    }

    bool hLoaded = terrain.LoadHeightMap(heightmapPath);
    assert(hLoaded);
    assert(terrain.GetHeightMapValue(5, 5) == 1337);
    assert(terrain.GetHeightMapValue(0, 0) == 420);

    // 2. Test AttrMap loading via VFS
    const char* attrmapPath = "/tmp/test_attrmap.raw";
#pragma pack(push, 1)
    struct MockAttrHeader
    {
        WORD magic = 2634;
        WORD width = CTerrainImpl::ATTRMAP_XSIZE;
        WORD height = CTerrainImpl::ATTRMAP_YSIZE;
    } attrHeader;
#pragma pack(pop)

    std::vector<BYTE> mockAttr(CTerrainImpl::ATTRMAP_XSIZE * CTerrainImpl::ATTRMAP_YSIZE, CTerrainImpl::ATTRIBUTE_BLOCK);
    mockAttr[10] = CTerrainImpl::ATTRIBUTE_WATER;

    {
        std::ofstream out(attrmapPath, std::ios::binary);
        out.write(reinterpret_cast<const char*>(&attrHeader), sizeof(attrHeader));
        out.write(reinterpret_cast<const char*>(mockAttr.data()), mockAttr.size());
    }

    bool aLoaded = terrain.LoadAttrMap(attrmapPath);
    assert(aLoaded);
    assert(terrain.m_abyAttrMap[0] == CTerrainImpl::ATTRIBUTE_BLOCK);
    assert(terrain.m_abyAttrMap[10] == CTerrainImpl::ATTRIBUTE_WATER);

    // 3. Test WaterMap loading via VFS
    const char* watermapPath = "/tmp/test_watermap.raw";
#pragma pack(push, 1)
    struct MockWaterHeader
    {
        WORD magic = 5426;
        WORD width = CTerrainImpl::WATERMAP_XSIZE;
        WORD height = CTerrainImpl::WATERMAP_YSIZE;
        BYTE layerCount = 2;
    } waterHeader;
#pragma pack(pop)

    std::vector<BYTE> mockWaterGrid(CTerrainImpl::WATERMAP_XSIZE * CTerrainImpl::WATERMAP_YSIZE, 0);
    mockWaterGrid[25] = 1;
    int32_t mockHeights[2] = { 500, 1200 };  // on-disk type is LONG (32-bit), not long

    {
        std::ofstream out(watermapPath, std::ios::binary);
        out.write(reinterpret_cast<const char*>(&waterHeader), sizeof(waterHeader));
        out.write(reinterpret_cast<const char*>(mockWaterGrid.data()), mockWaterGrid.size());
        out.write(reinterpret_cast<const char*>(mockHeights), sizeof(mockHeights));
    }

    bool wLoaded = terrain.LoadWaterMapFile(watermapPath);
    assert(wLoaded);
    assert(terrain.m_byNumWater == 2);
    assert(terrain.m_lWaterHeight[0] == 500);
    assert(terrain.m_lWaterHeight[1] == 1200);
    assert(terrain.m_abyWaterMap[25] == 1);

    // Clean up temporary files
    std::remove(heightmapPath);
    std::remove(attrmapPath);
    std::remove(watermapPath);

    std::printf("[PASS] test_terrain_impl_file_loaders\n");
}

void test_textureset_lifecycle()
{
    CTextureSet textureSet;
    textureSet.Create();
    assert(textureSet.GetTextureCount() == 1); // Eraser texture

    bool added = textureSet.AddTexture(
        "d:/ymir work/terrain/grass.dds",
        2.0f,
        3.0f,
        0.5f,
        0.25f,
        true,
        100,
        5000,
        1.0f
    );
    assert(added);
    assert(textureSet.GetTextureCount() == 2);

    TTerrainTexture& tex = textureSet.GetTexture(1);
    assert(tex.stFilename == "d:/ymir work/terrain/grass.dds");
    assert(std::fabs(tex.UScale - 2.0f) < 1e-4f);
    assert(std::fabs(tex.VScale - 3.0f) < 1e-4f);
    assert(std::fabs(tex.UOffset - 0.5f) < 1e-4f);
    assert(std::fabs(tex.VOffset - 0.25f) < 1e-4f);
    assert(tex.bSplat == true);
    assert(tex.Begin == 100);
    assert(tex.End == 5000);

    // Verify matrix scaling
    assert(std::fabs(tex.m_matTransform._11 - 2.0f) < 1e-4f);
    assert(std::fabs(tex.m_matTransform._22 - (-3.0f)) < 1e-4f);
    assert(std::fabs(tex.m_matTransform._41 - 0.5f) < 1e-4f);
    assert(std::fabs(tex.m_matTransform._42 - (-0.25f)) < 1e-4f);

    // Duplicate rejection
    bool dupAdded = textureSet.AddTexture(
        "d:/ymir work/terrain/grass.dds",
        1.0f, 1.0f, 0.0f, 0.0f, true, 0, 0, 1.0f
    );
    assert(!dupAdded);
    assert(textureSet.GetTextureCount() == 2);

    // Save test
    const char* savePath = "/tmp/test_textureset.txt";
    bool saved = textureSet.Save(savePath);
    assert(saved);

    // Load saved file
    CTextureSet loadedSet;
    bool loaded = loadedSet.Load(savePath, 1.0f);
    assert(loaded);
    assert(loadedSet.GetTextureCount() == 2);
    TTerrainTexture& loadedTex = loadedSet.GetTexture(1);
    assert(loadedTex.stFilename == "d:/ymir work/terrain/grass.dds");
    assert(std::fabs(loadedTex.UScale - 2.0f) < 1e-4f);
    assert(std::fabs(loadedTex.VScale - 3.0f) < 1e-4f);

    // Removal test
    bool removed = loadedSet.RemoveTexture(1);
    assert(removed);
    assert(loadedSet.GetTextureCount() == 1);

    std::remove(savePath);
    std::printf("[PASS] test_textureset_lifecycle\n");
}

} // namespace

int main()
{
    CEterPackManager packMgr;

    test_float_conversion_macros();
    test_terrain_constants_and_types();
    test_terrain_impl_lifecycle_and_shadowmap();
    test_terrain_impl_file_loaders();
    test_textureset_lifecycle();

    std::printf("All PRTerrainLib smoke tests passed!\n");
    return 0;
}
