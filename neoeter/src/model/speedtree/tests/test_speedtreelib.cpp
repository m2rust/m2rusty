#include <Neoeter/Model/SpeedTree/StdAfx.h>
#include <Neoeter/Model/SpeedTree/SpeedTreeRTCompat.h>
#include <Neoeter/Model/SpeedTree/SpeedTreeWrapper.h>
#include <Neoeter/Model/SpeedTree/SpeedTreeForest.h>
#include <Neoeter/Model/SpeedTree/CSpeedTreeDirectX.h>
#include <Neoeter/Model/SpeedTree/BoundaryShapeManager.h>

#undef NDEBUG
#include <algorithm>
#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <vector>

namespace {

void test_speedtreert_compat()
{
    CSpeedTreeRT rt;
    rt.SetTreePosition(10.0f, 20.0f, 30.0f);
    const float* pos = rt.GetTreePosition();
    assert(pos != nullptr);
    assert(pos[0] == 10.0f && pos[1] == 20.0f && pos[2] == 30.0f);

    rt.SetTreeSize(500.0f, 10.0f);
    float size = 0.0f, variance = 0.0f;
    rt.GetTreeSize(size, variance);
    assert(size == 500.0f);

    float bounds[6] = {0};
    rt.GetBoundingBox(bounds);
    assert(bounds[5] == 500.0f);
    assert(bounds[0] < 0.0f && bounds[3] > 0.0f);

    assert(rt.Compute(nullptr, 42));
    assert(rt.GetSeed() == 42);

    // Collision cylinder
    assert(rt.GetCollisionObjectCount() >= 1);
    CSpeedTreeRT::ECollisionObjectType colType;
    float colPos[3] = {0};
    float colDims[3] = {0};
    rt.GetCollisionObject(0, colType, colPos, colDims);
    assert(colType == CSpeedTreeRT::CO_CYLINDER);
    assert(colDims[0] > 0.0f && colDims[1] > 0.0f);

    // Materials
    const float* mat = rt.GetBranchMaterial();
    assert(mat != nullptr);
    assert(mat[0] == 1.0f); // Diffuse R

    // Instancing
    CSpeedTreeRT* inst = rt.MakeInstance();
    assert(inst != nullptr);
    assert(inst->InstanceOf() == &rt);
    delete inst;

    std::printf("[PASS] test_speedtreert_compat\n");
}

void test_speedtree_wrapper_and_spt_loading()
{
    CSpeedTreeWrapper wrapper;

    // Create a synthetic SPT buffer with mock tags and texture filenames
    std::string mockSpt = "__IdvSpt_02_\x00\x00\x00\x1b" "D:\\Tree\\BeechBark.tga\x00\x00" "CompositeMapB1\x00\x00" "CompositeShadowMapB1.tga\x00\x00" "BeechLeaves_RT_1.tga";
    mockSpt += std::string("\x00\x00\x00\x00\x00\x00\x00\x00", 8);

    bool loadOk = wrapper.LoadTree("test_tree.spt", reinterpret_cast<const BYTE*>(mockSpt.data()), static_cast<unsigned int>(mockSpt.size()));
    assert(loadOk);

    const float* bbox = wrapper.GetBoundingBox();
    assert(bbox != nullptr);

    wrapper.SetPosition(50.0f, 100.0f, 20.0f);

    D3DXVECTOR3 v3Center;
    float fRadius = 0.0f;
    bool bsOk = wrapper.GetBoundingSphere(v3Center, fRadius);
    assert(bsOk);
    assert(fRadius > 0.0f);
    assert(v3Center.x == 50.0f && v3Center.y == 100.0f);

    // Test wrapper instancing
    CSpeedTreeWrapper* pInst = wrapper.MakeInstance();
    assert(pInst != nullptr);
    assert(pInst->InstanceOf() == &wrapper);

    unsigned int instCount = 0;
    CSpeedTreeWrapper** ppInstances = wrapper.GetInstances(instCount);
    assert(instCount == 1);
    assert(ppInstances[0] == pInst);

    wrapper.DeleteInstance(pInst);
    ppInstances = wrapper.GetInstances(instCount);
    assert(instCount == 0);

    std::printf("[PASS] test_speedtree_wrapper_and_spt_loading\n");
}


// Build a minimal IdvSpt parameter block: texture strings + the size property
// (id 2001 followed by a float) that the loader reads.
static std::vector<unsigned char> MakeSyntheticSpt(float fSize)
{
    std::vector<unsigned char> buf;
    auto append = [&buf](const void* p, size_t n) {
        const unsigned char* b = static_cast<const unsigned char*>(p);
        buf.insert(buf.end(), b, b + n);
    };

    const char header[] = "__IdvSpt_02_";
    append(header, sizeof(header) - 1);
    const char bark[] = "BeechBark.tga";
    append(bark, sizeof(bark));
    const char composite[] = "CompositeMap.tga";
    append(composite, sizeof(composite));

    const unsigned int sizePropertyId = 2001;
    append(&sizePropertyId, sizeof(sizePropertyId));
    append(&fSize, sizeof(fSize));

    return buf;
}

void test_speedtree_generated_geometry()
{
    const float kSize = 500.0f;
    std::vector<unsigned char> spt = MakeSyntheticSpt(kSize);

    CSpeedTreeRT rt;
    assert(rt.LoadTree(spt.data(), static_cast<unsigned int>(spt.size())));

    float size = 0.0f, variance = 0.0f;
    rt.GetTreeSize(size, variance);
    assert(std::fabs(size - kSize) < 1.0f);

    assert(rt.Compute(nullptr, 7));

    float bbox[6] = {0};
    rt.GetBoundingBox(bbox);
    assert(std::fabs(bbox[5] - kSize) < 1.0f);

    CSpeedTreeRT::SGeometry g;
    rt.GetGeometry(g);

    // Branches: one contiguous strip, valid indices, non-null arrays.
    assert(g.m_sBranches.m_usVertexCount > 8);
    assert(g.m_sBranches.m_usNumStrips == 1);
    assert(g.m_sBranches.m_pStripLengths != nullptr);
    assert(g.m_sBranches.m_pStripLengths[0] > 3);
    assert(g.m_sBranches.m_pStrips != nullptr);
    assert(g.m_sBranches.m_pStrips[0] != nullptr);
    assert(g.m_sBranches.m_pCoords != nullptr);
    assert(g.m_sBranches.m_pColors != nullptr);
    assert(g.m_sBranches.m_pTexCoords0 != nullptr);
    assert(g.m_sBranches.m_pTexCoords1 != nullptr);
    assert(g.m_sBranches.m_nDiscreteLodLevel == 0);
    assert(g.m_fBranchAlphaTestValue > 0.0f);
    assert(rt.GetBranchTriangleCount() > 0);

    for (unsigned short i = 0; i < g.m_sBranches.m_pStripLengths[0]; ++i)
        assert(g.m_sBranches.m_pStrips[0][i] < g.m_sBranches.m_usVertexCount);

    // Concave silhouette: the trunk starts at the origin and the crown is above it.
    float fMinZ = 1e9f, fMaxZ = -1e9f;
    for (unsigned int i = 0; i < g.m_sBranches.m_usVertexCount; ++i) {
        fMinZ = std::min(fMinZ, g.m_sBranches.m_pCoords[i * 3 + 2]);
        fMaxZ = std::max(fMaxZ, g.m_sBranches.m_pCoords[i * 3 + 2]);
    }
    assert(fMinZ >= 0.0f && fMinZ < 1.0f);
    assert(fMaxZ > kSize * 0.4f);

    // Leaves: active, valid per-leaf pointers, normalised UVs.
    assert(g.m_sLeaves0.m_bIsActive);
    assert(g.m_sLeaves0.m_usLeafCount > 0);
    assert(g.m_sLeaves0.m_fAlphaTestValue > 0.0f);
    assert(g.m_sLeaves0.m_nDiscreteLodLevel == 0);
    assert(g.m_sLeaves0.m_pCenterCoords != nullptr);
    assert(g.m_sLeaves0.m_pColors != nullptr);
    assert(g.m_sLeaves0.m_pLeafMapCoords != nullptr);
    assert(g.m_sLeaves0.m_pLeafMapTexCoords != nullptr);
    assert(rt.GetLeafTriangleCount() == g.m_sLeaves0.m_usLeafCount * 2);

    bool bAnyCornerOffset = false;
    for (unsigned short l = 0; l < g.m_sLeaves0.m_usLeafCount; ++l) {
        assert(g.m_sLeaves0.m_pLeafMapCoords[l] != nullptr);
        assert(g.m_sLeaves0.m_pLeafMapTexCoords[l] != nullptr);
        for (int c = 0; c < 4; ++c) {
            const float* uv = g.m_sLeaves0.m_pLeafMapTexCoords[l] + c * 2;
            assert(uv[0] >= 0.0f && uv[0] <= 1.0f);
            assert(uv[1] >= 0.0f && uv[1] <= 1.0f);
            const float* off = g.m_sLeaves0.m_pLeafMapCoords[l] + c * 4;
            if (std::fabs(off[0]) + std::fabs(off[1]) + std::fabs(off[2]) > 1e-3f)
                bAnyCornerOffset = true;
        }
    }
    assert(bAnyCornerOffset);

    // Billboards/fronds are intentionally inactive.
    assert(!g.m_sBillboard0.m_bIsActive);
    assert(!g.m_sLeaves1.m_bIsActive);
    assert(g.m_sFronds.m_usVertexCount == 0);

    // Camera-facing leaf cards: changing the camera must update the corners
    // without changing the leaf count.
    const float camA[3] = {0.0f, -400.0f, 200.0f};
    const float camB[3] = {400.0f, 0.0f, 200.0f};
    const float dir[3] = {0.0f, 1.0f, 0.0f};
    CSpeedTreeRT::SetCamera(camA, dir);
    rt.GetGeometry(g, SpeedTree_LeafGeometry);
    float savedCorner[3] = {g.m_sLeaves0.m_pLeafMapCoords[0][0],
                            g.m_sLeaves0.m_pLeafMapCoords[0][1],
                            g.m_sLeaves0.m_pLeafMapCoords[0][2]};
    const unsigned short savedCount = g.m_sLeaves0.m_usLeafCount;

    CSpeedTreeRT::SetCamera(camB, dir);
    rt.GetGeometry(g, SpeedTree_LeafGeometry);
    assert(g.m_sLeaves0.m_usLeafCount == savedCount);
    const float dx = g.m_sLeaves0.m_pLeafMapCoords[0][0] - savedCorner[0];
    const float dy = g.m_sLeaves0.m_pLeafMapCoords[0][1] - savedCorner[1];
    assert(std::fabs(dx) + std::fabs(dy) > 1e-3f);

    // Instances must produce the same topology (shared vertex buffers).
    CSpeedTreeRT* pInstance = rt.MakeInstance();
    assert(pInstance != nullptr);
    pInstance->SetTreePosition(100.0f, 50.0f, 0.0f);
    CSpeedTreeRT::SGeometry gi;
    pInstance->GetGeometry(gi);
    assert(gi.m_sBranches.m_usVertexCount == g.m_sBranches.m_usVertexCount);
    assert(gi.m_sLeaves0.m_usLeafCount == g.m_sLeaves0.m_usLeafCount);
    delete pInstance;

    // Tree height drives the collision cylinder.
    CSpeedTreeRT::ECollisionObjectType colType;
    float colPos[3] = {0}, colDims[3] = {0};
    rt.GetCollisionObject(0, colType, colPos, colDims);
    assert(colType == CSpeedTreeRT::CO_CYLINDER);
    assert(colDims[0] > 0.0f && colDims[1] > 0.0f);
    assert(std::fabs(colDims[1] - kSize * 0.60f) < 1.0f);

    std::printf("[PASS] test_speedtree_generated_geometry\n");
}

void test_speedtree_forest()
{
    CSpeedTreeDirectX directxForest;

    directxForest.SetWindStrength(0.75f);
    assert(std::fabs(directxForest.GetWindStrength() - 0.75f) < 1e-4f);

    float dir[3] = {0.0f, 1.0f, 0.0f};
    float amb[3] = {0.2f, 0.2f, 0.2f};
    float diff[3] = {0.8f, 0.8f, 0.8f};
    directxForest.SetLight(dir, amb, diff);

    directxForest.SetFog(500.0f, 2000.0f);

    directxForest.UpdateSystem(0.016f);

    directxForest.Clear();

    std::printf("[PASS] test_speedtree_forest\n");
}

void test_boundary_shape_manager()
{
    CBoundaryShapeManager bsm;
    assert(!bsm.PointInside(100.0f, 100.0f));

    float rx = 0.0f, ry = 0.0f;
    assert(!bsm.RandomPoint(rx, ry));

    std::printf("[PASS] test_boundary_shape_manager\n");
}

} // namespace

int main()
{
    test_speedtreert_compat();
    test_speedtree_wrapper_and_spt_loading();
    test_speedtree_generated_geometry();
    test_speedtree_forest();
    test_boundary_shape_manager();

    std::printf("All SpeedTreeLib tests passed successfully!\n");
    return 0;
}
