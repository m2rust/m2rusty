// Reference dump for the Rust effect data loaders (crates/m2-formats/src/effect).
//
//   m2diff_effect <client dir> <index file> <mse|msf|mde>
//
// Registers packs like m2diff_pack, then for every registered file with the
// given extension (sorted by name) loads it with the real loader and prints
// what the data object holds:
//
//   mse: CEffectData::LoadScript   (particles, meshes, lights, sounds)
//   msf: CFlyingData::LoadScriptFile
//   mde: CEffectMesh::Load         (the binary effect mesh, .ifl textures)
//
// Floats are printed as the hex of their bits. Texture requests
// (CResourceManager::GetResourcePointer) are wrapped at link time: they are
// recorded and printed as "R <name>" lines after each file, and return null.
// Files the C++ loader would read out of bounds on print "F <name> skip"
// (the Rust port rejects them). A loader throwing prints "F <name> throw".

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <map>
#include <set>
#include <sstream>
#include <string>
#include <vector>

#include <unistd.h>

// The dump needs the loaders' protected members.
#define protected public
#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Platform/Base/FileLoader.h>
#include <Neoeter/Render/ResourceManager.h>
#include <Neoeter/Render/TextFileLoader.h>
#include <Neoeter/Effect/EffectData.h>
#include <Neoeter/Effect/EffectManager.h>
#include <Neoeter/Effect/EffectMesh.h>
#include <Neoeter/Effect/ParticleSystemData.h>
#include <Neoeter/Effect/SimpleLightData.h>
#include <Client/Game/FlyingData.h>
#undef protected

namespace
{

FILE *g_out = nullptr;
std::vector<std::string> g_requests;

} // namespace

// --wrap=CResourceManager::GetResourcePointer(const char *)
extern "C" CResource *__wrap__ZN16CResourceManager18GetResourcePointerEPKc(CResourceManager *, const char *name)
{
    g_requests.emplace_back(name ? name : "");
    return nullptr;
}

