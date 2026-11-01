// Reference dump for crates/m2-world.
//
//   m2diff_world <client dir> <index file> maps
//
// For every map directory (a registered */setting.txt or */mapproperty.txt):
//
//   M   <dir>
//   MS  <ok> <countX> <countY> <viewRadius> <heightScale> <opaqueWater> <textures>
//   IB  <lod> <numIndices> <fnv of the WORDs> <primitives> <strip>
//   per NNNNNN directory (sorted):
//     T   <id> <loaded>                    CMapOutdoor::LoadTerrain, real code
//     PA  <px> <py> <type> <water> <minx> <maxx> <miny> <maxy> <minz> <maxz>
//         <fnv vertices> <water vertex count> <fnv water vertices>
//     SP  <texture> <active> <fnv level 0 alpha> <ok> <fnv mip1..mip4>
//     LT  <config> <fnv of the diffuse of all 64 patches>
//     UV  <fnv of tile/alpha/shadow coordinates of all 64 patches>
//     AN  <placed objects>
//     AP  ...                               CArea::__SetObjectInstance decisions
//   then for every player position:
//     W   <pos index> <curX> <curY> <slots> <points>
//     Q   <kind> <fnv> <count>              one line per query kind
//
// Fields are tab separated, floats are the hex of their bits.
//
// What is real C++: CMapOutdoor::LoadTerrain (all loaders, CalculateTerrainPatch,
// RAW_GenerateSplat/AddTexture32 on the compatibility D3D device with a null
// render backend), SetIndexBuffer, every query (GetTerrainHeight, GetNormal,
// GetWaterHeight, isAttrOn, GetAttr, GetShadowMapColor, GetTerrainNum,
// ConvertToMapCoords, the terrain picking), the patch lighting and
// __SoftwareTransformPatch_SetTransform, CPropertyManager and the
// prt::Property*StringToData functions, std::ranges::sort of the objects.
//
// Replayed here: the first block of CMapOutdoor::Update (it also builds areas
// and the sky), CArea::__SetObjectInstance's switch (the rest creates graphic
// instances), and the mip loop of AddTexture32 (the compatibility layer's
// GetLevelCount() is 1, so the C++ build never runs it).

#include <Client/Game/StdAfx.h>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <set>
#include <string>
#include <vector>

#include <fcntl.h>
#include <unistd.h>

#define private public
#include <Client/Game/MapOutdoor.h>
#undef private
#include <Client/Game/Area.h>
#include <Client/Game/AreaTerrain.h>
#include <Client/Game/MapType.h>
#include <Client/Game/Property.h>
#include <Client/Game/PropertyManager.h>
#include <Client/Game/TerrainPatch.h>
#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/CRC32.h>
#include <Neoeter/Platform/Base/FileLoader.h>
#include <Neoeter/Platform/Base/Filename.h>
#include <Neoeter/Platform/Base/Timer.h>
#include <Neoeter/Render/GrpImage.h>
#include <Neoeter/Render/Ray.h>
#include <Neoeter/Render/ResourceManager.h>
#include <Neoeter/Render/Util.h>
#include <Neoeter/Terrain/TextureSet.h>
#include <Platform/RenderBackend.h>

// The compatibility D3D device keeps its textures and buffers on the CPU and
// only talks to the render backend to upload them. A backend that hands out
// no handles makes every upload a no-op.
class NullBackend : public IRenderBackend
{
  public:
    RenderCaps Caps() const override
    {
        return {};
    }
    RenderHandle CreateTexture() override
    {
        return {0, 0};
    }
    RenderHandle CreateDepthStencil(uint32_t, uint32_t) override
    {
        return {0, 0};
    }
    void Destroy(RenderHandle) override
    {
    }
    void UploadTextureLevel(RenderHandle, uint32_t, uint32_t, uint32_t, uint32_t, const void *, size_t) override
    {
    }
    void FinalizeTexture(RenderHandle, uint32_t) override
    {
    }
    void BindDefaultRenderTarget() override
    {
    }
    void BindRenderTarget(RenderHandle) override
    {
    }
    void BindDepthStencil(RenderHandle) override
    {
    }
    void SetViewport(int32_t, int32_t, uint32_t, uint32_t) override
    {
    }
    void Clear(uint32_t, uint32_t, float, uint32_t) override
    {
    }
    void BeginFrame() override
    {
    }
    void Present() override
    {
    }
    void BindStageTexture(uint32_t, RenderHandle) override
    {
    }
    void ApplyState(const RenderState &) override
    {
    }
    void Draw(const DrawCall &) override
    {
    }
    bool Readback(uint32_t, uint32_t, uint32_t, uint32_t, uint32_t *) override
    {
        return false;
    }
};

static CResource *NewImage(const char *name)
{
    return new CGraphicImage(name);
}

IRenderBackend &Platform_GetRenderBackend()
{
    static NullBackend backend;
    return backend;
}

