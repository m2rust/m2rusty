#include "Neoeter/Model/Grn/Gr2Reader.h"
#include "Neoeter/Model/Grn/Gr2Decompress.h"
#include "Neoeter/Model/Grn/GrannyCompat.h"

#include <vector>
#include <deque>
#include <map>
#include <string>
#include <memory>
#include <cstring>
#include <cstdint>

namespace {

struct GR2ParsedContext
{
    granny_file file;
    granny_file_info fileInfo;
    std::vector<std::vector<uint8_t>> sections;
    std::vector<std::unique_ptr<granny_model>> models;
    std::vector<granny_model*> modelPtrs;
    std::vector<std::unique_ptr<granny_skeleton>> skeletons;
    std::vector<std::vector<granny_bone>> skeletonBones;
    std::vector<std::vector<granny_model_mesh_binding>> modelMeshBindings;
    std::vector<std::unique_ptr<granny_mesh>> meshes;
    std::vector<std::unique_ptr<granny_vertex_data>> vertexDatas;
    std::vector<std::unique_ptr<granny_tri_topology>> topologies;
    std::vector<std::vector<granny_tri_material_group>> triGroups;
    std::vector<std::vector<granny_material_binding>> meshMaterialBindings;
    std::vector<std::unique_ptr<granny_material>> materials;
    std::vector<std::vector<granny_material_map>> materialMaps;
    std::vector<std::unique_ptr<granny_texture>> textures;
    std::vector<std::vector<granny_bone_binding>> meshBoneBindings;
    std::vector<std::unique_ptr<granny_animation>> animations;
    std::vector<granny_animation*> animPtrs;
    std::vector<std::unique_ptr<granny_track_group>> trackGroups;
    std::vector<granny_track_group*> tgPtrs;
    std::vector<std::vector<granny_transform_track>> tgTransformTracks;
    std::vector<std::vector<float>> curveKnots;
    std::vector<std::vector<float>> curveControls;
    std::deque<std::string> stringStore;

    const char* StoreString(const char* s)
    {
        if (!s || !*s) return "";
        stringStore.emplace_back(s);
        return stringStore.back().c_str();
    }
};

static const char* ResolveString(
    const std::vector<std::vector<uint8_t>>& sections,
    const std::vector<std::map<uint32_t, std::pair<uint32_t, uint32_t>>>& fixups,
    uint32_t secIdx, uint32_t offset)
{
    if (secIdx >= fixups.size()) return "";
    auto it = fixups[secIdx].find(offset);
    if (it == fixups[secIdx].end()) return "";
    uint32_t toSec = it->second.first;
    uint32_t toOff = it->second.second;
    if (toSec >= sections.size() || toOff >= sections[toSec].size()) return "";

    const uint8_t* p = sections[toSec].data() + toOff;
    const size_t remaining = sections[toSec].size() - toOff;
    if (std::memchr(p, 0, remaining) == nullptr) return "";
    return reinterpret_cast<const char*>(p);
}

// Hard limits that keep a hostile file from turning a count into an oversized
// allocation. Real Metin2 assets are far below every one of these.
constexpr uint32_t kMaxSections = 4096;
constexpr uint32_t kMaxSectionBytes = 256u * 1024u * 1024u;
constexpr int32_t kMaxRecords = 200000;

} // namespace