namespace
{

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

void PutStr(const char *s)
{
    std::fputc('\t', g_out);
    std::fputs(s, g_out);
}

void PutStr(const std::string &s)
{
    std::fputc('\t', g_out);
    Put(s);
}

void PutF(float f)
{
    uint32_t bits = 0;
    std::memcpy(&bits, &f, sizeof(f));
    std::fprintf(g_out, "\t%08x", bits);
}

void PutI(long long v)
{
    std::fprintf(g_out, "\t%lld", v);
}

void PutU(unsigned long long v)
{
    std::fprintf(g_out, "\t%llu", v);
}

void PutV3(const D3DXVECTOR3 &v)
{
    PutF(v.x);
    PutF(v.y);
    PutF(v.z);
}

void PutColor(const D3DXCOLOR &c)
{
    PutF(c.r);
    PutF(c.g);
    PutF(c.b);
    PutF(c.a);
}

void End()
{
    std::fputc('\n', g_out);
}

void PutTable(const char *label, const TTimeEventTableFloat &table)
{
    std::fprintf(g_out, "T\t%s\t%zu", label, table.size());
    for (auto const &e : table)
    {
        PutF(e.m_fTime);
        PutF(e.m_Value);
    }
    End();
}

void PutColorTable(const char *label, const TTimeEventTableColor &table)
{
    std::fprintf(g_out, "T\t%s\t%zu", label, table.size());
    for (auto const &e : table)
    {
        PutF(e.m_fTime);
        std::fprintf(g_out, "\t%08x", static_cast<unsigned>(e.m_Value.m_dwColor));
    }
    End();
}

void PutRequests()
{
    for (auto const &r : g_requests)
    {
        std::fputs("R", g_out);
        PutStr(r);
        End();
    }
    g_requests.clear();
}

void PutElement(CEffectElementBase &e)
{
    std::fputs("E", g_out);
    PutF(e.m_fStartTime);
    PutU(e.m_TimeEventTablePosition.size());
    End();
    for (auto const &p : e.m_TimeEventTablePosition)
    {
        std::fputs("EP", g_out);
        PutF(p.m_fTime);
        PutI(p.m_iMovingType);
        PutV3(p.m_vecPosition);
        PutV3(p.m_vecControlPoint);
        End();
    }
}

void PutParticle(CParticleSystemData &d)
{
    CEmitterProperty const &e = d.m_EmitterProperty;
    std::fputs("PE", g_out);
    PutU(e.m_dwMaxEmissionCount);
    PutF(e.m_fCycleLength);
    PutI(e.m_bCycleLoopFlag);
    PutI(e.m_iLoopCount);
    PutU(e.m_byEmitterShape);
    PutU(e.m_byEmitterAdvancedType);
    PutI(e.m_bEmitFromEdgeFlag);
    PutV3(e.m_v3EmittingSize);
    PutF(e.m_fEmittingRadius);
    PutV3(e.m_v3EmittingDirection);
    End();
    PutTable("emittingsize", e.m_TimeEventEmittingSize);
    PutTable("emittingangularvelocity", e.m_TimeEventEmittingAngularVelocity);
    PutTable("emittingdirectionx", e.m_TimeEventEmittingDirectionX);
    PutTable("emittingdirectiony", e.m_TimeEventEmittingDirectionY);
    PutTable("emittingdirectionz", e.m_TimeEventEmittingDirectionZ);
    PutTable("emittingvelocity", e.m_TimeEventEmittingVelocity);
    PutTable("emissioncountpersecond", e.m_TimeEventEmissionCountPerSecond);
    PutTable("lifetime", e.m_TimeEventLifeTime);
    PutTable("sizex", e.m_TimeEventSizeX);
    PutTable("sizey", e.m_TimeEventSizeY);

    CParticleProperty const &p = d.m_ParticleProperty;
    std::fputs("PP", g_out);
    PutU(p.m_bySrcBlendType);
    PutU(p.m_byDestBlendType);
    PutU(p.m_byColorOperationType);
    PutU(p.m_byBillboardType);
    PutU(p.m_byRotationType);
    PutF(p.m_fRotationSpeed);
    PutU(p.m_wRotationRandomStartingBegin);
    PutU(p.m_wRotationRandomStartingEnd);
    PutI(p.m_bAttachFlag);
    PutI(p.m_bStretchFlag);
    PutU(p.m_byTexAniType);
    PutF(p.m_fTexAniDelay);
    PutI(p.m_bTexAniRandomStartFrameFlag);
    PutU(p.m_ImageVector.size());
    End();
    PutTable("gravity", p.m_TimeEventGravity);
    PutTable("airresistance", p.m_TimeEventAirResistance);
    PutTable("scalex", p.m_TimeEventScaleX);
    PutTable("scaley", p.m_TimeEventScaleY);
    PutColorTable("color", p.m_TimeEventColor);
    PutTable("rotation", p.m_TimeEventRotation);
}

void PutMeshScript(CEffectMeshScript &m)
{
    std::fputs("MS", g_out);
    PutStr(m.m_strMeshFileName);
    PutI(m.m_isMeshAnimationLoop);
    PutI(m.m_iMeshAnimationLoopCount);
    PutF(m.m_fMeshAnimationFrameDelay);
    PutU(m.m_MeshDataVector.size());
    End();
    for (auto const &d : m.m_MeshDataVector)
    {
        std::fputs("MD", g_out);
        PutU(d.byBillboardType);
        PutI(d.bBlendingEnable);
        PutU(d.byBlendingSrcType);
        PutU(d.byBlendingDestType);
        PutI(d.bTextureAlphaEnable);
        PutU(d.byColorOperationType);
        PutColor(d.ColorFactor);
        PutI(d.bTextureAnimationLoopEnable);
        PutF(d.fTextureAnimationFrameDelay);
        PutU(d.dwTextureAnimationStartFrame);
        End();
        PutTable("alpha", d.TimeEventAlpha);
    }
}

void PutLight(CLightData &l)
{
    std::fputs("LD", g_out);
    PutF(l.m_fDuration);
    PutI(l.m_bLoopFlag);
    PutI(l.m_iLoopCount);
    PutColor(l.m_cAmbient);
    PutColor(l.m_cDiffuse);
    PutF(l.m_fMaxRange);
    PutF(l.m_fAttenuation0);
    PutF(l.m_fAttenuation1);
    PutF(l.m_fAttenuation2);
    End();
    PutTable("range", l.m_TimeEventTableRange);
}

void DumpEffect(const std::string &name)
{
    // Never freed: CDynamicPool would hand recycled elements to the next
    // file, and Clear() does not reset the position table.
    auto *data = new CEffectData;
    bool ok = false;
    try
    {
        ok = data->LoadScript(name.c_str());
    }
    catch (const std::exception &)
    {
        std::fputs("F\t", g_out);
        Put(name);
        std::fputs("\tthrow\n", g_out);
        g_requests.clear();
        return;
    }

    std::fputs("F\t", g_out);
    Put(name);
    std::fprintf(g_out, "\t%d\n", ok ? 1 : 0);
    if (ok)
    {
        std::fputs("B", g_out);
        PutStr(data->m_strFileName);
        PutF(data->m_fBoundingSphereRadius);
        PutV3(data->m_v3BoundingSpherePosition);
        End();
        for (auto *p : data->m_ParticleVector)
        {
            std::fputs("P\n", g_out);
            PutElement(*p);
            PutParticle(*p);
        }
        for (auto *m : data->m_MeshVector)
        {
            std::fputs("M\n", g_out);
            PutElement(*m);
            PutMeshScript(*m);
        }
        for (auto *l : data->m_LightVector)
        {
            std::fputs("L\n", g_out);
            PutElement(*l);
            PutLight(*l);
        }
        for (auto const &s : data->m_SoundInstanceVector)
        {
            std::fputs("S", g_out);
            PutU(s.dwFrame);
            PutStr(s.strSoundFileName);
            End();
        }
    }
    PutRequests();
}

void DumpFlying(const std::string &name)
{
    auto *data = new CFlyingData;
    bool ok = false;
    try
    {
        ok = data->LoadScriptFile(name.c_str());
    }
    catch (const std::exception &)
    {
        std::fputs("F\t", g_out);
        Put(name);
        std::fputs("\tthrow\n", g_out);
        g_requests.clear();
        return;
    }
    // Requests come from the effects it registers, not from the flying data.
    g_requests.clear();

    std::fputs("F\t", g_out);
    Put(name);
    std::fprintf(g_out, "\t%d\n", ok ? 1 : 0);
    std::fputs("FD", g_out);
    PutStr(data->m_strFilename);
    PutI(data->m_bSpreading);
    PutI(data->m_bMaintainParallel);
    PutF(data->m_fInitVel);
    PutF(data->m_fConeAngle);
    PutF(data->m_fRollAngle);
    PutV3(data->m_v3AngVel);
    PutF(data->m_fGravity);
    PutF(data->m_fBombRange);
    PutStr(data->m_strBombEffectName);
    std::fprintf(g_out, "\t%08x", static_cast<unsigned>(data->m_dwBombEffectID));
    PutI(data->m_bIsHoming);
    PutF(data->m_fHomingMaxAngle);
    PutF(data->m_fHomingStartTime);
    PutI(data->m_bHitOnBackground);
    PutI(data->m_bHitOnAnotherMonster);
    PutI(data->m_iPierceCount);
    PutF(data->m_fCollisionSphereRadius);
    PutF(data->m_fRange);
    PutV3(data->m_v3Accel);
    End();
    for (auto const &a : data->m_AttachDataVector)
    {
        std::fputs("A", g_out);
        PutI(a.iType);
        PutI(a.iFlyType);
        PutStr(a.strFilename);
        PutI(a.bHasTail);
        std::fprintf(g_out, "\t%08x", static_cast<unsigned>(a.dwTailColor));
        PutF(a.fTailLength);
        PutF(a.fTailSize);
        PutI(a.bRectShape);
        PutF(a.fRoll);
        PutF(a.fDistance);
        PutF(a.fPeriod);
        PutF(a.fAmplitude);
        End();
    }
}

// Walks an .mde like __LoadData_Ver001/002 with bounds and index checks:
// whether the C++ reader stays inside the buffer and its vertex arrays.
bool MeshInBounds(const std::vector<char> &buf)
{
    size_t pos = 0;
    auto take = [&](size_t n) {
        if (buf.size() - pos < n)
            return false;
        pos += n;
        return true;
    };
    auto readU32 = [&](uint32_t &v) {
        if (buf.size() - pos < 4)
            return false;
        std::memcpy(&v, buf.data() + pos, 4);
        pos += 4;
        return true;
    };
    if (!take(11))
        return false;
    bool const v2 = std::memcmp(buf.data(), "MDEData002", 11) == 0;
    if (!v2 && std::memcmp(buf.data(), "EffectData", 11) != 0)
        return true; // rejected by the header check before reading further
    uint32_t geom = 0;
    uint32_t frames = 0;
    if (!readU32(geom) || !readU32(frames))
        return false;
    if (static_cast<int>(geom) < 0 || static_cast<int>(frames) < 0 || geom > 0x7fff)
        return false;
    for (uint32_t n = 0; n < geom; ++n)
    {
        if (!take(32 + 128))
            return false;
        uint32_t vc = 0;
        uint32_t ic = 0;
        uint32_t tc = 0;
        if (!v2 && (!readU32(vc) || !readU32(ic) || !readU32(tc)))
            return false;
        for (uint32_t f = 0; f < frames; ++f)
        {
            if (v2 && (!take(1 + 4) || !readU32(vc) || !readU32(ic) || !readU32(tc)))
                return false;
            if (!v2 && !take(4))
                return false;
            size_t const vbytes = size_t(vc) * 12;
            size_t const ibytes = size_t(ic) * 4;
            size_t const tbytes = size_t(tc) * 8;
            size_t const idx = pos + vbytes;
            size_t const tidx = idx + ibytes + tbytes;
            if (!take(vbytes) || !take(ibytes) || !take(tbytes) || !take(ibytes))
                return false;
            for (uint32_t j = 0; j < ic; ++j)
            {
                uint32_t a = 0;
                uint32_t b = 0;
                std::memcpy(&a, buf.data() + idx + size_t(j) * 4, 4);
                std::memcpy(&b, buf.data() + tidx + size_t(j) * 4, 4);
                if (a >= vc || b >= tc)
                    return false;
            }
        }
    }
    return true;
}

std::string CStr(const char *s, size_t cap)
{
    return {s, strnlen(s, cap)};
}

void DumpMesh(CEterPackManager &manager, const std::string &name)
{
    CEterPackManager::TPackDataPtr data;
    if (manager.Get(name, data) && data && !MeshInBounds(*data))
    {
        std::fputs("F\t", g_out);
        Put(name);
        std::fputs("\tskip\n", g_out);
        return;
    }

    auto *mesh = new CEffectMesh(name.c_str());
    mesh->Load();
    bool const ok = !mesh->IsEmpty();
    std::fputs("F\t", g_out);
    Put(name);
    std::fprintf(g_out, "\t%d\n", ok ? 1 : 0);
    if (ok)
    {
        std::fputs("H", g_out);
        PutI(mesh->m_iGeomCount);
        PutI(mesh->m_iFrameCount);
        End();
        for (auto *m : mesh->m_pEffectMeshDataVector)
        {
            std::fputs("G", g_out);
            PutStr(CStr(m->szObjectName, sizeof(m->szObjectName)));
            PutStr(CStr(m->szDiffuseMapFileName, sizeof(m->szDiffuseMapFileName)));
            PutU(m->EffectFrameDataVector.size());
            PutU(m->pImageVector.size());
            End();
            for (auto const &f : m->EffectFrameDataVector)
            {
                std::fputs("FR", g_out);
                PutU(f.byChangedFrame);
                PutF(f.fVisibility);
                PutU(f.dwVertexCount);
                PutU(f.dwTextureVertexCount);
                PutU(f.dwIndexCount);
                PutU(f.PDTVertexVector.size());
                End();
                for (auto const &v : f.PDTVertexVector)
                {
                    std::fputs("V", g_out);
                    PutF(v.position.x);
                    PutF(v.position.y);
                    PutF(v.position.z);
                    PutF(v.texCoord.x);
                    PutF(v.texCoord.y);
                    End();
                }
            }
        }
    }
    PutRequests();
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

} // namespace

int main(int argc, char **argv)
{
    if (argc < 4)
    {
        std::fprintf(stderr, "usage: %s <client dir> <index file> <mse|msf|mde>\n", argv[0]);
        return 2;
    }

    // TraceError writes to stdout: keep the real stdout for the dump only.
    std::fflush(stdout);
    g_out = fdopen(dup(STDOUT_FILENO), "w");
    dup2(STDERR_FILENO, STDOUT_FILENO);

    std::filesystem::current_path(argv[1]);
    std::string const kind = argv[3];

    CEterPackManager manager;
    manager.SetSearchMode(true);
    if (!RegisterIndex(manager, "pack", argv[2]))
        return 1;

    CResourceManager resourceManager;
    CEffectManager effectManager;

    std::vector<std::string> names;
    for (auto const &entry : manager.GetFileMap())
        if (Extension(entry.first) == kind)
            names.push_back(entry.first);
    std::ranges::sort(names);

    for (auto const &name : names)
    {
        if (kind == "mse")
            DumpEffect(name);
        else if (kind == "msf")
            DumpFlying(name);
        else
            DumpMesh(manager, name);
    }
    std::fclose(g_out);
    std::_Exit(0); // skip the singletons' teardown
}