namespace
{

FILE *g_out = nullptr;

bool RegisterIndex(CEterPackManager &manager, const std::string &packFolder, const std::string &indexFile)
{
    std::ifstream in(indexFile, std::ios::binary);
    if (!in)
    {
        std::fprintf(stderr, "cannot open %s\n", indexFile.c_str());
        return false;
    }
    std::vector<char> bytes((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());

    CMemoryTextFileLoader loader;
    loader.Bind(static_cast<int>(bytes.size()), bytes.data());

    std::string const folder = packFolder + "/";
    CTokenVector tokens;
    for (DWORD i = 0; i < loader.GetLineCount() - 1; i++)
    {
        loader.SplitLineByTab(i, &tokens);
        if (tokens.size() != 2)
            return false;

        std::string const name = folder + tokens[0];
        manager.RegisterPack(name, tokens[1]);

        std::string const texCache = name + "_texcache";
        if (manager.PackExists(texCache, tokens[1]))
            manager.RegisterPack(texCache, tokens[1]);
    }
    return true;
}

void Put(const std::string &s)
{
    std::fwrite(s.data(), 1, s.size(), g_out);
}

void PutF(float f)
{
    uint32_t bits = 0;
    std::memcpy(&bits, &f, sizeof(f));
    std::fprintf(g_out, "\t%08x", bits);
}

// FNV-1a 64 over whatever is fed in, little endian.
struct Fnv
{
    uint64_t h = 0xcbf29ce484222325ULL;
    void Bytes(const void *data, size_t size)
    {
        const auto *p = static_cast<const unsigned char *>(data);
        for (size_t i = 0; i < size; ++i)
            h = (h ^ p[i]) * 0x100000001b3ULL;
    }
    void U8(uint8_t v)
    {
        Bytes(&v, 1);
    }
    void U32(uint32_t v)
    {
        Bytes(&v, 4);
    }
    void U64(uint64_t v)
    {
        Bytes(&v, 8);
    }
    void F(float f)
    {
        uint32_t bits = 0;
        std::memcpy(&bits, &f, 4);
        U32(bits);
    }
};

struct Rng
{
    uint64_t s;
    uint64_t Next()
    {
        s += 0x9E3779B97F4A7C15ULL;
        uint64_t z = s;
        z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
        z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
        return z ^ (z >> 31);
    }
};

struct DumpTerrain : CTerrain
{
    using CTerrainImpl::m_lpAlphaTexture;
};

struct DumpArea : CArea
{
    using CArea::__Load_LoadAmbience;
    using CArea::__Load_LoadObject;
    using CArea::m_ObjectDataVector;
    using CArea::ObjectDataComp;
};

struct DumpPropertyManager : CPropertyManager
{
    using CPropertyManager::m_PropertyByCRCMap;
};

struct DumpMap : CMapOutdoor
{
    using CMapOutdoor::AssignTerrainPtr;
    using CMapOutdoor::CreateTerrainPatchProxyList;
    using CMapOutdoor::ConvertToMapCoords;
    using CMapOutdoor::LoadTerrain;
    using CMapOutdoor::m_CurCoordinate;
    using CMapOutdoor::m_fTerrainTexCoordBase;
    using CMapOutdoor::m_IndexBuffer;
    using CMapOutdoor::m_lCurCoordStartX;
    using CMapOutdoor::m_lCurCoordStartY;
    using CMapOutdoor::m_matSplatAlpha;
    using CMapOutdoor::m_pTerrain;
    using CMapOutdoor::m_PrevCoordinate;
    using CMapOutdoor::m_sTerrainCountX;
    using CMapOutdoor::m_sTerrainCountY;
    using CMapOutdoor::m_TerrainVector;
    using CMapOutdoor::m_TextureSet;
    using CMapOutdoor::m_wNumIndices;
    using CGraphicBase::ms_lpd3dDevice;

    // The first block of CMapOutdoor::Update.
    bool MoveTo(float fX, float fY)
    {
        int ix = 0;
        int iy = 0;
        PR_FLOAT_TO_INT(fX, ix);
        if (fY < 0)
            fY = -fY;
        PR_FLOAT_TO_INT(fY, iy);

        short const sCoordX = MINMAX(0, ix / CTerrainImpl::TERRAIN_XSIZE, m_sTerrainCountX - 1);
        short const sCoordY = MINMAX(0, iy / CTerrainImpl::TERRAIN_YSIZE, m_sTerrainCountY - 1);

        bool const bNeedInit = (m_PrevCoordinate.m_sTerrainCoordX == -1 || m_PrevCoordinate.m_sTerrainCoordY == -1);
        if (!(bNeedInit || (m_CurCoordinate.m_sTerrainCoordX / LOAD_SIZE_WIDTH) != (sCoordX / LOAD_SIZE_WIDTH) ||
              (m_CurCoordinate.m_sTerrainCoordY / LOAD_SIZE_WIDTH) != (sCoordY / LOAD_SIZE_WIDTH)))
            return false;

        if (bNeedInit)
        {
            m_PrevCoordinate.m_sTerrainCoordX = sCoordX;
            m_PrevCoordinate.m_sTerrainCoordY = sCoordY;
        }
        else
        {
            m_PrevCoordinate.m_sTerrainCoordX = m_CurCoordinate.m_sTerrainCoordX;
            m_PrevCoordinate.m_sTerrainCoordY = m_CurCoordinate.m_sTerrainCoordY;
        }
        m_CurCoordinate.m_sTerrainCoordX = sCoordX;
        m_CurCoordinate.m_sTerrainCoordY = sCoordY;
        m_lCurCoordStartX = sCoordX * CTerrainImpl::TERRAIN_XSIZE;
        m_lCurCoordStartY = sCoordY * CTerrainImpl::TERRAIN_YSIZE;

        short const minX = std::max(m_CurCoordinate.m_sTerrainCoordX - LOAD_SIZE_WIDTH, 0);
        short const maxX = std::min<int>(m_CurCoordinate.m_sTerrainCoordX + LOAD_SIZE_WIDTH, m_sTerrainCountX - 1);
        short const minY = std::max(m_CurCoordinate.m_sTerrainCoordY - LOAD_SIZE_WIDTH, 0);
        short const maxY = std::min<int>(m_CurCoordinate.m_sTerrainCoordY + LOAD_SIZE_WIDTH, m_sTerrainCountY - 1);
        for (WORD usY = minY; usY <= maxY; ++usY)
            for (WORD usX = minX; usX <= maxX; ++usX)
                LoadTerrain(usX, usY, 0, 0);
        AssignTerrainPtr();
        return true;
    }

    CTerrain *FindTerrain(WORD x, WORD y)
    {
        for (auto *t : m_TerrainVector)
        {
            WORD cx = 0;
            WORD cy = 0;
            t->GetCoordinate(&cx, &cy);
            if (cx == x && cy == y)
                return t;
        }
        return nullptr;
    }

    // __SoftwareTransformPatch_SetTransform on a patch with an identity
    // transform: the texture coordinates are what matters.
    void HashUV(CTerrain *t, Fnv &h)
    {
        WORD cx = 0;
        WORD cy = 0;
        t->GetCoordinate(&cx, &cy);
        for (int py = 0; py < CTerrainImpl::PATCH_YCOUNT; ++py)
            for (int px = 0; px < CTerrainImpl::PATCH_XCOUNT; ++px)
            {
                CTerrainPatch *patch = t->GetTerrainPatchPtr(px, py);
                CTerrainPatchProxy proxy;
                proxy.SetTerrainPatch(patch);
                SoftwareTransformPatch_SRenderState rk{};
                D3DXMatrixIdentity(&rk.m_m4Frustum);
                rk.m_fScreenHalfWidth = 512.0f;
                rk.m_fScreenHalfHeight = -384.0f;
                SoftwareTransformPatch_STLVertex tv[CTerrainPatch::TERRAIN_VERTEX_COUNT];
                if (!__SoftwareTransformPatch_SetTransform(rk, tv, proxy, cx, cy, false, false))
                {
                    h.U8(0xEE);
                    continue;
                }
                for (auto &v : tv)
                {
                    h.F(v.kTexTile.x);
                    h.F(v.kTexTile.y);
                    h.F(v.kTexAlpha.x);
                    h.F(v.kTexAlpha.y);
                    h.F(v.kTexStaticShadow.x);
                    h.F(v.kTexStaticShadow.y);
                }
            }
    }
};

struct FsPoint
{
    float x;
    float y;
};

// The query point sets; the Rust side generates the same ones.
std::vector<FsPoint> GenPoints(int64_t ox, int64_t oy, Rng &rng)
{
    std::vector<FsPoint> pts;
    for (int j = 0; j < 105; ++j)
        for (int i = 0; i < 105; ++i)
            pts.push_back({static_cast<float>(ox - 28800 + 800 * i), static_cast<float>(oy - 28800 + 800 * j)});

    std::vector<float> bx;
    std::vector<float> by;
    for (int k = 0; k < 27; ++k)
        for (int d = -1; d <= 1; ++d)
        {
            bx.push_back(static_cast<float>(ox - 28800 + 3200 * k + d));
            by.push_back(static_cast<float>(oy - 28800 + 3200 * k + d));
        }
    for (float x : bx)
        for (float y : by)
            pts.push_back({x, y});

    for (int n = 0; n < 4000; ++n)
    {
        uint64_t const kind = rng.Next() % 4;
        if (kind == 0 || kind == 2)
        {
            float x = static_cast<float>(ox - 60000) + static_cast<float>(rng.Next() % 2720000) / 16.0f;
            float y = static_cast<float>(oy - 60000) + static_cast<float>(rng.Next() % 2720000) / 16.0f;
            if (kind == 0 ? (rng.Next() & 1) != 0 : true)
                y = -y;
            pts.push_back({x, y});
        }
        else if (kind == 1)
        {
            int64_t const ax = static_cast<int64_t>(rng.Next() % 384);
            int64_t const bx2 = static_cast<int64_t>(rng.Next() % 3) - 1;
            int64_t const ay = static_cast<int64_t>(rng.Next() % 384);
            int64_t const by2 = static_cast<int64_t>(rng.Next() % 3) - 1;
            bool const flip = (rng.Next() & 1) != 0;
            int64_t const cx = ox + ax * 200 - 12800 + bx2;
            int64_t const cy = oy + ay * 200 - 12800 + by2;
            float const y = static_cast<float>(cy);
            pts.push_back({static_cast<float>(cx), flip ? -y : y});
        }
        else
        {
            float const x = static_cast<float>(static_cast<int64_t>(rng.Next() % 40000000) - 10000000);
            float const y = static_cast<float>(static_cast<int64_t>(rng.Next() % 40000000) - 10000000);
            pts.push_back({x, y});
        }
    }

    const float specials[] = {0.0f,  -0.0f,        1e-30f,         25599.99f, 25600.0f, -1.0f,      1e10f,
                              -1e10f, 4294967296.0f, 3e9f, std::nanf(""), INFINITY, -INFINITY};
    for (float x : specials)
        for (float y : specials)
            pts.push_back({x, y});
    return pts;
}

struct Acc
{
    Fnv h;
    uint64_t count = 0;
    uint64_t hits = 0;
};

void Line(const char *kind, const Acc &a)
{
    std::fprintf(g_out, "Q\t%s\t%016llx\t%llu\t%llu\n", kind, static_cast<unsigned long long>(a.h.h),
                 static_cast<unsigned long long>(a.count), static_cast<unsigned long long>(a.hits));
}

void RunQueries(DumpMap &map, int posIndex)
{
    int64_t const ox = static_cast<int64_t>(map.m_CurCoordinate.m_sTerrainCoordX) * 25600;
    int64_t const oy = static_cast<int64_t>(map.m_CurCoordinate.m_sTerrainCoordY) * 25600;
    Rng rng{((static_cast<uint64_t>(map.m_CurCoordinate.m_sTerrainCoordX) << 32) |
             static_cast<uint32_t>(map.m_CurCoordinate.m_sTerrainCoordY)) *
                0x9E3779B97F4A7C15ULL +
            static_cast<uint64_t>(posIndex)};
    auto pts = GenPoints(ox, oy, rng);

    Acc height;
    Acc normal;
    Acc water;
    Acc attr;
    Acc getattr;
    Acc shadow;
    Acc num;
    Acc coords;
    const BYTE flags[] = {1, 2, 4, 3, 8, 16, 32, 48, 255};
    for (auto const &p : pts)
    {
        float const fx = p.x;
        float const fy = p.y;
        int ix = 0;
        int iy = 0;
        PR_FLOAT_TO_INT(fx, ix);
        PR_FLOAT_TO_INT(fy, iy);

        float const th = map.GetTerrainHeight(fx, fy);
        height.h.F(th);
        ++height.count;
        height.hits += th != 0.0f;

        D3DXVECTOR3 n(0, 0, 0);
        bool const nok = map.GetNormal(ix, iy, &n);
        normal.h.U8(nok);
        if (nok)
        {
            normal.h.F(n.x);
            normal.h.F(n.y);
            normal.h.F(n.z);
        }
        ++normal.count;
        normal.hits += nok;

        long wh = 0;
        bool const wok = map.GetWaterHeight(ix, iy, &wh);
        water.h.U8(wok);
        if (wok)
            water.h.U64(static_cast<uint64_t>(wh));
        ++water.count;
        water.hits += wok;

        unsigned bits = 0;
        for (size_t k = 0; k < sizeof(flags); ++k)
            bits |= (map.isAttrOn(fx, fy, flags[k]) ? 1u : 0u) << k;
        attr.h.U32(bits);
        ++attr.count;
        attr.hits += bits != 0;

        BYTE a = 0;
        bool const aok = map.GetAttr(fx, fy, &a);
        getattr.h.U8(aok);
        getattr.h.U8(a);
        ++getattr.count;
        getattr.hits += aok;

        DWORD const sc = map.GetShadowMapColor(fx, fy);
        shadow.h.U32(sc);
        ++shadow.count;
        shadow.hits += sc != 0xFFFFFFFFu;

        BYTE tn = 0;
        bool const tok = map.GetTerrainNum(fx, fy, &tn);
        num.h.U8(tok);
        num.h.U8(tn);
        ++num.count;
        num.hits += tok;

        if (std::isfinite(fx) && std::isfinite(fy) && std::fabs(fx) < 1e8f && std::fabs(fy) < 1e8f)
        {
            int cellX = 0;
            int cellY = 0;
            BYTE subX = 0;
            BYTE subY = 0;
            WORD tx = 0;
            WORD ty = 0;
            map.ConvertToMapCoords(fx, fy, &cellX, &cellY, &subX, &subY, &tx, &ty);
            coords.h.U32(static_cast<uint32_t>(cellX));
            coords.h.U32(static_cast<uint32_t>(cellY));
            coords.h.U8(subX);
            coords.h.U8(subY);
            coords.h.U32(tx);
            coords.h.U32(ty);
            ++coords.count;
        }
    }

    Acc pick;
    for (int n = 0; n < 40; ++n)
    {
        float const sx = static_cast<float>(ox - 25600) + static_cast<float>(rng.Next() % 76800);
        float sy = static_cast<float>(oy - 25600) + static_cast<float>(rng.Next() % 76800);
        if (rng.Next() & 1)
            sy = -sy;
        float const sz = static_cast<float>(rng.Next() % 8000);
        float const dx = static_cast<float>(static_cast<int>(rng.Next() % 2001) - 1000) / 1000.0f;
        float const dy = static_cast<float>(static_cast<int>(rng.Next() % 2001) - 1000) / 1000.0f;
        float const dz = -(static_cast<float>(static_cast<int>(rng.Next() % 951) + 50) / 1000.0f);
        float const range = static_cast<float>(1000 + rng.Next() % 39000);
        CRay ray(D3DXVECTOR3(sx, sy, sz), D3DXVECTOR3(dx, dy, dz), range);
        D3DXVECTOR3 hit(0, 0, 0);
        bool const ok = map.GetPickingPointWithRayOnlyTerrain(ray, &hit);
        pick.h.U8(ok);
        if (ok)
        {
            pick.h.F(hit.x);
            pick.h.F(hit.y);
            pick.h.F(hit.z);
        }
        ++pick.count;
        pick.hits += ok;
    }

    std::fprintf(g_out, "W\t%d\t%d\t%d\t", posIndex, map.m_CurCoordinate.m_sTerrainCoordX,
                 map.m_CurCoordinate.m_sTerrainCoordY);
    for (int i = 0; i < 9; ++i)
        std::fputc(map.m_pTerrain[i] ? '1' : '0', g_out);
    std::fprintf(g_out, "\t%zu\n", pts.size());
    Line("height", height);
    Line("normal", normal);
    Line("water", water);
    Line("attr", attr);
    Line("getattr", getattr);
    Line("shadow", shadow);
    Line("terrainnum", num);
    Line("mapcoords", coords);
    Line("pick", pick);
}

void DumpTerrainMesh(DumpMap &map, CTerrain *t, const std::string &mapDir, unsigned long id, size_t textureCount)
{
    for (int py = 0; py < CTerrainImpl::PATCH_YCOUNT; ++py)
        for (int px = 0; px < CTerrainImpl::PATCH_XCOUNT; ++px)
        {
            CTerrainPatch *p = t->GetTerrainPatchPtr(px, py);
            Fnv hv;
            hv.Bytes(p->SoftwareTransformPatch_GetTerrainVertexDataPtr(),
                     sizeof(SoftwareTransformPatch_SSourceVertex) * CTerrainPatch::TERRAIN_VERTEX_COUNT);
            Fnv hw;
            unsigned waterCount = 0;
            if (p->IsWaterExist())
            {
                CGraphicVertexBuffer *vb = p->GetWaterVertexBufferPointer();
                void *data = nullptr;
                waterCount = p->GetWaterFaceCount() * 3;
                if (vb->Lock(&data))
                {
                    hw.Bytes(data, sizeof(SWaterVertex) * waterCount);
                    vb->Unlock();
                }
            }
            std::fprintf(g_out, "PA\t%d\t%d\t%d\t%d", px, py, p->GetType(), p->IsWaterExist() ? 1 : 0);
            PutF(p->GetMinX());
            PutF(p->GetMaxX());
            PutF(p->GetMinY());
            PutF(p->GetMaxY());
            PutF(p->GetMinZ());
            PutF(p->GetMaxZ());
            std::fprintf(g_out, "\t%016llx\t%u\t%016llx\n", static_cast<unsigned long long>(hv.h), waterCount,
                         static_cast<unsigned long long>(hw.h));
        }

    // Splat alpha maps.
    auto *dt = static_cast<DumpTerrain *>(t);
    for (size_t i = 1; i < textureCount && i < MAXTERRAINTEXTURES; ++i)
    {
        TTerainSplat const &splat = t->GetTerrainSplatPatch().Splats[i];
        if (!splat.Active || !dt->m_lpAlphaTexture[i])
            continue;
        D3DLOCKED_RECT r;
        dt->m_lpAlphaTexture[i]->LockRect(0, &r, nullptr, 0);
        const auto *bits = static_cast<const BYTE *>(r.pBits);
        std::vector<BYTE> alpha(256 * 256);
        bool ok = r.Pitch == 256 * 4;
        for (int k = 0; k < 256 * 256; ++k)
        {
            alpha[k] = bits[k * 4 + 3];
            ok = ok && bits[k * 4] == 0 && bits[k * 4 + 1] == 0 && bits[k * 4 + 2] == 0;
        }
        Fnv h0;
        h0.Bytes(alpha.data(), alpha.size());
        std::fprintf(g_out, "SP\t%zu\t%d\t%016llx\t%d", i, splat.Active ? 1 : 0, static_cast<unsigned long long>(h0.h),
                     ok ? 1 : 0);
        // The mip loop of AddTexture32.
        std::vector<BYTE> src = alpha;
        UINT size = 256;
        for (int level = 1; level < 5; ++level)
        {
            std::vector<BYTE> dst;
            for (UINT y = 0; y != size; y += 2)
                for (UINT x = 0; x != size; x += 2)
                    dst.push_back((src[y * size + x] + src[y * size + x + 1] + src[(y + 1) * size + x] +
                                   src[(y + 1) * size + x + 1]) >>
                                  2);
            size >>= 1;
            src = dst;
            Fnv h;
            h.Bytes(src.data(), src.size());
            std::fprintf(g_out, "\t%016llx", static_cast<unsigned long long>(h.h));
        }
        std::fputc('\n', g_out);
    }

    // Lighting: several lights and materials, each with a new version.
    struct Cfg
    {
        float dir[3];
        float amb[3];
        float dif[3];
        float mamb[3];
        float mdif[3];
        float memi[3];
    };
    const Cfg cfgs[] = {
        {{0.3f, -0.5f, -0.8f}, {0.5f, 0.5f, 0.5f}, {0.8f, 0.8f, 0.8f}, {1, 1, 1}, {1, 1, 1}, {0, 0, 0}},
        {{-0.6f, 0.2f, 0.7f}, {0.9f, 0.9f, 0.9f}, {0.9f, 0.9f, 0.9f}, {0.9f, 0.9f, 0.9f}, {1, 1, 1}, {0, 0, 0}},
        {{0.1f, 0.9f, -0.4f}, {0.2f, 0.3f, 0.7f}, {0.7f, 0.4f, 0.1f}, {0.8f, 0.6f, 0.4f}, {0.5f, 0.9f, 0.7f},
         {0.05f, 0.0f, 0.1f}},
        {{0.0f, 0.0f, -1.0f}, {1.2f, 1.1f, 0.3f}, {0.9f, 0.9f, 0.9f}, {1, 1, 1}, {1, 1, 1}, {0, 0.2f, 0}},
    };
    WORD tx = 0;
    WORD ty = 0;
    t->GetCoordinate(&tx, &ty);
    {
        Fnv h;
        map.HashUV(t, h);
        std::fprintf(g_out, "UV\t%016llx\n", static_cast<unsigned long long>(h.h));
    }
    DWORD version = 1;
    for (size_t c = 0; c < sizeof(cfgs) / sizeof(cfgs[0]); ++c, ++version)
    {
        D3DLIGHT9 light{};
        light.Direction = D3DXVECTOR3(cfgs[c].dir[0], cfgs[c].dir[1], cfgs[c].dir[2]);
        light.Ambient = {cfgs[c].amb[0], cfgs[c].amb[1], cfgs[c].amb[2], 1.0f};
        light.Diffuse = {cfgs[c].dif[0], cfgs[c].dif[1], cfgs[c].dif[2], 1.0f};
        D3DMATERIAL9 mtrl{};
        mtrl.Ambient = {cfgs[c].mamb[0], cfgs[c].mamb[1], cfgs[c].mamb[2], 1.0f};
        mtrl.Diffuse = {cfgs[c].mdif[0], cfgs[c].mdif[1], cfgs[c].mdif[2], 1.0f};
        mtrl.Emissive = {cfgs[c].memi[0], cfgs[c].memi[1], cfgs[c].memi[2], 0.0f};
        Fnv h;
        for (int py = 0; py < CTerrainImpl::PATCH_YCOUNT; ++py)
            for (int px = 0; px < CTerrainImpl::PATCH_XCOUNT; ++px)
            {
                CTerrainPatch *p = t->GetTerrainPatchPtr(px, py);
                p->SoftwareTransformPatch_UpdateTerrainLighting(version, light, mtrl);
                auto *v = p->SoftwareTransformPatch_GetTerrainVertexDataPtr();
                for (int k = 0; k < CTerrainPatch::TERRAIN_VERTEX_COUNT; ++k)
                    h.U32(v[k].dwDiffuse);
            }
        std::fprintf(g_out, "LT\t%zu\t%016llx\n", c, static_cast<unsigned long long>(h.h));
    }
    (void)mapDir;
    (void)id;
}

// CArea::__Load_LoadObject/Ambience + the decisions of __SetObjectInstance.
void DumpAreaObjects(CEterPackManager &manager, const std::string &mapDir, WORD x, WORD y)
{
    unsigned long const ulID = (static_cast<unsigned long>(x) * 1000L) + static_cast<unsigned long>(y);
    char szAreaPathName[64 + 1];
    _snprintf(szAreaPathName, sizeof(szAreaPathName), "%s\\%06u\\", mapDir.c_str(), ulID);
    auto *area = new DumpArea; // leaked: ~CArea needs the effect manager
    std::string const objectFile = szAreaPathName + std::string("AreaData.txt");
    std::string const ambienceFile = szAreaPathName + std::string("AreaAmbienceData.txt");
    area->__Load_LoadObject(objectFile.c_str());
    area->__Load_LoadAmbience(ambienceFile.c_str());
    auto &v = area->m_ObjectDataVector;
    std::ranges::sort(v, DumpArea::ObjectDataComp());

    std::vector<std::string> lines;
    for (auto const &o : v)
    {
        CProperty *pProperty = nullptr;
        if (!CPropertyManager::Instance().Get(o.dwCRC, &pProperty))
            continue;
        const char *type = nullptr;
        if (!pProperty->GetString("PropertyType", &type))
            continue;

        char buf[1024];
        auto hex = [](float f) {
            uint32_t b = 0;
            std::memcpy(&b, &f, 4);
            char s[16];
            std::snprintf(s, sizeof(s), "%08x", b);
            return std::string(s);
        };
        float const px = o.Position.x;
        float const py = o.Position.y;
        float const pz = o.Position.z + o.m_fHeightBias;
        std::string head;
        std::snprintf(buf, sizeof(buf), "%u", o.dwCRC);
        head += buf;
        head += "\t" + hex(px) + "\t" + hex(py) + "\t" + hex(pz) + "\t" + hex(o.m_fYaw) + "\t" + hex(o.m_fPitch) + "\t" +
                hex(o.m_fRoll);
        for (BYTE const id : o.abyPortalID)
        {
            std::snprintf(buf, sizeof(buf), "\t%u", id);
            head += buf;
        }
        std::snprintf(buf, sizeof(buf), "\t%u\t", o.dwRange);
        head += buf;
        head += hex(o.fMaxVolumeAreaPercentage);

        std::string line;
        switch (prt::GetPropertyType(type))
        {
        case prt::PROPERTY_TYPE_TREE: {
            const char *tree = nullptr;
            if (!pProperty->GetString("TreeFile", &tree))
                continue;
            line = "tree\t" + head + "\t" + tree;
            break;
        }
        case prt::PROPERTY_TYPE_BUILDING: {
            prt::TPropertyBuilding d;
            if (!prt::PropertyBuildingStringToData(pProperty, &d))
                continue;
            std::vector<std::string> lods;
            for (UINT i = 1; i <= 3; ++i)
            {
                char end[256];
                sprintf(end, "_lod_%.2d.gr2", i);
                std::string const name = CFileNameHelper::NoExtension(d.strFileName) + end;
                if (!CResourceManager::Instance().IsFileExist(name.c_str()))
                    break;
                lods.push_back(name);
            }
            line = "building\t" + head + "\t" + d.strFileName + "\t" + d.strAttributeDataFileName + "\t" +
                   (CResourceManager::Instance().IsFileExist(d.strAttributeDataFileName.c_str()) ? "1" : "0") + "\t" +
                   std::to_string(d.isShadowFlag) + "\t" + std::to_string(lods.size());
            for (auto const &l : lods)
                line += "\t" + l;
            break;
        }
        case prt::PROPERTY_TYPE_EFFECT: {
            prt::TPropertyEffect d;
            if (!prt::PropertyEffectStringToData(pProperty, &d))
                continue;
            D3DXMATRIX mat;
            D3DXMatrixRotationYawPitchRoll(&mat, D3DXToRadian(o.m_fYaw), D3DXToRadian(o.m_fPitch),
                                           D3DXToRadian(o.m_fRoll));
            mat._41 = o.Position.x;
            mat._42 = o.Position.y;
            mat._43 = o.Position.z + o.m_fHeightBias;
            line = "effect\t" + head + "\t" + d.strFileName + "\t" +
                   std::to_string(GetCaseCRC32(d.strFileName.c_str(), d.strFileName.size()));
            const float *f = &mat._11;
            for (int i = 0; i < 16; ++i)
                line += "\t" + hex(f[i]);
            break;
        }
        case prt::PROPERTY_TYPE_AMBIENCE: {
            prt::TPropertyAmbience d;
            if (!prt::PropertyAmbienceStringToData(pProperty, &d))
                continue;
            int update = 3;
            if (d.strPlayType == "ONCE")
                update = 0;
            else if (d.strPlayType == "STEP")
                update = 1;
            else if (d.strPlayType == "LOOP")
                update = 2;
            line = "ambience\t" + head + "\t" + d.strName + "\t" + d.strPlayType + "\t" + std::to_string(update) + "\t" +
                   hex(d.fPlayInterval) + "\t" + hex(d.fPlayIntervalVariation) + "\t" + hex(d.fMaxVolumeAreaPercentage) +
                   "\t" + std::to_string(d.AmbienceSoundVector.size());
            for (auto const &s : d.AmbienceSoundVector)
                line += "\t" + s;
            break;
        }
        case prt::PROPERTY_TYPE_DUNGEON_BLOCK: {
            prt::TPropertyDungeonBlock d;
            if (!prt::PropertyDungeonBlockStringToData(pProperty, &d))
                continue;
            line = "dungeon\t" + head + "\t" + d.strFileName + "\t" + d.strAttributeDataFileName + "\t" +
                   (CResourceManager::Instance().IsFileExist(d.strAttributeDataFileName.c_str()) ? "1" : "0");
            break;
        }
        default:
            continue;
        }
        lines.push_back(line);
    }
    (void)manager;
    std::fprintf(g_out, "AN\t%zu\n", lines.size());
    for (auto const &l : lines)
    {
        std::fputs("AP\t", g_out);
        Put(l);
        std::fputc('\n', g_out);
    }
}

void DumpMapDir(CEterPackManager &manager, const std::string &name, const std::set<std::string> &files)
{
    std::fputs("M\t", g_out);
    Put(name);
    std::fputc('\n', g_out);

    auto *map = new DumpMap; // leaked: ~CMapOutdoor tears down what this tool never created
    map->SetName(name);
    map->LoadProperty();
    std::string const dir = map->GetMapDataDirectory();
    std::string const settingFile = dir + "\\Setting.txt";
    bool const settingOk = map->LoadSetting(settingFile.c_str());
    short cx = 0;
    short cy = 0;
    map->GetTerrainCount(&cx, &cy);
    size_t const textureCount = map->m_TextureSet.GetTextureCount();
    std::fprintf(g_out, "MS\t%d\t%d\t%d\t%ld", settingOk ? 1 : 0, cx, cy, map->GetViewRadius());
    PutF(map->GetHeightScale());
    PutF(map->GetOpaqueWaterDepth());
    std::fprintf(g_out, "\t%zu\n", textureCount);
    if (!settingOk)
        return;

    map->CreateTerrainPatchProxyList();
    for (int lod = 0; lod < TERRAINPATCH_LODMAX; ++lod)
    {
        void *data = nullptr;
        map->m_IndexBuffer[lod].Lock(&data);
        Fnv h;
        h.Bytes(data, sizeof(WORD) * map->m_wNumIndices[lod]);
        map->m_IndexBuffer[lod].Unlock();
        WORD prims = lod == 0 ? map->m_wNumIndices[lod] - 2 : map->m_wNumIndices[lod] / 3;
        std::fprintf(g_out, "IB\t%d\t%u\t%016llx\t%u\t%d\n", lod, map->m_wNumIndices[lod],
                     static_cast<unsigned long long>(h.h), prims, lod == 0 ? 1 : 0);
    }

    std::string prefix = dir + "/";
    std::ranges::transform(prefix, prefix.begin(), [](unsigned char c) { return c == '\\' ? '/' : std::tolower(c); });
    std::set<unsigned long> ids;
    for (auto it = files.lower_bound(prefix); it != files.end() && it->starts_with(prefix); ++it)
    {
        std::string const rest = it->substr(prefix.size());
        if (rest.size() > 7 && rest[6] == '/' &&
            std::all_of(rest.begin(), rest.begin() + 6, [](char c) { return c >= '0' && c <= '9'; }))
            ids.insert(std::stoul(rest.substr(0, 6)));
    }
    for (unsigned long const id : ids)
    {
        auto const x = static_cast<WORD>(id / 1000);
        auto const y = static_cast<WORD>(id % 1000);
        bool const ok = map->LoadTerrain(x, y, 0, 0);
        CTerrain *t = ok ? map->FindTerrain(x, y) : nullptr;
        std::fprintf(g_out, "T\t%06lu\t%d\n", id, t ? 1 : 0);
        if (t)
            DumpTerrainMesh(*map, t, dir, id, textureCount);
        DumpAreaObjects(manager, dir, x, y);
    }

    // Player positions: the middle of every terrain, then a few outside.
    struct Pos
    {
        float x;
        float y;
    };
    std::vector<Pos> positions;
    if (static_cast<int>(cx) * cy <= 64)
    {
        for (int j = 0; j < cy; ++j)
            for (int i = 0; i < cx; ++i)
                positions.push_back({static_cast<float>(i * 25600 + 12800),
                                     static_cast<float>(j * 25600 + 12800) * (((i + j) & 1) ? -1.0f : 1.0f)});
    }
    else
    {
        int n = 0;
        for (unsigned long const id : ids)
        {
            float const px = static_cast<float>((id / 1000) * 25600 + 12800);
            float const py = static_cast<float>((id % 1000) * 25600 + 12800);
            positions.push_back({px, (n++ & 1) ? -py : py});
        }
    }
    positions.push_back({-5000.0f, -5000.0f});
    positions.push_back({static_cast<float>(cx) * 25600.0f + 5000.0f, static_cast<float>(cy) * 25600.0f + 5000.0f});
    positions.push_back({1e9f, -1e9f});
    int index = 0;
    for (auto const &p : positions)
    {
        map->MoveTo(p.x, p.y);
        RunQueries(*map, index++);
    }
}

std::string BaseName(const std::string &name)
{
    auto const slash = name.find_last_of('/');
    return slash == std::string::npos ? name : name.substr(slash + 1);
}

} // namespace

int main(int argc, char **argv)
{
    if (argc < 4)
    {
        std::fprintf(stderr, "usage: %s <client dir> <index file> maps\n", argv[0]);
        return 2;
    }

    std::fflush(stdout);
    g_out = fdopen(dup(STDOUT_FILENO), "w");
    // TraceError writes to stdout; the query loops trigger it millions of
    // times, so it goes to /dev/null unless M2_TRACE is set.
    if (std::getenv("M2_TRACE"))
        dup2(STDERR_FILENO, STDOUT_FILENO);
    else
    {
        int const null = open("/dev/null", O_WRONLY);
        dup2(null, STDOUT_FILENO);
    }

    std::filesystem::current_path(argv[1]);

    CEterPackManager manager;
    manager.SetSearchMode(true);
    if (!RegisterIndex(manager, "pack", argv[2]))
        return 1;

    CTimer timer;
    CResourceManager resources;
    for (const char *ext : {"dds", "jpg", "tga", "bmp"})
        resources.RegisterResourceNewFunctionPointer(ext, NewImage);
    DumpPropertyManager properties;
    properties.Initialize();

    DumpMap::ms_lpd3dDevice = new IDirect3DDevice9;

    std::set<std::string> files;
    for (auto const &entry : manager.GetFileMap())
        files.insert(entry.first);

    std::string const only = argc > 4 ? argv[4] : "";
    std::set<std::string> dirs;
    for (auto const &name : files)
    {
        std::string const base = BaseName(name);
        if ((base == "setting.txt" || base == "mapproperty.txt") && name.size() > base.size())
            dirs.insert(name.substr(0, name.size() - base.size() - 1));
    }
    for (auto const &dir : dirs)
        if (only.empty() || dir == only)
            DumpMapDir(manager, dir, files);
    std::fclose(g_out);
    std::_Exit(0);
}
