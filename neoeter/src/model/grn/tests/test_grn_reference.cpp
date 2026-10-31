// Skinning / animation reference values (TA.5).
//
// The client deforms skinned meshes and samples animation curves on the CPU;
// once GR2 assets are converted the math has to keep producing the same poses.
// These tests pin the transform hierarchy, weighted skinning and curve sampling
// against hand-computed reference values. They need no GPU and no game assets.
//
// Tests rely on assert() even in Release builds.
#ifdef NDEBUG
#undef NDEBUG
#endif

#include <Neoeter/Model/Grn/GrannyCompat.h>
#include <Neoeter/Model/Grn/StdAfx.h>

#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstring>

namespace {

const float kSqrt2Over2 = 0.70710678f;

void assert_close(float actual, float expected, float tolerance = 1e-4f)
{
    assert(std::fabs(actual - expected) <= tolerance);
}

void init_transform(granny_transform& transform, float x, float y, float z,
                    float qx = 0.0f, float qy = 0.0f, float qz = 0.0f,
                    float qw = 1.0f)
{
    std::memset(&transform, 0, sizeof(transform));
    transform.Flags = GrannyHasPosition | GrannyHasOrientation;
    transform.Position[0] = x;
    transform.Position[1] = y;
    transform.Position[2] = z;
    transform.Orientation[0] = qx;
    transform.Orientation[1] = qy;
    transform.Orientation[2] = qz;
    transform.Orientation[3] = qw;
    transform.ScaleShear[0][0] = 1.0f;
    transform.ScaleShear[1][1] = 1.0f;
    transform.ScaleShear[2][2] = 1.0f;
}

void test_world_pose_reference_rotation()
{
    granny_bone bones[2];
    std::memset(bones, 0, sizeof(bones));

    // Root: translated to (1,2,3) and rotated 90 degrees around Z.
    bones[0].Name = "Bip01";
    bones[0].ParentIndex = GrannyNoParentBone;
    init_transform(bones[0].LocalTransform, 1.0f, 2.0f, 3.0f,
                   0.0f, 0.0f, kSqrt2Over2, kSqrt2Over2);
    GrannyMathInternal::LoadIdentity4x4(bones[0].InverseWorld4x4);

    // Child: 2 units along local X, no extra rotation.
    bones[1].Name = "Bip01 Hand";
    bones[1].ParentIndex = 0;
    init_transform(bones[1].LocalTransform, 2.0f, 0.0f, 0.0f);
    GrannyMathInternal::LoadIdentity4x4(bones[1].InverseWorld4x4);

    granny_skeleton skeleton;
    std::memset(&skeleton, 0, sizeof(skeleton));
    skeleton.Name = "ReferenceSkeleton";
    skeleton.BoneCount = 2;
    skeleton.Bones = bones;

    granny_local_pose* localPose = GrannyNewLocalPose(2);
    localPose->Transforms[0] = bones[0].LocalTransform;
    localPose->Transforms[1] = bones[1].LocalTransform;

    granny_world_pose* worldPose = GrannyNewWorldPose(2);
    GrannyBuildWorldPose(&skeleton, 0, 2, localPose, nullptr, worldPose);

    // Rz(90) in Granny's row-vector layout, translation in the last row.
    const float* root = GrannyGetWorldPose4x4(worldPose, 0);
    assert(root != nullptr);
    assert_close(root[0], 0.0f);
    assert_close(root[1], 1.0f);
    assert_close(root[4], -1.0f);
    assert_close(root[5], 0.0f);
    assert_close(root[10], 1.0f);
    assert_close(root[12], 1.0f);
    assert_close(root[13], 2.0f);
    assert_close(root[14], 3.0f);

    // Child world position = parentTranslation + Rz(90) * (2,0,0).
    const float* hand = GrannyGetWorldPose4x4(worldPose, 1);
    assert(hand != nullptr);
    assert_close(hand[0], 0.0f);
    assert_close(hand[1], 1.0f);
    assert_close(hand[4], -1.0f);
    assert_close(hand[5], 0.0f);
    assert_close(hand[12], 1.0f);
    assert_close(hand[13], 4.0f);
    assert_close(hand[14], 3.0f);

    GrannyFreeWorldPose(worldPose);
    GrannyFreeLocalPose(localPose);

    std::printf("[PASS] test_world_pose_reference_rotation\n");
}

void deform_one(const granny_pwnt3432_vertex& source,
                const granny_matrix_4x4* matrices,
                granny_pnt332_vertex* result)
{
    granny_int32x boneMap[2] = {0, 1};
    granny_mesh_deformer* deformer = GrannyNewMeshDeformer(
        GrannyPWNT3432VertexType, GrannyPNT332VertexType,
        GrannyDeformPositionNormal);
    assert(deformer != nullptr);
    assert(deformer->HasBoneWeights);

    std::memset(result, 0, sizeof(*result));
    GrannyDeformVertices(deformer, boneMap,
                         reinterpret_cast<const granny_real32*>(matrices), 1,
                         &source, result);

    GrannyFreeMeshDeformer(deformer);
}

void test_skinning_reference_weights()
{
    // Bone 0 rotates 90 degrees around Z; bone 1 translates 10 units on Y.
    granny_matrix_4x4 matrices[2] = {
        {{0.0f, 1.0f, 0.0f, 0.0f},
         {-1.0f, 0.0f, 0.0f, 0.0f},
         {0.0f, 0.0f, 1.0f, 0.0f},
         {0.0f, 0.0f, 0.0f, 1.0f}},
        {{1.0f, 0.0f, 0.0f, 0.0f},
         {0.0f, 1.0f, 0.0f, 0.0f},
         {0.0f, 0.0f, 1.0f, 0.0f},
         {0.0f, 10.0f, 0.0f, 1.0f}}
    };

    granny_pwnt3432_vertex source;
    std::memset(&source, 0, sizeof(source));
    source.Position[0] = 1.0f;
    source.Position[1] = 0.0f;
    source.Position[2] = 0.0f;
    source.Normal[0] = 0.0f;
    source.Normal[1] = 1.0f;
    source.Normal[2] = 0.0f;
    source.UV[0] = 0.25f;
    source.UV[1] = 0.75f;

    granny_pnt332_vertex result;

    // Pure bone 0: (1,0,0) -> (0,1,0), normal (0,1,0) -> (-1,0,0).
    source.BoneWeights[0] = 255;
    source.BoneWeights[1] = 0;
    source.BoneIndices[0] = 0;
    source.BoneIndices[1] = 1;
    deform_one(source, matrices, &result);
    assert_close(result.Position[0], 0.0f);
    assert_close(result.Position[1], 1.0f);
    assert_close(result.Position[2], 0.0f);
    assert_close(result.Normal[0], -1.0f);
    assert_close(result.Normal[1], 0.0f);
    assert_close(result.Normal[2], 0.0f);
    assert_close(result.UV[0], 0.25f);
    assert_close(result.UV[1], 0.75f);

    // Pure bone 1: (1,0,0) -> (1,10,0), normal unchanged.
    source.BoneWeights[0] = 0;
    source.BoneWeights[1] = 255;
    deform_one(source, matrices, &result);
    assert_close(result.Position[0], 1.0f);
    assert_close(result.Position[1], 10.0f);
    assert_close(result.Position[2], 0.0f);
    assert_close(result.Normal[0], 0.0f);
    assert_close(result.Normal[1], 1.0f);
    assert_close(result.Normal[2], 0.0f);

    // 128/255 + 127/255, weighted exactly like the engine (weights are not
    // renormalised): x = 127/255, y = 128/255 + 10*127/255.
    source.BoneWeights[0] = 128;
    source.BoneWeights[1] = 127;
    deform_one(source, matrices, &result);
    assert_close(result.Position[0], 127.0f / 255.0f);
    assert_close(result.Position[1], (128.0f + 1270.0f) / 255.0f);
    assert_close(result.Position[2], 0.0f);
    assert_close(result.Normal[0], -128.0f / 255.0f);
    assert_close(result.Normal[1], 127.0f / 255.0f);
    assert_close(result.Normal[2], 0.0f);

    std::printf("[PASS] test_skinning_reference_weights\n");
}

void test_curve_sampling_reference()
{
    // Position curve: two keys, linear from (0,0,0) to (10,20,30).
    granny_real32 knots[2] = {0.0f, 1.0f};
    granny_real32 positions[6] = {0.0f, 0.0f, 0.0f, 10.0f, 20.0f, 30.0f};
    granny_curve positionCurve;
    std::memset(&positionCurve, 0, sizeof(positionCurve));
    positionCurve.Degree = 1;
    positionCurve.KnotCount = 2;
    positionCurve.Knots = knots;
    positionCurve.ControlCount = 6;
    positionCurve.Controls = positions;

    float out[3] = {0.0f, 0.0f, 0.0f};
    GrannySampleCurveVec3(positionCurve, 0.25f, out);
    assert_close(out[0], 2.5f);
    assert_close(out[1], 5.0f);
    assert_close(out[2], 7.5f);

    // Clamping before the first / after the last key.
    GrannySampleCurveVec3(positionCurve, -1.0f, out);
    assert_close(out[0], 0.0f);
    assert_close(out[1], 0.0f);
    assert_close(out[2], 0.0f);
    GrannySampleCurveVec3(positionCurve, 2.0f, out);
    assert_close(out[0], 10.0f);
    assert_close(out[1], 20.0f);
    assert_close(out[2], 30.0f);

    // Orientation curve: identity -> Rz(90); halfway is Rz(45).
    granny_real32 orientations[8] = {
        0.0f, 0.0f, 0.0f, 1.0f,
        0.0f, 0.0f, kSqrt2Over2, kSqrt2Over2
    };
    granny_curve orientationCurve;
    std::memset(&orientationCurve, 0, sizeof(orientationCurve));
    orientationCurve.Degree = 1;
    orientationCurve.KnotCount = 2;
    orientationCurve.Knots = knots;
    orientationCurve.ControlCount = 8;
    orientationCurve.Controls = orientations;

    float quat[4] = {0.0f, 0.0f, 0.0f, 1.0f};
    GrannySampleCurveQuat(orientationCurve, 0.5f, quat);
    assert_close(quat[0], 0.0f, 1e-5f);
    assert_close(quat[1], 0.0f, 1e-5f);
    assert_close(quat[2], std::sin(3.14159265f / 8.0f), 1e-5f);
    assert_close(quat[3], std::cos(3.14159265f / 8.0f), 1e-5f);

    std::printf("[PASS] test_curve_sampling_reference\n");
}

void test_animation_pose_reference()
{
    granny_bone bone;
    std::memset(&bone, 0, sizeof(bone));
    bone.Name = "Bip01";
    bone.ParentIndex = GrannyNoParentBone;
    init_transform(bone.LocalTransform, 0.0f, 0.0f, 0.0f);
    GrannyMathInternal::LoadIdentity4x4(bone.InverseWorld4x4);

    granny_skeleton skeleton;
    std::memset(&skeleton, 0, sizeof(skeleton));
    skeleton.Name = "ReferenceSkeleton";
    skeleton.BoneCount = 1;
    skeleton.Bones = &bone;

    // Keyframes: at t=0 the bone sits at x=0 facing forward; at t=1 it is at
    // x=10 and rotated 90 degrees around Z.
    granny_real32 knots[2] = {0.0f, 1.0f};
    granny_real32 positions[6] = {0.0f, 0.0f, 0.0f, 10.0f, 0.0f, 0.0f};
    granny_real32 orientations[8] = {
        0.0f, 0.0f, 0.0f, 1.0f,
        0.0f, 0.0f, kSqrt2Over2, kSqrt2Over2
    };

    granny_curve positionCurve;
    std::memset(&positionCurve, 0, sizeof(positionCurve));
    positionCurve.Degree = 1;
    positionCurve.KnotCount = 2;
    positionCurve.Knots = knots;
    positionCurve.ControlCount = 6;
    positionCurve.Controls = positions;

    granny_curve orientationCurve;
    std::memset(&orientationCurve, 0, sizeof(orientationCurve));
    orientationCurve.Degree = 1;
    orientationCurve.KnotCount = 2;
    orientationCurve.Knots = knots;
    orientationCurve.ControlCount = 8;
    orientationCurve.Controls = orientations;

    granny_curve emptyCurve;
    std::memset(&emptyCurve, 0, sizeof(emptyCurve));

    granny_transform_track track = {
        "Bip01", positionCurve, orientationCurve, emptyCurve
    };
    granny_track_group group;
    std::memset(&group, 0, sizeof(group));
    group.Name = "Bip01";
    group.TransformTrackCount = 1;
    group.TransformTracks = &track;

    granny_track_group* groups[1] = {&group};
    granny_animation animation;
    std::memset(&animation, 0, sizeof(animation));
    animation.Name = "reference_walk";
    animation.Duration = 2.0f;
    animation.TrackGroupCount = 1;
    animation.TrackGroups = groups;

    granny_model model;
    std::memset(&model, 0, sizeof(model));
    model.Name = "ReferenceModel";
    model.Skeleton = &skeleton;

    granny_model_instance* instance = GrannyInstantiateModel(&model);
    assert(instance != nullptr);

    granny_control* control =
        GrannyPlayControlledAnimation(0.0f, &animation, instance);
    assert(control != nullptr);
    GrannySetControlLoopCount(control, 0);
    GrannySetModelClock(instance, 0.25f);

    granny_local_pose* localPose = GrannyNewLocalPose(1);
    GrannySampleModelAnimations(instance, 0, 1, localPose);

    // Position is a quarter of the way from 0 to 10.
    assert_close(localPose->Transforms[0].Position[0], 2.5f);
    assert_close(localPose->Transforms[0].Position[1], 0.0f);
    assert_close(localPose->Transforms[0].Position[2], 0.0f);

    // Orientation is a quarter of the way from 0 to 90 degrees.
    assert_close(localPose->Transforms[0].Orientation[2],
                 std::sin(3.14159265f / 16.0f), 1e-5f);
    assert_close(localPose->Transforms[0].Orientation[3],
                 std::cos(3.14159265f / 16.0f), 1e-5f);

    granny_world_pose* worldPose = GrannyNewWorldPose(1);
    GrannyBuildWorldPose(&skeleton, 0, 1, localPose, nullptr, worldPose);

    const float* world = GrannyGetWorldPose4x4(worldPose, 0);
    assert(world != nullptr);
    const float c = std::cos(3.14159265f / 8.0f);
    const float s = std::sin(3.14159265f / 8.0f);
    assert_close(world[0], c, 1e-4f);
    assert_close(world[1], s, 1e-4f);
    assert_close(world[4], -s, 1e-4f);
    assert_close(world[5], c, 1e-4f);
    assert_close(world[12], 2.5f);
    assert_close(world[13], 0.0f);
    assert_close(world[14], 0.0f);

    GrannyFreeWorldPose(worldPose);
    GrannyFreeLocalPose(localPose);
    GrannyFreeModelInstance(instance);

    std::printf("[PASS] test_animation_pose_reference\n");
}

} // namespace

int main()
{
    test_world_pose_reference_rotation();
    test_skinning_reference_weights();
    test_curve_sampling_reference();
    test_animation_pose_reference();

    std::printf("All EterGrnLib reference tests passed!\n");
    return 0;
}
