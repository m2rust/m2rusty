// Reference dump for the Rust gr2 reader (crates/m2-formats/src/gr2).
//
//   m2diff_gr2 [--full] pack <client dir> <index file>
//   m2diff_gr2 [--full] files <path>...
//
// "pack" registers packs like m2diff_pack and dumps every file whose virtual
// name ends in ".gr2", sorted by name; "files" dumps plain files in argument
// order. Each file goes through GrannyParseGR2FromMemory and everything the
// parsed granny_file exposes is printed, floats as the hex of their bits.
// Bulk arrays (vertices, indices, curve knots and controls) are printed as
// count + fnv1a64 of their bytes, or element by element with --full.

#include <Neoeter/IO/EterPackManager.h>
#include <Neoeter/Model/Grn/Gr2Reader.h>
#include <Neoeter/Platform/Base/FileLoader.h>

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

namespace
{

bool g_full = false;

uint64_t Fnv1a64(const void *data, size_t size)
{
    const auto *p = static_cast<const unsigned char *>(data);
    uint64_t hash = 0xcbf29ce484222325ull;
    for (size_t i = 0; i < size; ++i)
    {
        hash ^= p[i];
        hash *= 0x100000001b3ull;
    }
    return hash;
}

uint32_t Bits(float f)
{
    uint32_t u;
    std::memcpy(&u, &f, sizeof(u));
    return u;
}

// Bytes as they are, except control bytes and '\', which print as \xHH.
void PutName(const char *s)
{
    if (!s)
        return;
    for (const auto *p = reinterpret_cast<const unsigned char *>(s); *p; ++p)
    {
        if (*p < 0x20 || *p == '\\')
            std::printf("\\x%02x", *p);
        else
            std::putchar(*p);
    }
}

void PutFloats(const float *f, int n)
{
    for (int i = 0; i < n; ++i)
        std::printf(i ? ",%08x" : "%08x", Bits(f[i]));
}

void PutTransform(const granny_transform &t)
{
    std::printf("%08x,", t.Flags);
    PutFloats(t.Position, 3);
    std::putchar(',');
    PutFloats(t.Orientation, 4);
    std::putchar(',');
    PutFloats(&t.ScaleShear[0][0], 9);
}

// " <count> <fnv1a64>" of n floats, "-" when the pointer is null; with
// --full the floats themselves.
void PutFloatArray(const float *f, int n)
{
    if (!f)
    {
        std::printf(" -");
        return;
    }
    if (g_full)
    {
        std::putchar(' ');
        PutFloats(f, n);
        return;
    }
    std::printf(" %016llx", static_cast<unsigned long long>(Fnv1a64(f, static_cast<size_t>(n) * 4)));
}

void PutCurve(const char *label, const granny_curve &c)
{
    std::printf(" %s=%d,%d", label, c.Degree, c.KnotCount);
    PutFloatArray(c.Knots, c.KnotCount);
    std::printf(" %d", c.ControlCount);
    PutFloatArray(c.Controls, c.ControlCount);
}

void DumpVertexData(const granny_mesh &mesh)
{
    const granny_vertex_data *v = mesh.PrimaryVertexData;
    if (!v)
    {
        std::printf("  vertexdata none\n");
        return;
    }
    bool const rigid = v->VertexType == GrannyPNT332VertexType;
    int const stride = rigid ? 32 : 40;
    int const n = std::max(0, v->VertexCount);
    std::printf("  vertexdata %s %d", rigid ? "pnt332" : "pwnt3432", v->VertexCount);
    if (!v->Vertices)
    {
        std::printf(" -\n");
        return;
    }
    if (!g_full)
    {
        std::printf(" %016llx\n",
                    static_cast<unsigned long long>(Fnv1a64(v->Vertices, static_cast<size_t>(n) * stride)));
        return;
    }
    std::printf("\n");
    for (int i = 0; i < n; ++i)
    {
        const uint8_t *p = v->Vertices + (static_cast<size_t>(i) * stride);
        float f[10];
        if (rigid)
        {
            std::memcpy(f, p, 32);
            std::printf("   v ");
            PutFloats(f, 8);
        }
        else
        {
            std::memcpy(f, p, 12);
            std::memcpy(f + 3, p + 20, 20);
            std::printf("   v ");
            PutFloats(f, 3);
            std::printf(" %02x%02x%02x%02x %02x%02x%02x%02x ", p[12], p[13], p[14], p[15], p[16], p[17], p[18], p[19]);
            PutFloats(f + 3, 5);
        }
        std::printf("\n");
    }
}

void DumpTopology(const granny_mesh &mesh)
{
    const granny_tri_topology *t = mesh.PrimaryTopology;
    if (!t)
    {
        std::printf("  topology none\n");
        return;
    }
    std::printf("  topology groups %d\n", t->GroupCount);
    for (int g = 0; g < t->GroupCount; ++g)
        std::printf("   group %d %d %d %d\n", g, t->Groups[g].MaterialIndex, t->Groups[g].TriFirst,
                    t->Groups[g].TriCount);

    const char *kind = t->Indices16 ? "u16" : t->Indices ? "i32" : "none";
    std::printf("  indices %s %d", kind, t->IndexCount);
    size_t const bytes = static_cast<size_t>(std::max(0, t->IndexCount)) * (t->Indices16 ? 2 : 4);
    const void *data = t->Indices16 ? static_cast<const void *>(t->Indices16) : t->Indices;
    if (!data)
        std::printf("\n");
    else if (!g_full)
        std::printf(" %016llx\n", static_cast<unsigned long long>(Fnv1a64(data, bytes)));
    else
    {
        for (int i = 0; i < t->IndexCount; ++i)
        {
            if (t->Indices16)
                std::printf(i ? ",%u" : " %u", t->Indices16[i]);
            else
                std::printf(i ? ",%d" : " %d", t->Indices[i]);
        }
        std::printf("\n");
    }
}

void DumpMaterial(int index, const granny_material *m)
{
    if (!m)
    {
        std::printf("  material %d none\n", index);
        return;
    }
    std::printf("  material %d ", index);
    PutName(m->Name);
    std::printf("\t%d %d\n", m->MapCount, m->Maps ? m->MapCount : 0);
    for (int i = 0; m->Maps && i < m->MapCount; ++i)
    {
        const granny_material_map &map = m->Maps[i];
        std::printf("   map %d ", i);
        PutName(map.Usage);
        if (!map.Material)
            std::printf("\tnone\n");
        else if (!map.Material->Texture)
            std::printf("\tnotexture\n");
        else
        {
            std::printf("\ttexture ");
            PutName(map.Material->Texture->FromFileName);
            std::printf("\n");
        }
    }
}

void DumpMesh(int index, const granny_mesh *mesh)
{
    if (!mesh)
    {
        std::printf(" mesh %d none\n", index);
        return;
    }
    std::printf(" mesh %d ", index);
    PutName(mesh->Name);
    std::printf("\n");
    DumpVertexData(*mesh);
    DumpTopology(*mesh);

    std::printf("  materials %d %d\n", mesh->MaterialBindingCount,
                mesh->MaterialBindings ? mesh->MaterialBindingCount : 0);
    for (int i = 0; mesh->MaterialBindings && i < mesh->MaterialBindingCount; ++i)
        DumpMaterial(i, mesh->MaterialBindings[i].Material);

    std::printf("  bonebindings %d %d\n", mesh->BoneBindingCount, mesh->BoneBindings ? mesh->BoneBindingCount : 0);
    for (int i = 0; mesh->BoneBindings && i < mesh->BoneBindingCount; ++i)
    {
        const granny_bone_binding &b = mesh->BoneBindings[i];
        std::printf("   bonebinding %d ", i);
        PutName(b.BoneName);
        std::putchar('\t');
        PutFloats(b.OBBMin, 3);
        std::putchar(' ');
        PutFloats(b.OBBMax, 3);
        std::printf("\n");
    }
}

void DumpModel(int index, const granny_model *model)
{
    std::printf("model %d ", index);
    PutName(model->Name);
    std::putchar('\t');
    PutTransform(model->InitialPlacement);
    std::printf("\n");

    if (const granny_skeleton *s = model->Skeleton)
    {
        std::printf(" skeleton ");
        PutName(s->Name);
        std::printf("\t%d %d\n", s->BoneCount, s->Bones ? s->BoneCount : 0);
        for (int b = 0; s->Bones && b < s->BoneCount; ++b)
        {
            const granny_bone &bone = s->Bones[b];
            std::printf("  bone %d ", b);
            PutName(bone.Name);
            std::printf("\t%d ", bone.ParentIndex);
            PutTransform(bone.LocalTransform);
            std::putchar(' ');
            PutFloats(&bone.InverseWorld4x4[0][0], 16);
            std::printf("\n");
        }
    }
    else
        std::printf(" skeleton none\n");

    std::printf(" meshbindings %d %d\n", model->MeshBindingCount, model->MeshBindings ? model->MeshBindingCount : 0);
    for (int i = 0; model->MeshBindings && i < model->MeshBindingCount; ++i)
        DumpMesh(i, model->MeshBindings[i].Mesh);
}

void DumpAnimation(int index, const granny_animation *a)
{
    std::printf("animation %d ", index);
    PutName(a->Name);
    std::printf("\t%08x %08x %d\n", Bits(a->Duration), Bits(a->TimeStep), a->TrackGroupCount);
    for (int g = 0; g < a->TrackGroupCount; ++g)
    {
        const granny_track_group *tg = a->TrackGroups[g];
        std::printf(" trackgroup %d ", g);
        PutName(tg->Name);
        std::putchar('\t');
        PutFloats(tg->LoopTranslation, 3);
        std::printf(" %d\n", tg->TransformTrackCount);
        for (int t = 0; t < tg->TransformTrackCount; ++t)
        {
            const granny_transform_track &tt = tg->TransformTracks[t];
            std::printf("  track %d ", t);
            PutName(tt.Name);
            std::putchar('\t');
            PutCurve("pos", tt.PositionCurve);
            PutCurve("rot", tt.OrientationCurve);
            PutCurve("ss", tt.ScaleShearCurve);
            std::printf("\n");
        }
    }
}

void Dump(const std::string &name, const std::vector<char> &data)
{
    std::printf("file %s\n", name.c_str());
    granny_file *file = GrannyParseGR2FromMemory(static_cast<int>(data.size()), data.data());
    if (!file)
    {
        std::printf("error\n");
        return;
    }
    const granny_file_info *info = file->FileInfo;
    std::printf("models %d %d\n", info->ModelCount, info->Models ? info->ModelCount : 0);
    for (int m = 0; info->Models && m < info->ModelCount; ++m)
        DumpModel(m, info->Models[m]);
    std::printf("animations %d %d\n", info->AnimationCount, info->Animations ? info->AnimationCount : 0);
    for (int a = 0; info->Animations && a < info->AnimationCount; ++a)
        DumpAnimation(a, info->Animations[a]);
    GrannyFreeParsedGR2(file);
}

// Same registration loop as tools/differential/pack/PackDump.cpp.
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
        {
            std::fprintf(stderr, "line %u: expected 2 tokens, got %zu\n", static_cast<unsigned>(i), tokens.size());
            return false;
        }

