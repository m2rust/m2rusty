// Reference dump for the Rust effect runtime (crates/m2-effect).
//
//   m2diff_effect_runtime <client dir> <index file> summary
//   m2diff_effect_runtime <client dir> <index file> full <effect name>...
//
// Runs every registered .mse (or the named ones) through the real
// CEffectInstance for fixed scenarios from a fixed random seed, and after each
// frame emits the state of every element: particles (physics state and the
// billboard corners the renderer would draw), effect meshes (frame controllers
// and the draw parameters of OnRender) and lights. All numbers are 32-bit
// words (floats as their bits). Every record is folded into an FNV-1a hash; in
// "full" mode it is also printed as a line of tab separated hex.
//
// Time comes from CTimer in custom mode: Advance() steps 17, 16, 17, 16 ms and
// Adjust(gap) adds an irregular gap before it.
//
// Textures are not created: GetResourcePointer returns null for them and the
// real CEffectMesh for .mde files, which are loaded from the pack.

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <deque>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <list>
#include <map>
#include <memory>
#include <set>
#include <sstream>
#include <string>
#include <vector>

#include <unistd.h>

#include <Platform/Platform.h>
#include <Platform/DirectXMathCompat.h>
#include <Neoeter/Platform/Base/Singleton.h>
#define protected public
#define private public
#include <Neoeter/Render/GrpLightManager.h>
#include <Neoeter/Render/GrpBase.h>
#include <Neoeter/Render/Camera.h>
#include <Neoeter/Render/CullingManager.h>
#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/FileLoader.h>
#include <Neoeter/Platform/Base/Timer.h>
#include <Neoeter/Platform/Base/Random.h>
#include <Neoeter/Render/ResourceManager.h>
#include <Neoeter/Render/TextFileLoader.h>
#include <Neoeter/Render/GrpMath.h>
#include <Neoeter/Effect/EffectData.h>
#include <Neoeter/Effect/EffectInstance.h>
#include <Neoeter/Effect/EffectManager.h>
#include <Neoeter/Effect/EffectMesh.h>
#include <Neoeter/Effect/ParticleSystemData.h>
#include <Neoeter/Effect/SimpleLightData.h>
#undef private
#undef protected

namespace
{

constexpr unsigned kSeed = 1234567;

FILE *g_out = nullptr;
bool g_text = false;
uint64_t g_hash = 0xcbf29ce484222325ull;

void HashByte(uint8_t b)
{
    g_hash ^= b;
    g_hash *= 0x100000001b3ull;
}

// One record: a tag, 32-bit words, end.
struct Rec
{
    explicit Rec(const char *tag)
    {
        for (const char *p = tag; *p; ++p)
            HashByte(static_cast<uint8_t>(*p));
        if (g_text)
            std::fputs(tag, g_out);
    }
    Rec &u(uint32_t v)
    {
        for (int i = 0; i < 4; ++i)
            HashByte(static_cast<uint8_t>(v >> (8 * i)));
        if (g_text)
            std::fprintf(g_out, "\t%08x", v);
        return *this;
    }
    Rec &f(float v)
    {
        uint32_t bits = 0;
        std::memcpy(&bits, &v, 4);
        return u(bits);
    }
    Rec &v3(const D3DXVECTOR3 &v)
    {
        return f(v.x).f(v.y).f(v.z);
    }
    Rec &m(const D3DXMATRIX &mat)
    {
        for (int r = 0; r < 4; ++r)
            for (int c = 0; c < 4; ++c)
                f(mat.m[r][c]);
        return *this;
    }
    ~Rec()
    {
        HashByte('\n');
        if (g_text)
            std::fputc('\n', g_out);
    }
};

std::map<std::string, CEffectMesh *> g_meshes;
size_t g_meshLoads = 0;

bool EndsWithMde(const char *name)
{
    size_t const n = std::strlen(name);
    if (n < 4)
        return false;
    std::string ext(name + n - 4);
    for (char &c : ext)
        if (c >= 'A' && c <= 'Z')
            c = static_cast<char>(c - 'A' + 'a');
    return ext == ".mde";
}

} // namespace

