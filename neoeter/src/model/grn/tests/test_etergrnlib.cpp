#include <Neoeter/Model/Grn/StdAfx.h>
#include <Neoeter/Model/Grn/Thing.h>
#include <Neoeter/Model/Grn/ThingInstance.h>
#include <Neoeter/Model/Grn/LODController.h>
#include <Neoeter/Model/Grn/Model.h>
#include <Neoeter/Model/Grn/ModelInstance.h>
#include <Neoeter/Model/Grn/Mesh.h>
#include <Neoeter/Model/Grn/Motion.h>
#include <Neoeter/Model/Grn/Material.h>
#include <Neoeter/Render/ResourceManager.h>

#undef NDEBUG
#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstring>

namespace {

void test_granny_skeleton_and_world_pose_math()
{
    // Build a 2-bone hierarchy: Root -> Hand
    granny_bone bones[2];
    std::memset(bones, 0, sizeof(bones));

    bones[0].Name = "Bip01";
    bones[0].ParentIndex = GrannyNoParentBone;
    bones[0].LocalTransform.Flags = GrannyHasPosition | GrannyHasOrientation;
    bones[0].LocalTransform.Position[0] = 10.0f;
    bones[0].LocalTransform.Position[1] = 20.0f;
    bones[0].LocalTransform.Position[2] = 30.0f;
    bones[0].LocalTransform.Orientation[3] = 1.0f;
    GrannyMathInternal::LoadIdentity4x4(bones[0].InverseWorld4x4);

    bones[1].Name = "Bip01_R_Hand";
    bones[1].ParentIndex = 0;
    bones[1].LocalTransform.Flags = GrannyHasPosition | GrannyHasOrientation;
    bones[1].LocalTransform.Position[0] = 5.0f;
    bones[1].LocalTransform.Position[1] = -3.0f;
    bones[1].LocalTransform.Position[2] = 2.0f;
    bones[1].LocalTransform.Orientation[3] = 1.0f;
    GrannyMathInternal::LoadIdentity4x4(bones[1].InverseWorld4x4);

    granny_skeleton skel;
    skel.Name = "WarriorSkeleton";
    skel.BoneCount = 2;
    skel.Bones = bones;

    granny_int32x foundIdx = -1;
    bool foundBone = GrannyFindBoneByName(&skel, "bip01_r_hand", &foundIdx);
    assert(foundBone);
    assert(foundIdx == 1);

    granny_local_pose* localPose = GrannyNewLocalPose(2);
    granny_world_pose* worldPose = GrannyNewWorldPose(2);

    localPose->Transforms[0] = bones[0].LocalTransform;
    localPose->Transforms[1] = bones[1].LocalTransform;

    GrannyBuildWorldPose(&skel, 0, 2, localPose, nullptr, worldPose);

    const float* rootMat = GrannyGetWorldPose4x4(worldPose, 0);
    assert(rootMat != nullptr);
    assert(std::fabs(rootMat[12] - 10.0f) < 1e-4f);
    assert(std::fabs(rootMat[13] - 20.0f) < 1e-4f);
    assert(std::fabs(rootMat[14] - 30.0f) < 1e-4f);

    const float* handMat = GrannyGetWorldPose4x4(worldPose, 1);
    assert(handMat != nullptr);
    assert(std::fabs(handMat[12] - 15.0f) < 1e-4f);
    assert(std::fabs(handMat[13] - 17.0f) < 1e-4f);
    assert(std::fabs(handMat[14] - 32.0f) < 1e-4f);

    GrannyFreeWorldPose(worldPose);
    GrannyFreeLocalPose(localPose);

    std::printf("[PASS] test_granny_skeleton_and_world_pose_math\n");
}

void test_granny_vertex_deformer_skinning()
{
    granny_pwnt3432_vertex srcVert;
    std::memset(&srcVert, 0, sizeof(srcVert));
    srcVert.Position[0] = 1.0f;
    srcVert.Position[1] = 2.0f;
    srcVert.Position[2] = 3.0f;
    srcVert.BoneWeights[0] = 255;
    srcVert.BoneIndices[0] = 0;
    srcVert.Normal[0] = 0.0f;
    srcVert.Normal[1] = 0.0f;
    srcVert.Normal[2] = 1.0f;
    srcVert.UV[0] = 0.25f;
    srcVert.UV[1] = 0.75f;

    granny_matrix_4x4 boneMat;
    GrannyMathInternal::LoadIdentity4x4(boneMat);
    boneMat[3][0] = 100.0f;
    boneMat[3][1] = 200.0f;
    boneMat[3][2] = 300.0f;

    granny_int32x boneMap[1] = {0};

    granny_mesh_deformer* deformer = GrannyNewMeshDeformer(
        GrannyPWNT3432VertexType,
        GrannyPNT332VertexType,
        GrannyDeformPositionNormal);
    assert(deformer != nullptr);
    assert(deformer->HasBoneWeights);

    granny_pnt332_vertex dstVert;
    std::memset(&dstVert, 0, sizeof(dstVert));

    GrannyDeformVertices(deformer, boneMap, &boneMat[0][0], 1, &srcVert, &dstVert);

    assert(std::fabs(dstVert.Position[0] - 101.0f) < 1e-3f);
    assert(std::fabs(dstVert.Position[1] - 202.0f) < 1e-3f);
    assert(std::fabs(dstVert.Position[2] - 303.0f) < 1e-3f);
    assert(std::fabs(dstVert.Normal[2] - 1.0f) < 1e-3f);
    assert(std::fabs(dstVert.UV[0] - 0.25f) < 1e-3f);
    assert(std::fabs(dstVert.UV[1] - 0.75f) < 1e-3f);

    GrannyFreeMeshDeformer(deformer);

    std::printf("[PASS] test_granny_vertex_deformer_skinning\n");
}

void test_motion_and_thing_loading()
{
    // Test CGrannyMotion text track and duration
    granny_text_track_entry entries[2] = {
        {0.15f, "sound_swing1"},
        {0.45f, "Attack_Hit"}
    };
    granny_text_track textTrack = {"Events", 2, entries};
    granny_track_group trackGroup;
    std::memset(&trackGroup, 0, sizeof(trackGroup));
    trackGroup.Name = "WarriorTrackGroup";
    trackGroup.TextTrackCount = 1;
    trackGroup.TextTracks = &textTrack;

    granny_track_group* trackGroups[1] = {&trackGroup};

    granny_animation anim;
    std::memset(&anim, 0, sizeof(anim));
    anim.Name = "combo_attack_01";
    anim.Duration = 1.25f;
    anim.TimeStep = 1.0f / 30.0f;
    anim.TrackGroupCount = 1;
    anim.TrackGroups = trackGroups;

    CGrannyMotion motion;
    assert(motion.IsEmpty());
    bool bound = motion.BindGrannyAnimation(&anim);
    assert(bound);
    assert(!motion.IsEmpty());
    assert(std::strcmp(motion.GetName(), "combo_attack_01") == 0);
    assert(std::fabs(motion.GetDuration() - 1.25f) < 1e-4f);

    int hitCount = 0;
    float hitTimes[4] = {0.0f};
    motion.GetTextTrack("attack_hit", &hitCount, hitTimes);
    assert(hitCount == 1);
    assert(std::fabs(hitTimes[0] - 0.45f) < 1e-4f);

    // Test loading a synthetic model + animation into CGraphicThing via OpenGrannyMemoryEnvelope
    granny_bone rootBone;
    std::memset(&rootBone, 0, sizeof(rootBone));
    rootBone.Name = "Bip01";
    rootBone.ParentIndex = GrannyNoParentBone;
    rootBone.LocalTransform.Flags = GrannyHasPosition | GrannyHasOrientation;
    rootBone.LocalTransform.Orientation[3] = 1.0f;
    GrannyMathInternal::LoadIdentity4x4(rootBone.InverseWorld4x4);

    granny_skeleton skel = {"TestSkel", 1, &rootBone};

    granny_pnt332_vertex triVerts[3] = {
        {{0.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 1.0f}, {0.0f, 0.0f}},
        {{10.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 1.0f}, {1.0f, 0.0f}},
        {{0.0f, 10.0f, 0.0f}, {0.0f, 0.0f, 1.0f}, {0.0f, 1.0f}}
    };
    granny_vertex_data vtxData;
    std::memset(&vtxData, 0, sizeof(vtxData));
    vtxData.VertexType = GrannyPNT332VertexType;
    vtxData.VertexCount = 3;
    vtxData.Vertices = reinterpret_cast<granny_uint8*>(triVerts);

    granny_uint16 triIndices[3] = {0, 1, 2};
    granny_tri_material_group triGroup = {0, 0, 1};
    granny_tri_topology topo;
    std::memset(&topo, 0, sizeof(topo));
    topo.GroupCount = 1;
    topo.Groups = &triGroup;
    topo.IndexCount16 = 3;
    topo.Indices16 = triIndices;

    granny_bone_binding boneBinding = {"Bip01", {-5.0f, -5.0f, -5.0f}, {15.0f, 15.0f, 5.0f}, 0, nullptr};

    granny_mesh mesh;
    std::memset(&mesh, 0, sizeof(mesh));
    mesh.Name = "SwordMesh";
    mesh.PrimaryVertexData = &vtxData;
    mesh.PrimaryTopology = &topo;
    mesh.BoneBindingCount = 1;
    mesh.BoneBindings = &boneBinding;

    granny_model_mesh_binding meshBinding = {&mesh};
    granny_model model;
    std::memset(&model, 0, sizeof(model));
    model.Name = "SwordModel";
    model.Skeleton = &skel;
    model.MeshBindingCount = 1;
    model.MeshBindings = &meshBinding;

    granny_model* models[1] = {&model};
    granny_animation* anims[1] = {&anim};

    granny_file_info fileInfo;
    std::memset(&fileInfo, 0, sizeof(fileInfo));
    fileInfo.ModelCount = 1;
    fileInfo.Models = models;
    fileInfo.AnimationCount = 1;
    fileInfo.Animations = anims;

    OpenGrannyMemoryEnvelope env = {OPEN_GRANNY_MEM_MAGIC, 1, &fileInfo};

    struct ScopedD3DContext : public CGraphicBase
    {
        ScopedD3DContext()
        {
            ms_lpd3dDevice = new IDirect3DDevice9();
            D3DXCreateMatrixStack(0, &ms_lpd3dMatStack);
            D3DXMatrixIdentity(&ms_matIdentity);
        }
        ~ScopedD3DContext()
        {
            if (ms_lpd3dMatStack)
            {
                ms_lpd3dMatStack->Release();
                ms_lpd3dMatStack = nullptr;
            }
            if (ms_lpd3dDevice)
            {
                ms_lpd3dDevice->Release();
                ms_lpd3dDevice = nullptr;
            }
        }
    } d3dCtx;

    class TestGraphicThing : public CGraphicThing
    {
    public:
        explicit TestGraphicThing(const char* name) : CGraphicThing(name) {}
        bool LoadMemory(int size, const void* buf) { return OnLoad(size, buf); }
    };

    TestGraphicThing thing("item/weapon/sword_001.gr2");
    bool loaded = thing.LoadMemory(sizeof(env), &env);
    assert(loaded);
    assert(thing.GetModelCount() == 1);
    assert(thing.GetMotionCount() == 1);
    assert(thing.GetModelPointer(0) != nullptr);
    assert(thing.GetModelPointer(0)->GetRigidVertexCount() == 3);
    assert(thing.GetMotionPointer(0) != nullptr);

    // Test CGrannyModelInstance & bone queries
    CGrannyModelInstance* modelInst = CGrannyModelInstance::New();
    assert(modelInst != nullptr);
    modelInst->SetMainModelPointer(thing.GetModelPointer(0), nullptr);

    int boneIdx = -1;
    bool gotBone = modelInst->GetBoneIndexByName("Bip01", &boneIdx);
    assert(gotBone);
    assert(boneIdx == 0);

    D3DXMATRIX worldMat;
    D3DXMatrixIdentity(&worldMat);
    modelInst->DeformNoSkin(&worldMat);

    D3DXVECTOR3 vtMin, vtMax;
    modelInst->GetBoundBox(&vtMin, &vtMax);
    assert(vtMax.x > vtMin.x);

    modelInst->SetMotionPointer(thing.GetMotionPointer(0), 0.1f, 1, 1.0f);
    assert(modelInst->IsMotionPlaying());
    modelInst->UpdateLocalTime(0.5f);
    modelInst->Update(CGrannyModelInstance::ANIFPS_MIN);

    CGrannyModelInstance::Delete(modelInst);
    CGrannyModelInstance::DestroySystem();
    thing.DestroyDeviceObjects();

    std::printf("[PASS] test_motion_and_thing_loading\n");
}

void test_native_gr2_parsing_and_model_loading()
{
    CResourceManager resMgr;

    class TestGraphicThing : public CGraphicThing
    {
    public:
        explicit TestGraphicThing(const char* name) : CGraphicThing(name) {}
        bool LoadMemory(int size, const void* buf) { return OnLoad(size, buf); }
    };

    auto try_read_file = [](const std::vector<std::string>& candidates) -> std::vector<uint8_t> {
        for (const auto& path : candidates) {
            FILE* fp = std::fopen(path.c_str(), "rb");
            if (!fp) continue;
            std::fseek(fp, 0, SEEK_END);
            long sz = std::ftell(fp);
            std::fseek(fp, 0, SEEK_SET);
            if (sz > 0) {
                std::vector<uint8_t> data(sz);
                size_t readBytes = std::fread(data.data(), 1, sz, fp);
                std::fclose(fp);
                if (readBytes == static_cast<size_t>(sz))
                    return data;
            } else {
                std::fclose(fp);
            }
        }
        return {};
    };

    // Test 1: Rigid mesh (general_obj_fence03.gr2)
    std::vector<uint8_t> fenceData = try_read_file({
        "bin/pack/Zone/ymir work/zone/b/obj/general_obj_fence03.gr2",
        "../bin/pack/Zone/ymir work/zone/b/obj/general_obj_fence03.gr2",
        "../../bin/pack/Zone/ymir work/zone/b/obj/general_obj_fence03.gr2"
    });

    if (!fenceData.empty()) {
        TestGraphicThing thingFence("general_obj_fence03.gr2");
        bool loaded = thingFence.LoadMemory(fenceData.size(), fenceData.data());
        assert(loaded);
        assert(thingFence.GetModelCount() >= 1);
        CGrannyModel* model = thingFence.GetModelPointer(0);
        assert(model != nullptr);
        assert(model->GetMeshCount() >= 1);
        assert(model->GetRigidVertexCount() > 0);
        assert(model->GetRigidVertexCount() == 59);
        assert(model->GetMeshCount() == 1);

        CGrannyModelInstance* inst = CGrannyModelInstance::New();
        assert(inst != nullptr);
        inst->SetMainModelPointer(model, nullptr);

        D3DXMATRIX worldMat;
        D3DXMatrixIdentity(&worldMat);
        inst->DeformNoSkin(&worldMat);

        D3DXVECTOR3 vtMin, vtMax;
        inst->GetBoundBox(&vtMin, &vtMax);
        assert(vtMax.x > vtMin.x);

        CGrannyModelInstance::Delete(inst);
        thingFence.DestroyDeviceObjects();
        std::printf("[PASS] test_native_gr2_parsing_and_model_loading: rigid mesh verified\n");
    }

    // Test 2: Deformable skinned character mesh (warrior_novice.gr2)
    std::vector<uint8_t> warriorData = try_read_file({
        "bin/pack/PC/ymir work/pc/warrior/warrior_novice.gr2",
        "../bin/pack/PC/ymir work/pc/warrior/warrior_novice.gr2",
        "../../bin/pack/PC/ymir work/pc/warrior/warrior_novice.gr2"
    });

    if (!warriorData.empty()) {
        TestGraphicThing thingWarrior("warrior_novice.gr2");
        bool loaded = thingWarrior.LoadMemory(warriorData.size(), warriorData.data());
        assert(loaded);
        assert(thingWarrior.GetModelCount() >= 1);
        CGrannyModel* model = thingWarrior.GetModelPointer(0);
        assert(model != nullptr);
        assert(model->GetMeshCount() >= 1);
        assert(model->GetDeformVertexCount() > 0);
        assert(model->GetDeformVertexCount() == 2207);
        assert(model->GetMeshCount() == 3);

        CGrannyModelInstance* inst = CGrannyModelInstance::New();
        assert(inst != nullptr);
        inst->SetMainModelPointer(model, nullptr);

        D3DXMATRIX worldMat;
        D3DXMatrixIdentity(&worldMat);
        inst->Deform(&worldMat);

        D3DXVECTOR3 vtMin, vtMax;
        inst->GetBoundBox(&vtMin, &vtMax);
        assert(vtMax.z > vtMin.z);

        int boneIdx = -1;
        bool foundBip = inst->GetBoneIndexByName("Bip01", &boneIdx);
        assert(foundBip);
        assert(boneIdx >= 0);

        CGrannyModelInstance::Delete(inst);
        thingWarrior.DestroyDeviceObjects();
        std::printf("[PASS] test_native_gr2_parsing_and_model_loading: deformable skinned character verified\n");
    }

    CGrannyModelInstance::DestroySystem();
}

void init_test_transform(granny_transform& transform, float x, float y, float z)
{
    std::memset(&transform, 0, sizeof(transform));
    transform.Flags = GrannyHasPosition | GrannyHasOrientation | GrannyHasScaleShear;
    transform.Position[0] = x;
    transform.Position[1] = y;
    transform.Position[2] = z;
    transform.Orientation[3] = 1.0f;
    transform.ScaleShear[0][0] = 1.0f;
    transform.ScaleShear[1][1] = 1.0f;
    transform.ScaleShear[2][2] = 1.0f;
}

granny_curve make_constant_curve(granny_real32* controls, granny_int32 controlCount)
{
    granny_curve curve;
    std::memset(&curve, 0, sizeof(curve));
    curve.Degree = 0;
    curve.KnotCount = 1;
    curve.Knots = nullptr;
    curve.ControlCount = controlCount;
    curve.Controls = controls;
    return curve;
}

void test_animation_track_group_selection_and_blending()
{
    auto make_skeleton = []() -> granny_skeleton {
        static granny_bone bones[2];
        std::memset(bones, 0, sizeof(bones));

        bones[0].Name = "Bip01";
        bones[0].ParentIndex = GrannyNoParentBone;
        init_test_transform(bones[0].LocalTransform, 0.0f, 0.0f, 0.0f);

        bones[1].Name = "Bip01 Arm";
        bones[1].ParentIndex = 0;
        init_test_transform(bones[1].LocalTransform, 0.0f, 0.0f, 10.0f);

        granny_skeleton skeleton;
        std::memset(&skeleton, 0, sizeof(skeleton));
        skeleton.Name = "Bip01";
        skeleton.BoneCount = 2;
        skeleton.Bones = bones;
        return skeleton;
    };

    granny_skeleton skeleton = make_skeleton();
    granny_curve emptyCurve;
    std::memset(&emptyCurve, 0, sizeof(emptyCurve));

    // A helper track group that appears first in the animation, like the
    // "Box01" group found in older NPC motion files.
    granny_real32 helperPosition[3] = {99.0f, 0.0f, 0.0f};
    granny_curve helperPositionCurve = make_constant_curve(helperPosition, 3);
    granny_transform_track helperTrack = {"Box01", helperPositionCurve, emptyCurve, emptyCurve};
    granny_track_group helperGroup;
    std::memset(&helperGroup, 0, sizeof(helperGroup));
    helperGroup.Name = "Box01";
    helperGroup.TransformTrackCount = 1;
    helperGroup.TransformTracks = &helperTrack;

    // The real biped track group.
    granny_real32 rootPosition[3] = {7.0f, 0.0f, 0.0f};
    granny_real32 armRotation[4] = {0.0f, 0.0f, 0.70710678f, 0.70710678f};
    granny_curve rootPositionCurve = make_constant_curve(rootPosition, 3);
    granny_curve armRotationCurve = make_constant_curve(armRotation, 4);

    granny_transform_track bipedTracks[2] = {
        {"Bip01", rootPositionCurve, emptyCurve, emptyCurve},
        {"Bip01 Arm", emptyCurve, armRotationCurve, emptyCurve}
    };
    granny_track_group bipedGroup;
    std::memset(&bipedGroup, 0, sizeof(bipedGroup));
    bipedGroup.Name = "Bip01";
    bipedGroup.TransformTrackCount = 2;
    bipedGroup.TransformTracks = bipedTracks;

    granny_track_group* trackGroups[2] = {&helperGroup, &bipedGroup};
    granny_animation animation;
    std::memset(&animation, 0, sizeof(animation));
    animation.Name = "wait";
    animation.Duration = 1.0f;
    animation.TrackGroupCount = 2;
    animation.TrackGroups = trackGroups;

    granny_model model;
    std::memset(&model, 0, sizeof(model));
    model.Name = "TestNpc";
    model.Skeleton = &skeleton;

    granny_model_instance* instance = GrannyInstantiateModel(&model);
    assert(instance != nullptr);

    granny_control* control = GrannyPlayControlledAnimation(0.0f, &animation, instance);
    assert(control != nullptr);
    GrannySetControlLoopCount(control, 0);
    GrannySetModelClock(instance, 0.5f);

    granny_local_pose* pose = GrannyNewLocalPose(2);
    GrannySampleModelAnimations(instance, 0, 2, pose);

    // The biped group must be selected instead of the first (helper) group.
    assert(std::fabs(pose->Transforms[0].Position[0] - 7.0f) < 1e-4f);
    assert(std::fabs(pose->Transforms[1].Orientation[2] - 0.70710678f) < 1e-4f);
    assert(std::fabs(pose->Transforms[1].Orientation[3] - 0.70710678f) < 1e-4f);

    GrannyFreeLocalPose(pose);
    GrannyFreeModelInstance(instance);

    // Blending test: animation A at x=0 fading out while B at x=10 fades in.
    granny_real32 posA[3] = {0.0f, 0.0f, 0.0f};
    granny_real32 posB[3] = {10.0f, 0.0f, 0.0f};

    granny_curve curveA = make_constant_curve(posA, 3);
    granny_curve curveB = make_constant_curve(posB, 3);

    granny_transform_track trackA = {"Bip01", curveA, emptyCurve, emptyCurve};
    granny_transform_track trackB = {"Bip01", curveB, emptyCurve, emptyCurve};

    granny_track_group groupA;
    std::memset(&groupA, 0, sizeof(groupA));
    groupA.Name = "Bip01";
    groupA.TransformTrackCount = 1;
    groupA.TransformTracks = &trackA;

    granny_track_group groupB;
    std::memset(&groupB, 0, sizeof(groupB));
    groupB.Name = "Bip01";
    groupB.TransformTrackCount = 1;
    groupB.TransformTracks = &trackB;

    granny_track_group* groupsA[1] = {&groupA};
    granny_track_group* groupsB[1] = {&groupB};

    granny_animation animA;
    std::memset(&animA, 0, sizeof(animA));
    animA.Name = "A";
    animA.Duration = 1.0f;
    animA.TrackGroupCount = 1;
    animA.TrackGroups = groupsA;

    granny_animation animB;
    std::memset(&animB, 0, sizeof(animB));
    animB.Name = "B";
    animB.Duration = 1.0f;
    animB.TrackGroupCount = 1;
    animB.TrackGroups = groupsB;

    instance = GrannyInstantiateModel(&model);
    assert(instance != nullptr);

    granny_control* controlA = GrannyPlayControlledAnimation(0.0f, &animA, instance);
    assert(controlA != nullptr);
    GrannySetControlLoopCount(controlA, 0);

    GrannySetControlEaseOut(controlA, true);
    GrannySetControlEaseOutCurve(controlA, 0.0f, 1.0f, 1.0f, 1.0f, 0.0f, 0.0f);
    GrannyCompleteControlAt(controlA, 1.0f);

    granny_control* controlB = GrannyPlayControlledAnimation(0.0f, &animB, instance);
    assert(controlB != nullptr);
    GrannySetControlLoopCount(controlB, 0);
    GrannySetControlEaseIn(controlB, true);
    GrannySetControlEaseInCurve(controlB, 0.0f, 1.0f, 0.0f, 0.0f, 1.0f, 1.0f);

    GrannySetModelClock(instance, 0.5f);

    pose = GrannyNewLocalPose(2);
    GrannySampleModelAnimations(instance, 0, 2, pose);

    // Halfway through the cross-fade the pose sits halfway between both clips.
    assert(std::fabs(pose->Transforms[0].Position[0] - 5.0f) < 1e-3f);

    GrannyFreeLocalPose(pose);
    GrannyFreeModelInstance(instance);

    std::printf("[PASS] test_animation_track_group_selection_and_blending\n");
}

void test_root_motion_is_not_accumulated_during_transitions()
{
    granny_bone bone;
    std::memset(&bone, 0, sizeof(bone));
    bone.Name = "Bip01";
    bone.ParentIndex = GrannyNoParentBone;
    init_test_transform(bone.LocalTransform, 0.0f, 0.0f, 0.0f);

    granny_skeleton skeleton;
    std::memset(&skeleton, 0, sizeof(skeleton));
    skeleton.Name = "Bip01";
    skeleton.BoneCount = 1;
    skeleton.Bones = &bone;

    granny_track_group group;
    std::memset(&group, 0, sizeof(group));
    group.Name = "Bip01";
    group.LoopTranslation[0] = -100.0f;

    granny_track_group* groups[1] = {&group};

    granny_animation animation;
    std::memset(&animation, 0, sizeof(animation));
    animation.Name = "run";
    animation.Duration = 1.0f;
    animation.TrackGroupCount = 1;
    animation.TrackGroups = groups;

    granny_model model;
    std::memset(&model, 0, sizeof(model));
    model.Name = "TestPlayer";
    model.Skeleton = &skeleton;

    granny_model_instance* instance = GrannyInstantiateModel(&model);
    assert(instance != nullptr);

    granny_control* controlA = GrannyPlayControlledAnimation(0.0f, &animation, instance);
    assert(controlA != nullptr);
    GrannySetControlLoopCount(controlA, 0);
    GrannySetControlEaseOut(controlA, true);
    GrannySetControlEaseOutCurve(controlA, 0.0f, 1.0f, 1.0f, 1.0f, 0.0f, 0.0f);
    GrannyCompleteControlAt(controlA, 1.0f);

    granny_control* controlB = GrannyPlayControlledAnimation(0.0f, &animation, instance);
    assert(controlB != nullptr);
    GrannySetControlLoopCount(controlB, 0);
    GrannySetControlEaseIn(controlB, true);
    GrannySetControlEaseInCurve(controlB, 0.0f, 1.0f, 0.0f, 0.0f, 1.0f, 1.0f);

    GrannySetModelClock(instance, 0.5f);

    granny_real32 matrix[16] = {
        1.0f, 0.0f, 0.0f, 0.0f,
        0.0f, 1.0f, 0.0f, 0.0f,
        0.0f, 0.0f, 1.0f, 0.0f,
        0.0f, 0.0f, 0.0f, 1.0f
    };

    GrannyUpdateModelMatrix(instance, 1.0f, matrix, matrix);

    // Both controls carry the same -100 loop translation.  With weights
    // blended and normalized the character moves -100 on local Y, not -200.
    assert(std::fabs(matrix[13] + 100.0f) < 1e-3f);

    GrannyFreeModelInstance(instance);

    // Idle -> run: the incoming run control has zero weight on the first
    // frame, but root motion must already use its full translation.
    granny_track_group idleGroup;
    std::memset(&idleGroup, 0, sizeof(idleGroup));
    idleGroup.Name = "Bip01";

    granny_track_group* idleGroups[1] = {&idleGroup};

    granny_animation idleAnimation;
    std::memset(&idleAnimation, 0, sizeof(idleAnimation));
    idleAnimation.Name = "wait";
    idleAnimation.Duration = 1.0f;
    idleAnimation.TrackGroupCount = 1;
    idleAnimation.TrackGroups = idleGroups;

    instance = GrannyInstantiateModel(&model);
    assert(instance != nullptr);

    granny_control* idleControl = GrannyPlayControlledAnimation(0.0f, &idleAnimation, instance);
    assert(idleControl != nullptr);
    GrannySetControlLoopCount(idleControl, 0);

    granny_control* startControl = GrannyPlayControlledAnimation(0.0f, &animation, instance);
    assert(startControl != nullptr);
    GrannySetControlLoopCount(startControl, 0);
    GrannySetControlEaseIn(startControl, true);
    GrannySetControlEaseInCurve(startControl, 0.0f, 1.0f, 0.0f, 0.0f, 1.0f, 1.0f);

    GrannySetModelClock(instance, 0.0f);

    granny_real32 startMatrix[16] = {
        1.0f, 0.0f, 0.0f, 0.0f,
        0.0f, 1.0f, 0.0f, 0.0f,
        0.0f, 0.0f, 1.0f, 0.0f,
        0.0f, 0.0f, 0.0f, 1.0f
    };

    GrannyUpdateModelMatrix(instance, 1.0f, startMatrix, startMatrix);
    assert(std::fabs(startMatrix[13] + 100.0f) < 1e-3f);

    GrannyFreeModelInstance(instance);

    // Run -> idle: the outgoing run control fades out across the pose blend,
    // so movement should decelerate instead of stopping abruptly.
    instance = GrannyInstantiateModel(&model);
    assert(instance != nullptr);

    granny_control* runControl = GrannyPlayControlledAnimation(0.0f, &animation, instance);
    assert(runControl != nullptr);
    GrannySetControlLoopCount(runControl, 0);

    GrannySetControlEaseOut(runControl, true);
    GrannySetControlEaseOutCurve(runControl, 0.0f, 1.0f, 1.0f, 1.0f, 0.0f, 0.0f);
    GrannyCompleteControlAt(runControl, 1.0f);

    granny_control* stopControl = GrannyPlayControlledAnimation(0.0f, &idleAnimation, instance);
    assert(stopControl != nullptr);
    GrannySetControlLoopCount(stopControl, 0);
    GrannySetControlEaseIn(stopControl, true);
    GrannySetControlEaseInCurve(stopControl, 0.0f, 1.0f, 0.0f, 0.0f, 1.0f, 1.0f);

    GrannySetModelClock(instance, 0.5f);

    granny_real32 stopMatrix[16] = {
        1.0f, 0.0f, 0.0f, 0.0f,
        0.0f, 1.0f, 0.0f, 0.0f,
        0.0f, 0.0f, 1.0f, 0.0f,
        0.0f, 0.0f, 0.0f, 1.0f
    };

    GrannyUpdateModelMatrix(instance, 1.0f, stopMatrix, stopMatrix);
    assert(std::fabs(stopMatrix[13] + 50.0f) < 1e-3f);

    GrannyFreeModelInstance(instance);

    std::printf("[PASS] test_root_motion_is_not_accumulated_during_transitions\n");
}

void test_finished_one_shot_motion_keeps_final_pose_during_blend()
{
    struct D3DContext : public CGraphicBase
    {
        D3DContext()
        {
            ms_lpd3dDevice = new IDirect3DDevice9();
            D3DXCreateMatrixStack(0, &ms_lpd3dMatStack);
            D3DXMatrixIdentity(&ms_matIdentity);
        }
        ~D3DContext()
        {
            if (ms_lpd3dMatStack)
            {
                ms_lpd3dMatStack->Release();
                ms_lpd3dMatStack = nullptr;
            }
            if (ms_lpd3dDevice)
            {
                ms_lpd3dDevice->Release();
                ms_lpd3dDevice = nullptr;
            }
        }
    } d3dCtx;

    class TestGraphicThing : public CGraphicThing
    {
    public:
        explicit TestGraphicThing(const char* name) : CGraphicThing(name) {}
        bool LoadMemory(int size, const void* buf) { return OnLoad(size, buf); }
    };

    granny_bone rootBone;
    std::memset(&rootBone, 0, sizeof(rootBone));
    rootBone.Name = "Bip01";
    rootBone.ParentIndex = GrannyNoParentBone;
    init_test_transform(rootBone.LocalTransform, 0.0f, 0.0f, 0.0f);
    GrannyMathInternal::LoadIdentity4x4(rootBone.InverseWorld4x4);

    granny_skeleton skeleton = {"TestSkel", 1, &rootBone};

    granny_pnt332_vertex triVerts[3] = {
        {{0.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 1.0f}, {0.0f, 0.0f}},
        {{10.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 1.0f}, {1.0f, 0.0f}},
        {{0.0f, 10.0f, 0.0f}, {0.0f, 0.0f, 1.0f}, {0.0f, 1.0f}}
    };
    granny_vertex_data vtxData;
    std::memset(&vtxData, 0, sizeof(vtxData));
    vtxData.VertexType = GrannyPNT332VertexType;
    vtxData.VertexCount = 3;
    vtxData.Vertices = reinterpret_cast<granny_uint8*>(triVerts);

    granny_uint16 triIndices[3] = {0, 1, 2};
    granny_tri_material_group triGroup = {0, 0, 1};
    granny_tri_topology topo;
    std::memset(&topo, 0, sizeof(topo));
    topo.GroupCount = 1;
    topo.Groups = &triGroup;
    topo.IndexCount16 = 3;
    topo.Indices16 = triIndices;

    granny_bone_binding boneBinding = {"Bip01", {-5.0f, -5.0f, -5.0f}, {15.0f, 15.0f, 5.0f}, 0, nullptr};

    granny_mesh mesh;
    std::memset(&mesh, 0, sizeof(mesh));
    mesh.Name = "TestMesh";
    mesh.PrimaryVertexData = &vtxData;
    mesh.PrimaryTopology = &topo;
    mesh.BoneBindingCount = 1;
    mesh.BoneBindings = &boneBinding;

    granny_model_mesh_binding meshBinding = {&mesh};
    granny_model model;
    std::memset(&model, 0, sizeof(model));
    model.Name = "TestModel";
    model.Skeleton = &skeleton;
    model.MeshBindingCount = 1;
    model.MeshBindings = &meshBinding;

    granny_curve emptyCurve;
    std::memset(&emptyCurve, 0, sizeof(emptyCurve));

    // The attack clip ends at z = 10, while the skeleton rest pose is at z = 0.
    granny_real32 attackPosition[3] = {0.0f, 0.0f, 10.0f};
    granny_curve attackCurve = make_constant_curve(attackPosition, 3);
    granny_transform_track attackTrack = {"Bip01", attackCurve, emptyCurve, emptyCurve};
    granny_track_group attackGroup;
    std::memset(&attackGroup, 0, sizeof(attackGroup));
    attackGroup.Name = "Bip01";
    attackGroup.TransformTrackCount = 1;
    attackGroup.TransformTracks = &attackTrack;
    granny_track_group* attackGroups[1] = {&attackGroup};

    granny_animation attackAnimation;
    std::memset(&attackAnimation, 0, sizeof(attackAnimation));
    attackAnimation.Name = "normal_attack";
    attackAnimation.Duration = 0.4f;
    attackAnimation.TrackGroupCount = 1;
    attackAnimation.TrackGroups = attackGroups;

    granny_real32 waitPosition[3] = {0.0f, 0.0f, -10.0f};
    granny_curve waitCurve = make_constant_curve(waitPosition, 3);
    granny_transform_track waitTrack = {"Bip01", waitCurve, emptyCurve, emptyCurve};
    granny_track_group waitGroup;
    std::memset(&waitGroup, 0, sizeof(waitGroup));
    waitGroup.Name = "Bip01";
    waitGroup.TransformTrackCount = 1;
    waitGroup.TransformTracks = &waitTrack;
    granny_track_group* waitGroups[1] = {&waitGroup};

    granny_animation waitAnimation;
    std::memset(&waitAnimation, 0, sizeof(waitAnimation));
    waitAnimation.Name = "wait";
    waitAnimation.Duration = 1.0f;
    waitAnimation.TrackGroupCount = 1;
    waitAnimation.TrackGroups = waitGroups;

    granny_model* models[1] = {&model};
    granny_animation* animations[1] = {&attackAnimation};

    granny_file_info fileInfo;
    std::memset(&fileInfo, 0, sizeof(fileInfo));
    fileInfo.ModelCount = 1;
    fileInfo.Models = models;
    fileInfo.AnimationCount = 1;
    fileInfo.Animations = animations;

    OpenGrannyMemoryEnvelope env = {OPEN_GRANNY_MEM_MAGIC, 1, &fileInfo};

    TestGraphicThing thing("synthetic_attack.gr2");
    assert(thing.LoadMemory(sizeof(env), &env));

    CGrannyMotion attackMotion;
    assert(attackMotion.BindGrannyAnimation(&attackAnimation));
    CGrannyMotion waitMotion;
    assert(waitMotion.BindGrannyAnimation(&waitAnimation));

    CGrannyModelInstance* instance = CGrannyModelInstance::New();
    assert(instance != nullptr);
    instance->SetMainModelPointer(thing.GetModelPointer(0), nullptr);

    // Start the one-shot attack and advance past the end of the clip, so the
    // control reports complete by clock before the transition is requested.
    instance->SetMotionPointer(&attackMotion, 0.0f, 1, 1.0f);
    assert(instance->IsMotionPlaying());
    instance->UpdateLocalTime(0.5f);
    instance->Update(CGrannyModelInstance::ANIFPS_MAX);

    // Transition to WAIT exactly like CActorInstance::CurrentMotionProcess does.
    instance->SetMotionPointer(&waitMotion, 0.1f, 0, 1.0f);

    D3DXMATRIX worldMatrix;
    D3DXMatrixIdentity(&worldMatrix);
    instance->DeformNoSkin(&worldMatrix);

    const float* rootMatrix = instance->GetBoneMatrixPointer(0);
    assert(rootMatrix != nullptr);
    // The finishing attack pose must be held while it fades out.  Falling back
    // to the rest pose here is the one-frame T-pose at the end of an attack.
    assert(std::fabs(rootMatrix[14] - 10.0f) < 1e-3f);

    CGrannyModelInstance::Delete(instance);
    thing.DestroyDeviceObjects();
    CGrannyModelInstance::DestroySystem();

    std::printf("[PASS] test_finished_one_shot_motion_keeps_final_pose_during_blend\n");
}

} // namespace

int main()
{
    test_granny_skeleton_and_world_pose_math();
    test_granny_vertex_deformer_skinning();
    test_motion_and_thing_loading();
    test_native_gr2_parsing_and_model_loading();
    test_animation_track_group_selection_and_blending();
    test_root_motion_is_not_accumulated_during_transitions();
    test_finished_one_shot_motion_keeps_final_pose_during_blend();

    std::printf("All EterGrnLib smoke tests passed!\n");
    return 0;
}