        std::string const name = folder + tokens[0];
        manager.RegisterPack(name, tokens[1]);

        std::string const texCache = name + "_texcache";
        if (manager.PackExists(texCache, tokens[1]))
            manager.RegisterPack(texCache, tokens[1]);
    }
    return true;
}

int DumpPacks(const char *clientDir, const char *indexFile)
{
    std::filesystem::current_path(clientDir);
    CEterPackManager manager;
    manager.SetSearchMode(true);
    if (!RegisterIndex(manager, "pack", indexFile))
        return 1;

    std::vector<std::string> names;
    for (auto const &entry : manager.GetFileMap())
        if (entry.first.ends_with(".gr2"))
            names.push_back(entry.first);
    std::ranges::sort(names);

    for (auto const &name : names)
    {
        CEterPackManager::TPackDataPtr data;
        if (manager.Get(name, data) && data)
            Dump(name, *data);
        else
            std::printf("file %s\nunreadable\n", name.c_str());
    }
    return 0;
}

int DumpFiles(int count, char **paths)
{
    for (int i = 0; i < count; ++i)
    {
        std::ifstream in(paths[i], std::ios::binary);
        if (!in)
        {
            std::printf("file %s\nunreadable\n", paths[i]);
            continue;
        }
        std::vector<char> bytes((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
        Dump(paths[i], bytes);
    }
    return 0;
}

} // namespace

int main(int argc, char **argv)
{
    int arg = 1;
    if (arg < argc && std::strcmp(argv[arg], "--full") == 0)
    {
        g_full = true;
        ++arg;
    }
    if (arg + 2 < argc && std::strcmp(argv[arg], "pack") == 0)
        return DumpPacks(argv[arg + 1], argv[arg + 2]);
    if (arg < argc && std::strcmp(argv[arg], "files") == 0)
        return DumpFiles(argc - arg - 1, argv + arg + 1);

    std::fprintf(stderr, "usage: %s [--full] pack <client dir> <index file>\n       %s [--full] files <path>...\n",
                 argv[0], argv[0]);
    return 2;
}