// --wrap=CResourceManager::GetResourcePointer(const char *)
extern "C" CResource *__wrap__ZN16CResourceManager18GetResourcePointerEPKc(CResourceManager *, const char *name)
{
    if (!name || !EndsWithMde(name))
        return nullptr;
    auto it = g_meshes.find(name);
    if (it != g_meshes.end())
        return it->second;
    auto *mesh = new CEffectMesh(name);
    mesh->Load();
    if (mesh->IsEmpty())
    {
        delete mesh;
        mesh = nullptr;
    }
    else
        ++g_meshLoads;
    g_meshes[name] = mesh;
    return mesh;
}

// --wrap=CStateManager::SetRenderState: nothing to set without a device.
extern "C" void __wrap__ZN13CStateManager14SetRenderStateE19_D3DRENDERSTATETYPEj(void *, int, unsigned)
{
}

namespace
{

bool RegisterIndex(CEterPackManager &manager, const std::string &packFolder, const std::string &indexFile)
{
    std::ifstream in(indexFile, std::ios::binary);
    if (!in)
        return false;
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

std::string Extension(const std::string &name)
{
    auto const dot = name.find_last_of('.');
    if (dot == std::string::npos)
        return {};
    std::string ext = name.substr(dot + 1);
    for (char &c : ext)
        if (c >= 'A' && c <= 'Z')
            c = static_cast<char>(c - 'A' + 'a');
    return ext;
}

CCamera *g_camera = nullptr;
D3DXMATRIX g_global;

// ----- snapshot ------------------------------------------------------------

constexpr float kD3DXPi = 3.141592654f;

void DumpMeshDraws(CEffectMeshInstance &mi, const D3DXMATRIX &local)
{
    // CEffectMeshInstance::OnRender without the device calls.
    if (!mi.isActive())
        return;
    CEffectMesh *pEffectMesh = mi.m_roMesh.GetPointer();
    for (DWORD i = 0; i < pEffectMesh->GetMeshCount(); ++i)
    {
        CFrameController const &tex = mi.m_TextureInstanceVector[i].TextureFrameController;
        if (!tex.isActive(mi.m_MeshFrameController.GetCurrentFrame()))
            continue;
        int const iBillboardType = mi.m_pMeshScript->GetBillboardType(i);
        D3DXMATRIX matWorld;
        D3DXMatrixIdentity(&matWorld);
        switch (iBillboardType)
        {
        case MESH_BILLBOARD_TYPE_ALL: {
            D3DXMATRIX matTemp;
            D3DXMatrixRotationX(&matTemp, 90.0f);
            D3DXMatrixInverse(&matWorld, nullptr, &CScreen::GetViewMatrix());
            matWorld = matTemp * matWorld;
        }
        break;
        case MESH_BILLBOARD_TYPE_Y: {
            D3DXMATRIX matTemp;
            D3DXMatrixIdentity(&matTemp);
            D3DXMatrixInverse(&matTemp, nullptr, &CScreen::GetViewMatrix());
            matWorld._11 = matTemp._11;
            matWorld._12 = matTemp._12;
            matWorld._21 = matTemp._21;
            matWorld._22 = matTemp._22;
        }
        break;
        case MESH_BILLBOARD_TYPE_MOVE: {
            D3DXVECTOR3 Position;
            mi.m_pMeshScript->GetPosition(mi.m_fLocalTime, Position);
            D3DXVECTOR3 LastPosition;
            mi.m_pMeshScript->GetPosition(mi.m_fLocalTime - CTimer::Instance().GetElapsedSecond(), LastPosition);
            Position -= LastPosition;
            if (D3DXVec3LengthSq(&Position) > 0.001f)
            {
                D3DXVec3Normalize(&Position, &Position);
                D3DXQUATERNION q = SafeRotationNormalizedArc(D3DXVECTOR3(0.0f, -1.0f, 0.0f), Position);
                D3DXMatrixRotationQuaternion(&matWorld, &q);
            }
        }
        break;
        }
        bool const blend = mi.m_pMeshScript->isBlendingEnable(i);
        int src = 0;
        int dst = 0;
        if (blend)
        {
            src = mi.m_pMeshScript->GetBlendingSrcType(i);
            dst = mi.m_pMeshScript->GetBlendingDestType(i);
        }
        D3DXVECTOR3 Position;
        mi.m_pMeshScript->GetPosition(mi.m_fLocalTime, Position);
        matWorld._41 = Position.x;
        matWorld._42 = Position.y;
        matWorld._43 = Position.z;
        matWorld = matWorld * local;

        BYTE byType = 0;
        D3DXCOLOR Color(1.0f, 1.0f, 1.0f, 1.0f);
        bool const hasOp = mi.m_pMeshScript->GetColorOperationType(i, &byType);
        mi.m_pMeshScript->GetColorFactor(i, &Color);
        TTimeEventTableFloat *TableAlpha = nullptr;
        float fAlpha = 1.0f;
        if (mi.m_pMeshScript->GetTimeTableAlphaPointer(i, &TableAlpha) && !TableAlpha->empty())
            GetTimeEventBlendValue(mi.m_fLocalTime, *TableAlpha, &fAlpha);

        CEffectMesh::TEffectMeshData *pMeshData = pEffectMesh->GetMeshDataPointer(i);
        BYTE const curFrame = mi.m_MeshFrameController.GetCurrentFrame();
        if (curFrame >= pMeshData->EffectFrameDataVector.size())
            continue;
        CEffectMesh::TEffectFrameData &rFrameData = pMeshData->EffectFrameDataVector[curFrame];
        DWORD const dwcurTextureFrame = tex.GetCurrentFrame();
        bool const hasTexture = dwcurTextureFrame < mi.m_TextureInstanceVector[i].TextureInstanceVector.size();
        Color.a = fAlpha * rFrameData.fVisibility;
        DWORD const color = DWORD(Color);

        uint64_t vh = 0xcbf29ce484222325ull;
        for (auto const &v : rFrameData.PDTVertexVector)
        {
            float const vals[5] = {v.position.x, v.position.y, v.position.z, v.texCoord.x, v.texCoord.y};
            for (float f : vals)
            {
                uint32_t bits = 0;
                std::memcpy(&bits, &f, 4);
                for (int k = 0; k < 4; ++k)
                {
                    vh ^= static_cast<uint8_t>(bits >> (8 * k));
                    vh *= 0x100000001b3ull;
                }
            }
        }
        Rec("MD")
            .u(i)
            .u(hasTexture ? 1 : 0)
            .u(hasTexture ? dwcurTextureFrame : 0)
            .m(matWorld)
            .u(blend ? 1 : 0)
            .u(src)
            .u(dst)
            .u(hasOp ? 1 : 0)
            .u(byType)
            .u(color)
            .u(static_cast<uint32_t>(rFrameData.PDTVertexVector.size()))
            .u(static_cast<uint32_t>(vh))
            .u(static_cast<uint32_t>(vh >> 32));
    }
}

void DumpQuad(const char *tag, CParticleInstance *p, uint32_t frame)
{
    TPTVertex *mesh = p->GetParticleMeshPointer();
    Rec r(tag);
    r.u(frame).u(p->m_dcColor.m_dwColor);
    for (int k = 0; k < 4; ++k)
        r.v3(mesh[k].position);
}

void Snapshot(CEffectInstance &inst, unsigned frameNo, unsigned ms, float renderElapsed, size_t *maxParticles)
{
    Rec("FR").u(frameNo).u(ms).f(renderElapsed).u(inst.m_isAlive ? 1 : 0);
    size_t total = 0;

    for (size_t s = 0; s < inst.m_ParticleInstanceVector.size(); ++s)
    {
        CParticleSystemInstance &ps = *inst.m_ParticleInstanceVector[s];
        Rec("PS")
            .u(static_cast<uint32_t>(s))
            .u(ps.m_dwCurrentEmissionCount)
            .f(ps.m_fLocalTime)
            .u(static_cast<uint32_t>(ps.m_iLoopCount))
            .f(ps.m_fEmissionResidue)
            .u(ps.m_bStart ? 1 : 0)
            .u(ps.m_isActive ? 1 : 0);
        CParticleProperty *props = ps.m_pParticleProperty;
        DWORD const frames = props->GetTextureAnimationFrameCount();
        for (DWORD f = 0; f < frames; ++f)
        {
            for (CParticleInstance *p : ps.m_ParticleInstanceListVector[f])
            {
                ++total;
                Rec("P")
                    .u(f)
                    .v3(p->m_v3Position)
                    .v3(p->m_v3LastPosition)
                    .v3(p->m_v3Velocity)
                    .v3(p->m_v3StartPosition)
                    .f(p->m_v2HalfSize.x)
                    .f(p->m_v2HalfSize.y)
                    .f(p->m_v2Scale.x)
                    .f(p->m_v2Scale.y)
                    .f(p->m_fRotation)
                    .u(p->m_dcColor.m_dwColor)
                    .u(p->m_byTextureAnimationType)
                    .u(p->m_byFrameIndex)
                    .f(p->m_fLifeTime)
                    .f(p->m_fLastLifeTime);
            }
        }
        // CParticleSystemInstance::OnRender (no frustum test).
        BYTE const type = props->m_byBillboardType;
        bool const attach = props->m_bAttachFlag != 0;
        const D3DXMATRIX *pmat = attach ? &inst.m_matGlobal : nullptr;
        for (DWORD f = 0; f < frames; ++f)
        {
            for (CParticleInstance *p : ps.m_ParticleInstanceListVector[f])
            {
                if (type < BILLBOARD_TYPE_2FACE)
                {
                    p->Transform(pmat);
                    DumpQuad("Q", p, f);
                }
                else if (type == BILLBOARD_TYPE_2FACE)
                {
                    p->Transform(pmat, D3DXToRadian(-30.0f));
                    DumpQuad("Q", p, f);
                    p->Transform(pmat, D3DXToRadian(+30.0f));
                    DumpQuad("Q", p, f);
                }
                else if (type == BILLBOARD_TYPE_3FACE)
                {
                    p->Transform(pmat);
                    DumpQuad("Q", p, f);
                    p->Transform(pmat, D3DXToRadian(-60.0f));
                    DumpQuad("Q", p, f);
                    p->Transform(pmat, D3DXToRadian(+60.0f));
                    DumpQuad("Q", p, f);
                }
            }
        }
    }
    *maxParticles = std::max(*maxParticles, total);

    for (size_t i = 0; i < inst.m_MeshInstanceVector.size(); ++i)
    {
        CEffectMeshInstance &mi = *inst.m_MeshInstanceVector[i];
        {
        Rec r("MI");
        r.u(static_cast<uint32_t>(i)).u(mi.m_bStart ? 1 : 0).u(mi.m_isActive ? 1 : 0).f(mi.m_fLocalTime);
        r.u(mi.m_roMesh.GetPointer() ? 1 : 0).u(mi.isActive() ? 1 : 0).u(mi.m_MeshFrameController.GetCurrentFrame());
        r.u(mi.m_MeshFrameController.isActive(0) ? 1 : 0);
        for (auto &t : mi.m_TextureInstanceVector)
        {
            r.u(t.TextureFrameController.GetCurrentFrame())
                .u(t.TextureFrameController.isActive(mi.m_MeshFrameController.GetCurrentFrame()) ? 1 : 0);
        }
        }
        if (mi.m_roMesh.GetPointer())
            DumpMeshDraws(mi, inst.m_matGlobal);
    }

    for (size_t i = 0; i < inst.m_LightInstanceVector.size(); ++i)
    {
        CLightInstance &li = *inst.m_LightInstanceVector[i];
        CLight *light = li.m_LightID ? CLightManager::Instance().GetLight(li.m_LightID) : nullptr;
        Rec r("LI");
        r.u(static_cast<uint32_t>(i)).u(li.m_bStart ? 1 : 0).u(li.m_isActive ? 1 : 0).f(li.m_fLocalTime);
        r.u(light ? 1 : 0);
        if (light)
        {
            D3DLIGHT9 const &l = light->m_d3dLight;
            r.f(l.Position.x).f(l.Position.y).f(l.Position.z).f(l.Range);
            r.f(l.Ambient.r).f(l.Ambient.g).f(l.Ambient.b).f(l.Ambient.a);
            r.f(l.Diffuse.r).f(l.Diffuse.g).f(l.Diffuse.b).f(l.Diffuse.a);
            r.f(l.Attenuation0).f(l.Attenuation1).f(l.Attenuation2);
        }
    }
}

// ----- scenarios -------------------------------------------------------------

struct Scenario
{
    const char *name;
    unsigned frames;
    bool irregular;
};

const Scenario kScenarios[] = {{"regular", 240, false}, {"irregular", 400, true}};

unsigned GapFor(const Scenario &sc, unsigned frame)
{
    if (!sc.irregular)
        return 0;
    static const unsigned table[8] = {0, 0, 3, 40, 1, 250, 7, 100};
    if (frame == 10)
        return 5000;
    return table[(frame * 7 + 3) % 8];
}

void RunScenario(const std::string &name, CEffectData *data, const Scenario &sc, bool text)
{
    g_text = text;
    g_hash = 0xcbf29ce484222325ull;
    if (text)
    {
        std::fputs("B\t", g_out);
        std::fputs(name.c_str(), g_out);
        std::fprintf(g_out, "\t%s\n", sc.name);
    }

    srandom(kSeed);
    auto timer = std::make_unique<CTimer>();
    timer->UseCustomTime();
    CLightManager::Instance().Initialize();

    auto *inst = new CEffectInstance();
    inst->SetEffectDataPointer(data);
    inst->SetGlobalMatrix(g_global);

    size_t maxParticles = 0;
    unsigned ms = 0;
    unsigned step = 17;
    for (unsigned frame = 0; frame < sc.frames; ++frame)
    {
        unsigned const gap = GapFor(sc, frame);
        timer->Adjust(static_cast<int>(gap));
        timer->Advance();
        ms += gap + step;
        float const renderElapsed = timer->GetElapsedSecond();
        if (sc.irregular)
        {
            if (frame == 160 || frame == 220)
                inst->SetDeactive();
            if (frame == 190)
                inst->SetActive();
            if (frame == 280)
            {
                inst->Clear();
                inst->SetEffectDataPointer(data);
                inst->SetGlobalMatrix(g_global);
            }
        }
        inst->Update();
        Snapshot(*inst, frame, ms, renderElapsed, &maxParticles);
        step = step == 17 ? 16 : 17;
    }
    uint64_t const hash = g_hash;
    g_text = false;
    std::fprintf(g_out, "S\t%s\t%s\t%016llx\t%u\t%zu\t%d\t%zu\t%zu\t%zu\n", name.c_str(), sc.name,
                 static_cast<unsigned long long>(hash), sc.frames, maxParticles, inst->m_isAlive ? 1 : 0,
                 inst->m_ParticleInstanceVector.size(), inst->m_MeshInstanceVector.size(),
                 inst->m_LightInstanceVector.size());
    inst->Clear();
    // Pooled elements keep stale state when a mesh is missing (a C++ bug);
    // start every run from empty pools so results do not depend on history.
    CEffectInstance::DestroySystem();
    // The instance object is leaked on purpose (culling handles point at it).
}

void PutHeader()
{
    g_text = false;
    CCamera *cam = g_camera;
    std::fprintf(g_out, "CAM");
    for (const D3DXVECTOR3 *v : {&cam->GetUp(), &cam->GetCross(), &cam->GetView()})
        for (float f : {v->x, v->y, v->z})
        {
            uint32_t bits = 0;
            std::memcpy(&bits, &f, 4);
            std::fprintf(g_out, "\t%08x", bits);
        }
    std::fputc('\n', g_out);
    auto putMat = [](const char *tag, const D3DXMATRIX &m) {
        std::fputs(tag, g_out);
        for (int r = 0; r < 4; ++r)
            for (int c = 0; c < 4; ++c)
            {
                uint32_t bits = 0;
                std::memcpy(&bits, &m.m[r][c], 4);
                std::fprintf(g_out, "\t%08x", bits);
            }
        std::fputc('\n', g_out);
    };
    putMat("VIEW", cam->GetViewMatrix());
    putMat("GLOBAL", g_global);
    srandom(kSeed);
    std::fputs("RAND", g_out);
    for (int i = 0; i < 8; ++i)
        std::fprintf(g_out, "\t%lu", static_cast<unsigned long>(random()));
    std::fputc('\n', g_out);
}

} // namespace

int main(int argc, char **argv)
{
    if (argc < 4)
    {
        std::fprintf(stderr, "usage: %s <client dir> <index file> <summary|full names...>\n", argv[0]);
        return 2;
    }
    std::fflush(stdout);
    g_out = fdopen(dup(STDOUT_FILENO), "w");
    dup2(STDERR_FILENO, STDOUT_FILENO);

    std::filesystem::current_path(argv[1]);
    bool const full = std::string(argv[3]) == "full";

    CEterPackManager manager;
    manager.SetSearchMode(true);
    if (!RegisterIndex(manager, "pack", argv[2]))
        return 1;

    CResourceManager resourceManager;
    CEffectManager effectManager;
    CCullingManager cullingManager;
    CLightManager lightManager;
    CCameraManager cameraManager;

    g_camera = cameraManager.GetCurrentCamera();
    g_camera->SetViewParams(D3DXVECTOR3(300.0f, -400.0f, 250.0f), D3DXVECTOR3(10.0f, 20.0f, 30.0f),
                            D3DXVECTOR3(0.0f, 0.0f, 1.0f));
    CGraphicBase::ms_matView = g_camera->GetViewMatrix();

    D3DXMatrixRotationYawPitchRoll(&g_global, D3DXToRadian(30.0f), D3DXToRadian(10.0f), D3DXToRadian(-20.0f));
    g_global._41 = 100.0f;
    g_global._42 = 200.0f;
    g_global._43 = 50.0f;

    PutHeader();

    std::vector<std::string> names;
    if (full)
    {
        for (int i = 4; i < argc; ++i)
            names.emplace_back(argv[i]);
    }
    else
    {
        for (auto const &entry : manager.GetFileMap())
            if (Extension(entry.first) == "mse")
                names.push_back(entry.first);
        std::ranges::sort(names);
    }

    for (auto const &name : names)
    {
        CEffectData *data = CEffectData::New();
        bool ok = false;
        try
        {
            ok = data->LoadScript(name.c_str());
        }
        catch (const std::exception &)
        {
            std::fprintf(g_out, "S\t%s\tload\tthrow\n", name.c_str());
            continue;
        }
        if (!ok)
        {
            std::fprintf(g_out, "S\t%s\tload\tfail\n", name.c_str());
            continue;
        }
        for (auto const &sc : kScenarios)
            RunScenario(name, data, sc, full);
    }
    std::fprintf(g_out, "MESHES\t%zu\n", g_meshLoads);
    std::fclose(g_out);
    std::_Exit(0);
}