extern "C" granny_file* GrannyParseGR2FromMemory(int size, const void* memory)
{
    if (!memory || size < 88)
        return nullptr;

    const uint8_t* buf = static_cast<const uint8_t*>(memory);

    // Granny magic check: B8 67 B0 CA F8 6D B1 0F 84 72 8C 7E 5E 19 00 1E
    static const uint8_t GR2_MAGIC[16] = {
        0xb8, 0x67, 0xb0, 0xca, 0xf8, 0x6d, 0xb1, 0x0f,
        0x84, 0x72, 0x8c, 0x7e, 0x5e, 0x19, 0x00, 0x1e
    };

    if (std::memcmp(buf, GR2_MAGIC, 16) != 0)
        return nullptr;

    uint32_t secOffset   = *reinterpret_cast<const uint32_t*>(&buf[32 + 12]);
    uint32_t numSec      = *reinterpret_cast<const uint32_t*>(&buf[32 + 16]);
    uint32_t rootTypeSec = *reinterpret_cast<const uint32_t*>(&buf[32 + 20]);
    uint32_t rootTypeOff = *reinterpret_cast<const uint32_t*>(&buf[32 + 24]);
    uint32_t rootNodeSec = *reinterpret_cast<const uint32_t*>(&buf[32 + 28]);
    uint32_t rootNodeOff = *reinterpret_cast<const uint32_t*>(&buf[32 + 32]);

    if (numSec == 0 || numSec > kMaxSections)
        return nullptr;
    if (secOffset > static_cast<uint32_t>(size))
        return nullptr;

    const size_t sectionTable = 32u + static_cast<size_t>(secOffset);
    const size_t sectionTableBytes = static_cast<size_t>(numSec) * 44u;
    if (sectionTable > static_cast<size_t>(size) ||
        sectionTableBytes > static_cast<size_t>(size) - sectionTable)
        return nullptr;

    auto ctx = std::make_unique<GR2ParsedContext>();
    ctx->sections.resize(numSec);
    std::vector<std::map<uint32_t, std::pair<uint32_t, uint32_t>>> fixups(numSec);

    for (uint32_t i = 0; i < numSec; ++i) {
        const uint8_t* pSec = &buf[32 + secOffset + i * 44];
        uint32_t fmt      = *reinterpret_cast<const uint32_t*>(pSec + 0);
        uint32_t off      = *reinterpret_cast<const uint32_t*>(pSec + 4);
        uint32_t compSz   = *reinterpret_cast<const uint32_t*>(pSec + 8);
        uint32_t uncompSz = *reinterpret_cast<const uint32_t*>(pSec + 12);
        uint32_t f16      = *reinterpret_cast<const uint32_t*>(pSec + 20);
        uint32_t f8       = *reinterpret_cast<const uint32_t*>(pSec + 24);
        uint32_t relocOff = *reinterpret_cast<const uint32_t*>(pSec + 28);
        uint32_t numReloc = *reinterpret_cast<const uint32_t*>(pSec + 32);

        if (uncompSz > kMaxSectionBytes)
            return nullptr;
        if (fmt != 0 && fmt != 2)
            return nullptr;
        if (compSz > 0) {
            if (off > static_cast<uint32_t>(size) ||
                compSz > static_cast<uint32_t>(size) - off)
                return nullptr;
        }

        ctx->sections[i].resize(uncompSz);
        if (compSz > 0 && uncompSz > 0) {
            if (fmt == 0) {
                std::memcpy(ctx->sections[i].data(), &buf[off], std::min(compSz, uncompSz));
            } else if (!gr2_decompress(compSz, &buf[off], f16, f8, uncompSz,
                                       ctx->sections[i].data())) {
                return nullptr;
            }
        }

        if (numReloc > 0) {
            if (relocOff > static_cast<uint32_t>(size) ||
                static_cast<size_t>(numReloc) > (static_cast<size_t>(size) - relocOff) / 12)
                return nullptr;

            for (uint32_t r = 0; r < numReloc; ++r) {
                const uint8_t* pReloc = &buf[relocOff + r * 12];
                uint32_t fromOff = *reinterpret_cast<const uint32_t*>(pReloc + 0);
                uint32_t toSec   = *reinterpret_cast<const uint32_t*>(pReloc + 4);
                uint32_t toOff   = *reinterpret_cast<const uint32_t*>(pReloc + 8);
                fixups[i][fromOff] = {toSec, toOff};
            }
        }
    }

    if (rootNodeSec >= ctx->sections.size() || rootNodeOff + 16 > ctx->sections[rootNodeSec].size())
        return nullptr;

    // Dynamically discover member offsets from root type definition
    uint32_t modelsOffset = 0x3C;
    bool hasModels = false;
    uint32_t animsOffset = 0x4C;
    bool hasAnims = false;

    if (rootTypeSec < ctx->sections.size()) {
        uint32_t curTypeOff = rootTypeOff;
        uint32_t curDataOff = rootNodeOff;
        const auto& typeSec = ctx->sections[rootTypeSec];

        while (curTypeOff + 32 <= typeSec.size()) {
            uint32_t mType = *reinterpret_cast<const uint32_t*>(&typeSec[curTypeOff]);
            if (mType == 0) break; // GrannyEndMember

            std::string mName;
            if (fixups[rootTypeSec].count(curTypeOff + 4)) {
                auto [nSec, nOff] = fixups[rootTypeSec][curTypeOff + 4];
                if (nSec < ctx->sections.size() && nOff < ctx->sections[nSec].size()) {
                    mName = reinterpret_cast<const char*>(&ctx->sections[nSec][nOff]);
                }
            }

            uint32_t memberSize = 4;
            switch (mType) {
                case 1: memberSize = 0; break;
                case 2: case 8: case 10: case 15: case 16: memberSize = 4; break;
                case 3: case 4: case 5: memberSize = 8; break;
                case 7: memberSize = 12; break;
                case 9: memberSize = 68; break;
                case 11: case 12: memberSize = 1; break;
                case 13: case 14: memberSize = 2; break;
                default: memberSize = 4; break;
            }

            if (mName == "Models") {
                modelsOffset = curDataOff;
                hasModels = true;
            } else if (mName == "Animations") {
                animsOffset = curDataOff;
                hasAnims = true;
            }

            curDataOff += memberSize;
            curTypeOff += 32;
        }
    }

    int32_t modelCount = 0;
    if (hasModels && rootNodeSec < ctx->sections.size() && modelsOffset + 8 <= ctx->sections[rootNodeSec].size()) {
        modelCount = *reinterpret_cast<const int32_t*>(&ctx->sections[rootNodeSec][modelsOffset]);
        if (modelCount < 0 || modelCount > 2000) modelCount = 0;
    }

    int32_t animCount = 0;
    if (hasAnims && rootNodeSec < ctx->sections.size() && animsOffset + 8 <= ctx->sections[rootNodeSec].size()) {
        animCount = *reinterpret_cast<const int32_t*>(&ctx->sections[rootNodeSec][animsOffset]);
        if (animCount < 0 || animCount > 2000) animCount = 0;
    }

    ctx->fileInfo.ModelCount     = std::max(0, modelCount);
    ctx->fileInfo.AnimationCount = std::max(0, animCount);

    // Unmarshal Models
    if (modelCount > 0 && fixups[rootNodeSec].count(modelsOffset + 4)) {
        auto [mArrSec, mArrOff] = fixups[rootNodeSec][modelsOffset + 4];
        ctx->models.reserve(modelCount);
        ctx->modelMeshBindings.resize(modelCount);

        for (int m = 0; m < modelCount; ++m) {
            if (!fixups[mArrSec].count(mArrOff + m * 4)) continue;
            auto [mSec, mOff] = fixups[mArrSec][mArrOff + m * 4];
            if (mSec >= ctx->sections.size() || mOff + 84 > ctx->sections[mSec].size()) continue;

            const uint8_t* pModel = &ctx->sections[mSec][mOff];
            auto model = std::make_unique<granny_model>();
            std::memset(model.get(), 0, sizeof(granny_model));
            model->Name = ctx->StoreString(ResolveString(ctx->sections, fixups, mSec, mOff + 0));
            std::memcpy(&model->InitialPlacement, pModel + 8, sizeof(granny_transform));

            // Skeleton
            if (fixups[mSec].count(mOff + 4)) {
                auto [skSec, skOff] = fixups[mSec][mOff + 4];
                if (skSec < ctx->sections.size() && skOff + 12 <= ctx->sections[skSec].size()) {
                    const uint8_t* pSkel = &ctx->sections[skSec][skOff];
                    auto skel = std::make_unique<granny_skeleton>();
                    std::memset(skel.get(), 0, sizeof(granny_skeleton));
                    skel->Name = ctx->StoreString(ResolveString(ctx->sections, fixups, skSec, skOff + 0));
                    int32_t boneCount = *reinterpret_cast<const int32_t*>(pSkel + 4);
                    if (boneCount > kMaxRecords) return nullptr;
                    skel->BoneCount = std::max(0, boneCount);

                    if (boneCount > 0 && fixups[skSec].count(skOff + 8)) {
                        auto [bnSec, bnOff] = fixups[skSec][skOff + 8];
                        ctx->skeletonBones.emplace_back(boneCount);
                        auto& bones = ctx->skeletonBones.back();
                        for (int b = 0; b < boneCount; ++b) {
                            uint32_t bOff = bnOff + b * 156;
                            if (bnSec < ctx->sections.size() && bOff + 140 <= ctx->sections[bnSec].size()) {
                                const uint8_t* pBone = &ctx->sections[bnSec][bOff];
                                bones[b].Name = ctx->StoreString(ResolveString(ctx->sections, fixups, bnSec, bOff + 0));
                                bones[b].ParentIndex = *reinterpret_cast<const int32_t*>(pBone + 4);
                                std::memcpy(&bones[b].LocalTransform, pBone + 8, sizeof(granny_transform));
                                std::memcpy(bones[b].InverseWorld4x4, pBone + 76, sizeof(granny_matrix_4x4));
                            }
                        }
                        skel->Bones = bones.data();
                    }
                    model->Skeleton = skel.get();
                    ctx->skeletons.push_back(std::move(skel));
                }
            }

            // Mesh Bindings
            int32_t mbCount = *reinterpret_cast<const int32_t*>(pModel + 76);
            if (mbCount > kMaxRecords) return nullptr;
            model->MeshBindingCount = std::max(0, mbCount);
            if (mbCount > 0 && fixups[mSec].count(mOff + 80)) {
                auto [mbSec, mbOff] = fixups[mSec][mOff + 80];
                auto& bindings = ctx->modelMeshBindings[m];
                bindings.resize(mbCount);

                for (int b = 0; b < mbCount; ++b) {
                    if (!fixups[mbSec].count(mbOff + b * 4)) continue;
                    auto [meshSec, meshOff] = fixups[mbSec][mbOff + b * 4];
                    if (meshSec >= ctx->sections.size() || meshOff + 36 > ctx->sections[meshSec].size()) continue;

                    const uint8_t* pMesh = &ctx->sections[meshSec][meshOff];
                    auto mesh = std::make_unique<granny_mesh>();
                    std::memset(mesh.get(), 0, sizeof(granny_mesh));
                    mesh->Name = ctx->StoreString(ResolveString(ctx->sections, fixups, meshSec, meshOff + 0));

                    // Vertex Data
                    if (fixups[meshSec].count(meshOff + 4)) {
                        auto [vtxSec, vtxOff] = fixups[meshSec][meshOff + 4];
                        if (vtxSec < ctx->sections.size() && vtxOff + 12 <= ctx->sections[vtxSec].size()) {
                            const uint8_t* pVtx = &ctx->sections[vtxSec][vtxOff];
                            auto vData = std::make_unique<granny_vertex_data>();
                            std::memset(vData.get(), 0, sizeof(granny_vertex_data));
                            vData->VertexCount = *reinterpret_cast<const int32_t*>(pVtx + 4);
                            if (fixups[vtxSec].count(vtxOff + 8)) {
                                auto [dSec, dOff] = fixups[vtxSec][vtxOff + 8];
                                if (dSec < ctx->sections.size() && dOff < ctx->sections[dSec].size()) {
                                    vData->Vertices = &ctx->sections[dSec][dOff];
                                }
                            }
                            mesh->PrimaryVertexData = vData.get();
                            ctx->vertexDatas.push_back(std::move(vData));
                        }
                    }

                    // Topology
                    if (fixups[meshSec].count(meshOff + 16)) {
                        auto [topoSec, topoOff] = fixups[meshSec][meshOff + 16];
                        if (topoSec < ctx->sections.size() && topoOff + 24 <= ctx->sections[topoSec].size()) {
                            const uint8_t* pTopo = &ctx->sections[topoSec][topoOff];
                            auto topo = std::make_unique<granny_tri_topology>();
                            std::memset(topo.get(), 0, sizeof(granny_tri_topology));
                            int32_t grpCount   = *reinterpret_cast<const int32_t*>(pTopo + 0);
                            int32_t idxCount   = *reinterpret_cast<const int32_t*>(pTopo + 8);
                            int32_t idxCount16 = *reinterpret_cast<const int32_t*>(pTopo + 16);
                            if (grpCount > kMaxRecords || idxCount > kMaxRecords || idxCount16 > kMaxRecords)
                                return nullptr;

                            if (idxCount16 > 0 && fixups[topoSec].count(topoOff + 20)) {
                                auto [iSec, iOff] = fixups[topoSec][topoOff + 20];
                                if (iSec < ctx->sections.size() &&
                                    iOff <= ctx->sections[iSec].size() &&
                                    static_cast<size_t>(idxCount16) <=
                                        (ctx->sections[iSec].size() - iOff) / sizeof(uint16_t)) {
                                    topo->Indices16 = reinterpret_cast<uint16_t*>(&ctx->sections[iSec][iOff]);
                                    topo->IndexCount = idxCount16;
                                }
                            } else if (idxCount > 0 && fixups[topoSec].count(topoOff + 12)) {
                                auto [iSec, iOff] = fixups[topoSec][topoOff + 12];
                                if (iSec < ctx->sections.size() &&
                                    iOff <= ctx->sections[iSec].size() &&
                                    static_cast<size_t>(idxCount) <=
                                        (ctx->sections[iSec].size() - iOff) / sizeof(int32_t)) {
                                    topo->Indices = reinterpret_cast<int32_t*>(&ctx->sections[iSec][iOff]);
                                    topo->IndexCount = idxCount;
                                }
                            }

                            if (grpCount > 0 && fixups[topoSec].count(topoOff + 4)) {
                                auto [gSec, gOff] = fixups[topoSec][topoOff + 4];
                                if (gSec < ctx->sections.size() && gOff + grpCount * sizeof(granny_tri_material_group) <= ctx->sections[gSec].size()) {
                                    ctx->triGroups.emplace_back(grpCount);
                                    auto& groups = ctx->triGroups.back();
                                    std::memcpy(groups.data(), &ctx->sections[gSec][gOff], grpCount * sizeof(granny_tri_material_group));
                                    topo->GroupCount = grpCount;
                                    topo->Groups = groups.data();
                                }
                            }
                            mesh->PrimaryTopology = topo.get();
                            ctx->topologies.push_back(std::move(topo));
                        }
                    }

                    // Materials
                    int32_t matCount = *reinterpret_cast<const int32_t*>(pMesh + 20);
                    if (matCount > kMaxRecords) return nullptr;
                    mesh->MaterialBindingCount = std::max(0, matCount);
                    if (matCount > 0 && fixups[meshSec].count(meshOff + 24)) {
                        auto [matArrSec, matArrOff] = fixups[meshSec][meshOff + 24];
                        ctx->meshMaterialBindings.emplace_back(matCount);
                        auto& matBindings = ctx->meshMaterialBindings.back();

                        for (int m_idx = 0; m_idx < matCount; ++m_idx) {
                            if (!fixups[matArrSec].count(matArrOff + m_idx * 4)) continue;
                            auto [matSec, matOff] = fixups[matArrSec][matArrOff + m_idx * 4];
                            if (matSec >= ctx->sections.size() || matOff + 12 > ctx->sections[matSec].size()) continue;

                            const uint8_t* pMat = &ctx->sections[matSec][matOff];
                            auto mat = std::make_unique<granny_material>();
                            std::memset(mat.get(), 0, sizeof(granny_material));
                            mat->Name = ctx->StoreString(ResolveString(ctx->sections, fixups, matSec, matOff + 0));
                            int32_t mapCount = *reinterpret_cast<const int32_t*>(pMat + 4);
                            if (mapCount > kMaxRecords) return nullptr;
                            mat->MapCount = std::max(0, mapCount);

                            if (mapCount > 0 && fixups[matSec].count(matOff + 8)) {
                                auto [mapSec, mapOff] = fixups[matSec][matOff + 8];
                                ctx->materialMaps.emplace_back(mapCount);
                                auto& maps = ctx->materialMaps.back();
                                for (int mp = 0; mp < mapCount; ++mp) {
                                    uint32_t curMapOff = mapOff + mp * 8;
                                    maps[mp].Usage = ctx->StoreString(ResolveString(ctx->sections, fixups, mapSec, curMapOff + 0));
                                    if (fixups[mapSec].count(curMapOff + 4)) {
                                        auto [subMSec, subMOff] = fixups[mapSec][curMapOff + 4];
                                        auto subMat = std::make_unique<granny_material>();
                                        std::memset(subMat.get(), 0, sizeof(granny_material));
                                        if (fixups[subMSec].count(subMOff + 12)) {
                                            auto [texSec, texOff] = fixups[subMSec][subMOff + 12];
                                            auto tex = std::make_unique<granny_texture>();
                                            std::memset(tex.get(), 0, sizeof(granny_texture));
                                            tex->FromFileName = const_cast<char*>(ctx->StoreString(ResolveString(ctx->sections, fixups, texSec, texOff + 0)));
                                            subMat->Texture = tex.get();
                                            ctx->textures.push_back(std::move(tex));
                                        }
                                        maps[mp].Material = subMat.get();
                                        ctx->materials.push_back(std::move(subMat));
                                    }
                                }
                                mat->Maps = maps.data();
                            }
                            matBindings[m_idx].Material = mat.get();
                            ctx->materials.push_back(std::move(mat));
                        }
                        mesh->MaterialBindings = matBindings.data();
                    }

                    // Bone Bindings
                    int32_t bbCount = *reinterpret_cast<const int32_t*>(pMesh + 28);
                    if (bbCount > kMaxRecords) return nullptr;
                    mesh->BoneBindingCount = std::max(0, bbCount);
                    if (bbCount > 0 && fixups[meshSec].count(meshOff + 32)) {
                        auto [bbArrSec, bbArrOff] = fixups[meshSec][meshOff + 32];
                        ctx->meshBoneBindings.emplace_back(bbCount);
                        auto& bBindings = ctx->meshBoneBindings.back();
                        for (int bb = 0; bb < bbCount; ++bb) {
                            uint32_t curBbOff = bbArrOff + bb * 36;
                            if (bbArrSec < ctx->sections.size() && curBbOff + 28 <= ctx->sections[bbArrSec].size()) {
                                const uint8_t* pBb = &ctx->sections[bbArrSec][curBbOff];
                                bBindings[bb].BoneName = const_cast<char*>(ctx->StoreString(ResolveString(ctx->sections, fixups, bbArrSec, curBbOff + 0)));
                                std::memcpy(bBindings[bb].OBBMin, pBb + 4, sizeof(granny_triple));
                                std::memcpy(bBindings[bb].OBBMax, pBb + 16, sizeof(granny_triple));
                            }
                        }
                        mesh->BoneBindings = bBindings.data();
                    }

                    // Assign vertex type definition based on rigidity
                    if (mesh->PrimaryVertexData) {
                        if (mesh->BoneBindingCount <= 1) {
                            mesh->PrimaryVertexData->VertexType = GrannyPNT332VertexType;
                        } else {
                            mesh->PrimaryVertexData->VertexType = GrannyPWNT3432VertexType;
                        }
                    }

                    bindings[b].Mesh = mesh.get();
                    ctx->meshes.push_back(std::move(mesh));
                }
                model->MeshBindings = bindings.data();
            }

            ctx->modelPtrs.push_back(model.get());
            ctx->models.push_back(std::move(model));
        }
        ctx->fileInfo.Models = ctx->modelPtrs.empty() ? nullptr : ctx->modelPtrs.data();
        ctx->fileInfo.ModelCount = static_cast<granny_int32>(ctx->modelPtrs.size());
    }

    // Unmarshal Animations
    if (animCount > 0 && fixups[rootNodeSec].count(animsOffset + 4)) {
        auto [aArrSec, aArrOff] = fixups[rootNodeSec][animsOffset + 4];
        ctx->animations.reserve(animCount);

        for (int a = 0; a < animCount; ++a) {
            if (!fixups[aArrSec].count(aArrOff + a * 4)) continue;
            auto [aSec, aOff] = fixups[aArrSec][aArrOff + a * 4];
            if (aSec >= ctx->sections.size() || aOff + 16 > ctx->sections[aSec].size()) continue;

            const uint8_t* pAnim = &ctx->sections[aSec][aOff];
            auto anim = std::make_unique<granny_animation>();
            std::memset(anim.get(), 0, sizeof(granny_animation));
            anim->Name         = ctx->StoreString(ResolveString(ctx->sections, fixups, aSec, aOff + 0));
            anim->Duration     = *reinterpret_cast<const float*>(pAnim + 4);
            anim->TimeStep     = *reinterpret_cast<const float*>(pAnim + 8);
            anim->Oversampling = 0.0f;

            int32_t tgCount = *reinterpret_cast<const int32_t*>(pAnim + 12);
            if (tgCount > kMaxRecords) return nullptr;
            anim->TrackGroupCount = 0;
            anim->TrackGroups = nullptr;

            if (tgCount > 0 && fixups[aSec].count(aOff + 16)) {
                auto [tgArrSec, tgArrOff] = fixups[aSec][aOff + 16];
                size_t tgStartIdx = ctx->tgPtrs.size();

                for (int tgIdx = 0; tgIdx < tgCount; ++tgIdx) {
                    if (!fixups[tgArrSec].count(tgArrOff + tgIdx * 4)) continue;
                    auto [tgSec, tgOff] = fixups[tgArrSec][tgArrOff + tgIdx * 4];
                    if (tgSec >= ctx->sections.size() || tgOff + 20 > ctx->sections[tgSec].size()) continue;

                    const uint8_t* pTG = &ctx->sections[tgSec][tgOff];
                    auto trackGroup = std::make_unique<granny_track_group>();
                    std::memset(trackGroup.get(), 0, sizeof(granny_track_group));
                    trackGroup->Name = ctx->StoreString(ResolveString(ctx->sections, fixups, tgSec, tgOff + 0));

                    if (tgOff + 116 <= ctx->sections[tgSec].size()) {
                        const float* pLoopTrans = reinterpret_cast<const float*>(pTG + 104);
                        trackGroup->LoopTranslation[0] = pLoopTrans[0];
                        trackGroup->LoopTranslation[1] = pLoopTrans[1];
                        trackGroup->LoopTranslation[2] = pLoopTrans[2];
                    }

                    int32_t ttCount = *reinterpret_cast<const int32_t*>(pTG + 12);
                    if (ttCount > kMaxRecords) return nullptr;
                    if (ttCount > 0 && fixups[tgSec].count(tgOff + 16)) {
                        // Plain variables, not a structured binding: C++17
                        // lambdas cannot capture structured bindings.
                        const auto ttArr = fixups[tgSec][tgOff + 16];
                        const uint32_t ttArrSec = ttArr.first;
                        const uint32_t ttArrOff = ttArr.second;
                        ctx->tgTransformTracks.emplace_back();
                        auto& ttracks = ctx->tgTransformTracks.back();
                        ttracks.resize(ttCount);

                        for (int t = 0; t < ttCount; ++t) {
                            uint32_t curTTOff = ttArrOff + t * 64;
                            if (ttArrSec >= ctx->sections.size() || curTTOff + 64 > ctx->sections[ttArrSec].size()) break;
                            const uint8_t* pTT = &ctx->sections[ttArrSec][curTTOff];
                            ttracks[t].Name = ctx->StoreString(ResolveString(ctx->sections, fixups, ttArrSec, curTTOff + 0));

                            auto parseCurve = [&](uint32_t curveOff, granny_curve& curve) {
                                curve.Degree = *reinterpret_cast<const int32_t*>(pTT + curveOff + 0);
                                int32_t knotCount = *reinterpret_cast<const int32_t*>(pTT + curveOff + 4);
                                int32_t ctrlCount = *reinterpret_cast<const int32_t*>(pTT + curveOff + 12);
                                if (knotCount > kMaxRecords) knotCount = 0;
                                if (ctrlCount > kMaxRecords) ctrlCount = 0;

                                curve.KnotCount = std::max(0, knotCount);
                                curve.ControlCount = std::max(0, ctrlCount);

                                if (knotCount > 0 && fixups[ttArrSec].count(curTTOff + curveOff + 8)) {
                                    auto [kSec, kOff] = fixups[ttArrSec][curTTOff + curveOff + 8];
                                    if (kSec < ctx->sections.size() && kOff + knotCount * 4 <= ctx->sections[kSec].size()) {
                                        ctx->curveKnots.emplace_back(knotCount);
                                        std::memcpy(ctx->curveKnots.back().data(), &ctx->sections[kSec][kOff], knotCount * 4);
                                        curve.Knots = ctx->curveKnots.back().data();
                                    }
                                }
                                if (ctrlCount > 0 && fixups[ttArrSec].count(curTTOff + curveOff + 16)) {
                                    auto [cSec, cOff] = fixups[ttArrSec][curTTOff + curveOff + 16];
                                    if (cSec < ctx->sections.size() && cOff + ctrlCount * 4 <= ctx->sections[cSec].size()) {
                                        ctx->curveControls.emplace_back(ctrlCount);
                                        std::memcpy(ctx->curveControls.back().data(), &ctx->sections[cSec][cOff], ctrlCount * 4);
                                        curve.Controls = ctx->curveControls.back().data();
                                    }
                                }
                            };

                            parseCurve(4, ttracks[t].PositionCurve);
                            parseCurve(24, ttracks[t].OrientationCurve);
                            parseCurve(44, ttracks[t].ScaleShearCurve);
                        }

                        trackGroup->TransformTrackCount = static_cast<granny_int32>(ttracks.size());
                        trackGroup->TransformTracks = ttracks.data();
                    }

                    ctx->tgPtrs.push_back(trackGroup.get());
                    ctx->trackGroups.push_back(std::move(trackGroup));
                }

                if (ctx->tgPtrs.size() > tgStartIdx) {
                    anim->TrackGroupCount = static_cast<granny_int32>(ctx->tgPtrs.size() - tgStartIdx);
                    anim->TrackGroups = &ctx->tgPtrs[tgStartIdx];
                }
            }

            ctx->animPtrs.push_back(anim.get());
            ctx->animations.push_back(std::move(anim));
        }
        ctx->fileInfo.Animations = ctx->animPtrs.empty() ? nullptr : ctx->animPtrs.data();
        ctx->fileInfo.AnimationCount = static_cast<granny_int32>(ctx->animPtrs.size());
    }

    ctx->file.FileInfo     = &ctx->fileInfo;
    ctx->file.OwnsFileInfo = false;
    ctx->file.InternalData = ctx.get();

    return &ctx.release()->file;
}

extern "C" void GrannyFreeParsedGR2(granny_file* file)
{
    if (!file || !file->InternalData)
        return;

    auto* ctx = static_cast<GR2ParsedContext*>(file->InternalData);
    delete ctx;
}
