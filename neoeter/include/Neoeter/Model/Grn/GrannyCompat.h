#pragma once

// ============================================================================
// GrannyCompat.h - Clean-Room 64-Bit Cross-Platform Replacement for Granny 3D
// ============================================================================
// Provides 64-bit safe scene graph structures, skeleton/world-pose 4x4 matrix
// math, animation control blending, mesh bone binding, and CPU vertex skinning
// for EterGrnLib without requiring the proprietary 32-bit granny2.dll SDK.

#include <Platform/Platform.h>
#include <Platform/DirectXMathCompat.h>

#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <vector>

typedef int32_t  granny_int32;
typedef int32_t  granny_int32x;
typedef uint32_t granny_uint32;
typedef uint16_t granny_uint16;
typedef uint8_t  granny_uint8;
typedef float    granny_real32;
typedef double   granny_real64;

typedef granny_real32 granny_triple[3];
typedef granny_real32 granny_quad[4];
typedef granny_real32 granny_matrix_3x3[3][3];
typedef granny_real32 granny_matrix_4x4[4][4];

#ifndef GrannyNoParentBone
#define GrannyNoParentBone (-1)
#endif

// Standard section indices used by GrannyFreeFileSection
enum granny_standard_section_index
{
    GrannyStandardMainSection            = 0,
    GrannyStandardRigidVertexSection     = 1,
    GrannyStandardRigidIndexSection      = 2,
    GrannyStandardDeformableVertexSection = 3,
    GrannyStandardDeformableIndexSection = 4,
    GrannyStandardTextureSection         = 5
};

// Reflection / Data Type Definition system
enum granny_member_type
{
    GrannyEndMember = 0,
    GrannyInlineMember,
    GrannyReferenceMember,
    GrannyReferenceToArrayMember,
    GrannyArrayOfReferencesMember,
    GrannyVariantReferenceMember,
    GrannyUnsupportedMemberType_Remove,
    GrannyReferenceToVariantArrayMember,
    GrannyStringMember,
    GrannyTransformMember,
    GrannyReal32Member,
    GrannyInt8Member,
    GrannyUInt8Member,
    GrannyBinormalInt8Member,
    GrannyNormalUInt8Member,
    GrannyInt16Member,
    GrannyUInt16Member,
    GrannyBinormalInt16Member,
    GrannyNormalUInt16Member,
    GrannyInt32Member,
    GrannyUInt32Member,
    GrannyOnePastLastMemberType,
    GrannyBool32Member = GrannyInt32Member
};

struct granny_data_type_definition
{
    granny_member_type Type = GrannyEndMember;
    const char* Name = nullptr;
    granny_data_type_definition* ReferenceType = nullptr;
    granny_int32 ArrayWidth = 0;
    granny_int32 Extra[3] = {0, 0, 0};
    uintptr_t TraversalID = 0;
};

struct granny_variant
{
    granny_data_type_definition* Type;
    void* Object;
};

// Standard Vertex Attribute Names & Vertex Formats
#define GrannyVertexPositionName           "Position"
#define GrannyVertexNormalName             "Normal"
#define GrannyVertexTangentName            "Tangent"
#define GrannyVertexBinormalName           "Binormal"
#define GrannyVertexTextureCoordinatesName "TextureCoordinates"
#define GrannyVertexColorName              "DiffuseColor"
#define GrannyVertexBoneWeightsName        "BoneWeights"
#define GrannyVertexBoneIndicesName        "BoneIndices"

struct granny_pnt332_vertex
{
    granny_real32 Position[3];
    granny_real32 Normal[3];
    granny_real32 UV[2];
};

struct granny_pwnt3432_vertex
{
    granny_real32 Position[3];
    granny_uint8  BoneWeights[4];
    granny_uint8  BoneIndices[4];
    granny_real32 Normal[3];
    granny_real32 UV[2];
};

inline granny_data_type_definition GrannyPNT332VertexType[4] =
{
    {GrannyReal32Member, GrannyVertexPositionName, nullptr, 3, {0, 0, 0}, 0},
    {GrannyReal32Member, GrannyVertexNormalName, nullptr, 3, {0, 0, 0}, 0},
    {GrannyReal32Member, GrannyVertexTextureCoordinatesName "0", nullptr, 2, {0, 0, 0}, 0},
    {GrannyEndMember, nullptr, nullptr, 0, {0, 0, 0}, 0}
};

inline granny_data_type_definition GrannyPWNT3432VertexType[6] =
{
    {GrannyReal32Member, GrannyVertexPositionName, nullptr, 3, {0, 0, 0}, 0},
    {GrannyNormalUInt8Member, GrannyVertexBoneWeightsName, nullptr, 4, {0, 0, 0}, 0},
    {GrannyUInt8Member, GrannyVertexBoneIndicesName, nullptr, 4, {0, 0, 0}, 0},
    {GrannyReal32Member, GrannyVertexNormalName, nullptr, 3, {0, 0, 0}, 0},
    {GrannyReal32Member, GrannyVertexTextureCoordinatesName "0", nullptr, 2, {0, 0, 0}, 0},
    {GrannyEndMember, nullptr, nullptr, 0, {0, 0, 0}, 0}
};

// Transforms & Skeleton
enum granny_transform_flags
{
    GrannyHasPosition   = 0x1,
    GrannyHasOrientation = 0x2,
    GrannyHasScaleShear = 0x4
};

struct granny_transform
{
    granny_uint32 Flags;
    granny_triple Position;
    granny_quad   Orientation;
    granny_triple ScaleShear[3];
};

struct granny_light_info
{
    granny_variant ExtendedData;
};

struct granny_camera_info
{
    granny_variant ExtendedData;
};

struct granny_bone
{
    const char* Name;
    granny_int32 ParentIndex;
    granny_transform LocalTransform;
    granny_matrix_4x4 InverseWorld4x4;
    granny_light_info* LightInfo;
    granny_camera_info* CameraInfo;
    granny_variant ExtendedData;
};

struct granny_skeleton
{
    const char* Name;
    granny_int32 BoneCount;
    granny_bone* Bones;
};

// Textures & Materials
enum granny_material_texture_type
{
    GrannyUnknownTextureType = 0,
    GrannyAmbientColorTexture,
    GrannyDiffuseColorTexture,
    GrannySpecularColorTexture,
    GrannySelfIlluminationTexture,
    GrannyOpacityTexture,
    GrannyBumpHeightTexture,
    GrannyReflectionTexture,
    GrannyRefractionTexture,
    GrannyDisplacementTexture,
    GrannyOnePastLastMaterialTextureType
};

struct granny_pixel_layout
{
    granny_int32 BytesPerPixel;
    granny_int32 ShiftForComponent[4];
    granny_int32 BitsForComponent[4];
};

struct granny_texture_mip_level
{
    granny_int32 Stride;
    granny_int32 Size;
    void* Pixels;
};

struct granny_texture_image
{
    granny_int32 MIPLevelCount;
    granny_texture_mip_level* MIPLevels;
};

struct granny_texture
{
    const char* FromFileName;
    granny_int32 TextureType;
    granny_int32 Width;
    granny_int32 Height;
    granny_int32 Encoding;
    granny_int32 SubFormat;
    granny_pixel_layout Layout;
    granny_int32 ImageCount;
    granny_texture_image* Images;
    granny_variant ExtendedData;
};

struct granny_material;

struct granny_material_map
{
    const char* Usage;
    granny_material* Material;
};

struct granny_material
{
    const char* Name;
    granny_int32 MapCount;
    granny_material_map* Maps;
    granny_texture* Texture;
    granny_variant ExtendedData;
};

struct granny_material_binding
{
    granny_material* Material;
};

// Meshes & Models
struct granny_bone_binding
{
    const char* BoneName;
    granny_triple OBBMin;
    granny_triple OBBMax;
    granny_int32 TriangleCount;
    granny_int32* TriangleIndices;
};

struct granny_tri_material_group
{
    granny_int32 MaterialIndex;
    granny_int32 TriFirst;
    granny_int32 TriCount;
};

struct granny_tri_annotation_set
{
    const char* Name;
    granny_data_type_definition* TriAnnotationType;
    granny_int32 TriAnnotationCount;
    granny_uint8* TriAnnotations;
    granny_int32 TriAnnotationIndexCount;
    granny_int32* TriAnnotationIndices;
};

struct granny_tri_topology
{
    granny_int32 GroupCount;
    granny_tri_material_group* Groups;
    granny_int32 IndexCount;
    granny_int32* Indices;
    granny_int32 IndexCount16;
    granny_uint16* Indices16;
    granny_int32 VertexToVertexCount;
    granny_int32* VertexToVertexMap;
    granny_int32 VertexToTriangleCount;
    granny_int32* VertexToTriangleMap;
    granny_int32 SideToNeighborCount;
    granny_uint32* SideToNeighborMap;
    granny_int32 BonesForTriangleCount;
    granny_int32* BonesForTriangle;
    granny_int32 TriangleToBoneCount;
    granny_int32* TriangleToBoneIndices;
    granny_int32 TriAnnotationSetCount;
    granny_tri_annotation_set* TriAnnotationSets;
};

struct granny_vertex_annotation_set
{
    const char* Name;
    granny_data_type_definition* VertexAnnotationType;
    granny_int32 VertexAnnotationCount;
    granny_uint8* VertexAnnotations;
    granny_int32 VertexAnnotationIndexCount;
    granny_int32* VertexAnnotationIndices;
};

struct granny_vertex_data
{
    granny_data_type_definition* VertexType;
    granny_int32 VertexCount;
    granny_uint8* Vertices;
    granny_int32 VertexComponentNameCount;
    const char** VertexComponentNames;
    granny_int32 VertexAnnotationSetCount;
    granny_vertex_annotation_set* VertexAnnotationSets;
};

struct granny_morph_target
{
    const char* ScalarName;
    granny_vertex_data* VertexData;
};

struct granny_mesh
{
    const char* Name;
    granny_vertex_data* PrimaryVertexData;
    granny_int32 MorphTargetCount;
    granny_morph_target* MorphTargets;
    granny_tri_topology* PrimaryTopology;
    granny_int32 MaterialBindingCount;
    granny_material_binding* MaterialBindings;
    granny_int32 BoneBindingCount;
    granny_bone_binding* BoneBindings;
    granny_variant ExtendedData;
};

struct granny_model_mesh_binding
{
    granny_mesh* Mesh;
};

struct granny_model
{
    const char* Name;
    granny_skeleton* Skeleton;
    granny_transform InitialPlacement;
    granny_int32 MeshBindingCount;
    granny_model_mesh_binding* MeshBindings;
};

// Animations & Track Groups
struct granny_curve
{
    granny_int32 Degree;
    granny_int32 KnotCount;
    granny_real32* Knots;
    granny_int32 ControlCount;
    granny_real32* Controls;
};

struct granny_scalar_track
{
    const char* Name;
    granny_curve ValueCurve;
};

struct granny_transform_track
{
    const char* Name;
    granny_curve PositionCurve;
    granny_curve OrientationCurve;
    granny_curve ScaleShearCurve;
};

struct granny_text_track_entry
{
    granny_real32 TimeStamp;
    const char* Text;
};

struct granny_text_track
{
    const char* Name;
    granny_int32 EntryCount;
    granny_text_track_entry* Entries;
};

struct granny_periodic_loop;

struct granny_track_group
{
    const char* Name;
    granny_int32 ScalarTrackCount;
    granny_scalar_track* ScalarTracks;
    granny_int32 TransformTrackCount;
    granny_transform_track* TransformTracks;
    granny_int32 TextTrackCount;
    granny_text_track* TextTracks;
    granny_transform InitialPlacement;
    granny_int32 Flags;
    granny_triple LoopTranslation;
    granny_periodic_loop* PeriodicLoop;
    granny_transform_track* RootMotion;
    granny_variant ExtendedData;
};

struct granny_animation
{
    const char* Name;
    granny_real32 Duration;
    granny_real32 TimeStep;
    granny_real32 Oversampling;
    granny_int32 TrackGroupCount;
    granny_track_group** TrackGroups;
};

// File Info & Container
struct granny_art_tool_info;
struct granny_exporter_info;

struct granny_file_info
{
    granny_art_tool_info* ArtToolInfo;
    granny_exporter_info* ExporterInfo;
    const char* FromFileName;
    granny_int32 TextureCount;
    granny_texture** Textures;
    granny_int32 MaterialCount;
    granny_material** Materials;
    granny_int32 SkeletonCount;
    granny_skeleton** Skeletons;
    granny_int32 VertexDataCount;
    granny_vertex_data** VertexDatas;
    granny_int32 TriTopologyCount;
    granny_tri_topology** TriTopologies;
    granny_int32 MeshCount;
    granny_mesh** Meshes;
    granny_int32 ModelCount;
    granny_model** Models;
    granny_int32 TrackGroupCount;
    granny_track_group** TrackGroups;
    granny_int32 AnimationCount;
    granny_animation** Animations;
    granny_variant ExtendedData;
};

// Magic header for in-memory / pre-parsed granny_file_info envelopes ("OGR2")
static constexpr uint32_t OPEN_GRANNY_MEM_MAGIC = 0x3252474Fu; // "OGR2"

struct OpenGrannyMemoryEnvelope
{
    uint32_t magic;
    uint32_t version;
    granny_file_info* fileInfo;
};

struct granny_file
{
    bool IsByteReversed;
    void* Header;
    granny_int32x SectionCount;
    void** Sections;
    bool* Marshalled;
    void* ConversionBuffer;
    granny_file_info* FileInfo;
    bool OwnsFileInfo;
    void* InternalData = nullptr;
};

// Logging & System Clock
enum granny_log_message_type
{
    GrannyIgnoredLogMessage = 0,
    GrannyNoteLogMessage,
    GrannyWarningLogMessage,
    GrannyErrorLogMessage,
    GrannyOnePastLastMessageType
};

enum granny_log_message_origin
{
    GrannyNotImplementedLogMessage = 0,
    GrannyApplicationLogMessage,
    GrannyOnePastLastMessageOrigin
};

typedef void granny_log_function(
    granny_log_message_type Type,
    granny_log_message_origin Origin,
    const char* Error,
    void* UserData);

struct granny_log_callback
{
    granny_log_function* Function;
    void* UserData;
};

inline void GrannySetLogCallback(const granny_log_callback* /*LogCallback*/) {}

struct granny_system_clock
{
    granny_uint32 Data[4];
};

inline granny_system_clock GrannyGetSystemSeconds()
{
    granny_system_clock clk = {{0, 0, 0, 0}};
    clk.Data[0] = GetTickCount();
    return clk;
}

inline granny_real32 GrannyGetSecondsElapsed(
    const granny_system_clock* StartClock,
    const granny_system_clock* EndClock)
{
    if (!StartClock || !EndClock)
        return 0.0f;
    return float(EndClock->Data[0] - StartClock->Data[0]) * 0.001f;
}

// ============================================================================
// Runtime Stateful Structures (Poses, Controls, Model Instances, Deformers)
// ============================================================================

struct granny_local_pose
{
    granny_int32 BoneCount;
    std::vector<granny_transform> Transforms;
};

struct GrannyMatrix4x4Box
{
    granny_matrix_4x4 m;
};

struct granny_world_pose
{
    granny_int32 BoneCount;
    std::vector<GrannyMatrix4x4Box> World4x4;
    std::vector<GrannyMatrix4x4Box> Composite4x4;
};

struct granny_control
{
    const granny_animation* Animation = nullptr;
    granny_real32 StartTime       = 0.0f;
    granny_real32 RawLocalClock   = 0.0f;
    granny_real32 Speed           = 1.0f;
    granny_int32  LoopCount       = 1;
    bool          EaseIn          = false;
    bool          EaseOut         = false;
    granny_real32 EaseInStart     = 0.0f;
    granny_real32 EaseInEnd       = 0.0f;
    granny_real32 EaseOutStart    = 0.0f;
    granny_real32 EaseOutEnd      = 0.0f;
    bool          HasCompleteTime = false;
    granny_real32 CompleteTime    = 0.0f;
    bool          FreeOnceUnused  = false;
    bool          IsCompleted     = false;

    // Cached binding between the animation track group and the skeleton this
    // control is playing on.  GR2 animations may contain several track groups
    // (for example a helper "Box01" group followed by the real "Bip01" group),
    // so the correct group has to be resolved per skeleton.
    bool                        TrackGroupResolved = false;
    const granny_skeleton*      CachedSkeleton     = nullptr;
    const granny_track_group*   CachedTrackGroup   = nullptr;
    // Bone index -> index in CachedTrackGroup->TransformTracks (-1 if absent).
    std::vector<granny_int32>   BoneTrackIndices;
};

struct granny_model_instance
{
    const granny_model* SourceModel = nullptr;
    granny_real32 ModelClock        = 0.0f;
    std::vector<granny_control*> Controls;
};

struct granny_mesh_binding
{
    const granny_mesh* Mesh = nullptr;
    const granny_skeleton* FromSkeleton = nullptr;
    const granny_skeleton* ToSkeleton = nullptr;
    std::vector<granny_int32> BoneIndices;
};

enum granny_deformation_type
{
    GrannyDeformPosition = 1,
    GrannyDeformPositionNormal,
    GrannyDeformPositionNormalTangent,
    GrannyDeformPositionNormalTangentBinormal
};

struct granny_mesh_deformer
{
    const granny_data_type_definition* InputType  = nullptr;
    const granny_data_type_definition* OutputType = nullptr;
    granny_deformation_type DeformationType       = GrannyDeformPositionNormal;
    bool HasBoneWeights                           = false;
    size_t InputStride                            = sizeof(granny_pnt332_vertex);
    size_t OutputStride                           = sizeof(granny_pnt332_vertex);
};

// ============================================================================
// Reflection & Type Size Helpers
// ============================================================================

inline granny_int32x GrannyGetTotalObjectSize(const granny_data_type_definition* TypeDef)
{
    if (!TypeDef)
        return 0;
    granny_int32x total = 0;
    for (int i = 0; TypeDef[i].Type != GrannyEndMember; ++i)
    {
        granny_int32 width = TypeDef[i].ArrayWidth > 0 ? TypeDef[i].ArrayWidth : 1;
        switch (TypeDef[i].Type)
        {
            case GrannyReal32Member:
            case GrannyInt32Member:
            case GrannyUInt32Member:
                total += 4 * width;
                break;
            case GrannyInt16Member:
            case GrannyUInt16Member:
            case GrannyBinormalInt16Member:
            case GrannyNormalUInt16Member:
                total += 2 * width;
                break;
            case GrannyInt8Member:
            case GrannyUInt8Member:
            case GrannyBinormalInt8Member:
            case GrannyNormalUInt8Member:
                total += 1 * width;
                break;
            default:
                total += 4 * width;
                break;
        }
    }
    return total;
}

inline granny_int32x GrannyGetTotalTypeSize(const granny_data_type_definition* TypeDef)
{
    if (!TypeDef)
        return 0;
    // Note: CGrannyModel::LoadMeshs divides GrannyGetTotalTypeSize(VertexType) by 32
    // (the 32-bit size of granny_data_type_definition) to get the member count.
    granny_int32x count = 0;
    while (TypeDef[count].Type != GrannyEndMember)
        ++count;
    return count * 32;
}

inline granny_variant GrannyFindMatchingMember(
    const granny_data_type_definition* TypeDef,
    void* Object,
    const char* FieldName)
{
    granny_variant result = {nullptr, nullptr};
    if (!TypeDef || !Object || !FieldName)
        return result;

    uint8_t* ptr = static_cast<uint8_t*>(Object);
    for (int i = 0; TypeDef[i].Type != GrannyEndMember; ++i)
    {
        if (TypeDef[i].Name && _stricmp(TypeDef[i].Name, FieldName) == 0)
        {
            result.Type = const_cast<granny_data_type_definition*>(&TypeDef[i]);
            result.Object = ptr;
            return result;
        }
        granny_int32 width = TypeDef[i].ArrayWidth > 0 ? TypeDef[i].ArrayWidth : 1;
        ptr += 4 * width;
    }
    return result;
}

inline void GrannyConvertSingleObject(
    const granny_data_type_definition* SourceType,
    const void* SourceObject,
    const granny_data_type_definition* DestType,
    void* DestObject)
{
    if (!SourceType || !SourceObject || !DestType || !DestObject)
        return;

    // Copy matching scalar fields
    if (SourceType->Type == GrannyInt32Member && DestType[0].Type == GrannyInt32Member)
    {
        *static_cast<granny_int32*>(DestObject) = *static_cast<const granny_int32*>(SourceObject);
    }
}

// Forward declarations for native GR2 parser
#ifdef __cplusplus
extern "C" {
#endif
granny_file* GrannyParseGR2FromMemory(int size, const void* memory);
void GrannyFreeParsedGR2(granny_file* file);
#ifdef __cplusplus
}
#endif

// ============================================================================
// File Loading & Memory Management
// ============================================================================

inline granny_file* GrannyReadEntireFileFromMemory(int MemorySize, void* Memory)
{
    if (!Memory || MemorySize <= 0)
        return nullptr;

    if (static_cast<size_t>(MemorySize) >= sizeof(OpenGrannyMemoryEnvelope))
    {
        const OpenGrannyMemoryEnvelope* env = static_cast<const OpenGrannyMemoryEnvelope*>(Memory);
        if (env->magic == OPEN_GRANNY_MEM_MAGIC && env->fileInfo != nullptr)
        {
            granny_file* file = new granny_file();
            std::memset(file, 0, sizeof(granny_file));
            file->FileInfo = env->fileInfo;
            file->OwnsFileInfo = false;
            return file;
        }
    }

    // Attempt to parse binary .gr2 container
    granny_file* parsedFile = GrannyParseGR2FromMemory(MemorySize, Memory);
    if (parsedFile)
        return parsedFile;

    // Fallback to empty valid file_info container if parsing is not applicable
    granny_file* file = new granny_file();
    std::memset(file, 0, sizeof(granny_file));
    file->FileInfo = new granny_file_info();
    std::memset(file->FileInfo, 0, sizeof(granny_file_info));
    file->OwnsFileInfo = true;
    return file;
}

inline granny_file_info* GrannyGetFileInfo(granny_file* File)
{
    if (!File)
        return nullptr;
    return File->FileInfo;
}

inline void GrannyFreeFileSection(granny_file* /*File*/, int /*SectionIndex*/)
{
}

inline void GrannyFreeFile(granny_file* File)
{
    if (!File)
        return;

    if (File->InternalData)
    {
        GrannyFreeParsedGR2(File);
        return;
    }

    if (File->OwnsFileInfo && File->FileInfo)
        delete File->FileInfo;
    delete File;
}

// ============================================================================
// Material & Mesh Queries
// ============================================================================

inline granny_texture* GrannyGetMaterialTextureByType(
    const granny_material* Material,
    granny_material_texture_type Type)
{
    if (!Material)
        return nullptr;

    if (Material->Texture)
        return Material->Texture;

    for (int i = 0; i < Material->MapCount; ++i)
    {
        const granny_material_map& map = Material->Maps[i];
        if (!map.Material)
            continue;
        if (Type == GrannyDiffuseColorTexture && map.Usage &&
            (_stricmp(map.Usage, "Diffuse Color") == 0 || _stricmp(map.Usage, "color") == 0))
        {
            if (map.Material->Texture)
                return map.Material->Texture;
        }
        else if (Type == GrannyOpacityTexture && map.Usage &&
                 (_stricmp(map.Usage, "Opacity") == 0 || _stricmp(map.Usage, "Filter Color") == 0))
        {
            if (map.Material->Texture)
                return map.Material->Texture;
        }
    }

    if (Type == GrannyDiffuseColorTexture && Material->MapCount > 0 && Material->Maps[0].Material)
        return Material->Maps[0].Material->Texture;

    return nullptr;
}

inline bool GrannyMeshIsRigid(const granny_mesh* Mesh)
{
    if (!Mesh)
        return true;
    return Mesh->BoneBindingCount <= 1;
}

inline granny_int32x GrannyGetMeshVertexCount(const granny_mesh* Mesh)
{
    if (!Mesh || !Mesh->PrimaryVertexData)
        return 0;
    return Mesh->PrimaryVertexData->VertexCount;
}

inline void* GrannyGetMeshVertices(const granny_mesh* Mesh)
{
    if (!Mesh || !Mesh->PrimaryVertexData)
        return nullptr;
    return Mesh->PrimaryVertexData->Vertices;
}

inline granny_data_type_definition* GrannyGetMeshVertexType(const granny_mesh* Mesh)
{
    if (!Mesh || !Mesh->PrimaryVertexData)
        return GrannyPNT332VertexType;
    return Mesh->PrimaryVertexData->VertexType ? Mesh->PrimaryVertexData->VertexType : GrannyPNT332VertexType;
}

inline void GrannyCopyMeshVertices(
    const granny_mesh* Mesh,
    const granny_data_type_definition* VertexType,
    void* DestVertices)
{
    if (!Mesh || !Mesh->PrimaryVertexData || !Mesh->PrimaryVertexData->Vertices || !DestVertices)
        return;

    granny_int32x count = Mesh->PrimaryVertexData->VertexCount;
    granny_int32x dstStride = GrannyGetTotalObjectSize(VertexType);
    if (dstStride <= 0)
        dstStride = sizeof(granny_pnt332_vertex);

    granny_int32x srcStride = GrannyGetTotalObjectSize(Mesh->PrimaryVertexData->VertexType);
    if (srcStride <= 0)
        srcStride = dstStride;

    if (srcStride == dstStride)
    {
        std::memcpy(DestVertices, Mesh->PrimaryVertexData->Vertices, static_cast<size_t>(count) * static_cast<size_t>(dstStride));
    }
    else
    {
        const uint8_t* src = static_cast<const uint8_t*>(Mesh->PrimaryVertexData->Vertices);
        uint8_t* dst = static_cast<uint8_t*>(DestVertices);
        if (srcStride >= 40 && dstStride == sizeof(granny_pnt332_vertex))
        {
            for (int i = 0; i < count; ++i)
            {
                const granny_pwnt3432_vertex* s = reinterpret_cast<const granny_pwnt3432_vertex*>(src + i * srcStride);
                granny_pnt332_vertex* d = reinterpret_cast<granny_pnt332_vertex*>(dst + i * dstStride);
                std::memcpy(d->Position, s->Position, sizeof(float) * 3);
                std::memcpy(d->Normal, s->Normal, sizeof(float) * 3);
                std::memcpy(d->UV, s->UV, sizeof(float) * 2);
            }
        }
        else
        {
            size_t copyBytes = static_cast<size_t>(srcStride < dstStride ? srcStride : dstStride);
            for (int i = 0; i < count; ++i)
            {
                std::memset(dst + i * dstStride, 0, static_cast<size_t>(dstStride));
                std::memcpy(dst + i * dstStride, src + i * srcStride, copyBytes);
            }
        }
    }
}

inline granny_int32x GrannyGetMeshIndexCount(const granny_mesh* Mesh)
{
    if (!Mesh || !Mesh->PrimaryTopology)
        return 0;
    if (Mesh->PrimaryTopology->IndexCount16 > 0)
        return Mesh->PrimaryTopology->IndexCount16;
    return Mesh->PrimaryTopology->IndexCount;
}

inline void GrannyCopyMeshIndices(
    const granny_mesh* Mesh,
    granny_int32x BytesPerIndex,
    void* DestIndices)
{
    if (!Mesh || !Mesh->PrimaryTopology || !DestIndices)
        return;

    const granny_tri_topology* topo = Mesh->PrimaryTopology;
    granny_int32x count = GrannyGetMeshIndexCount(Mesh);
    if (count <= 0)
        return;

    if (BytesPerIndex == 2)
    {
        uint16_t* dst16 = static_cast<uint16_t*>(DestIndices);
        if (topo->Indices16)
        {
            std::memcpy(dst16, topo->Indices16, static_cast<size_t>(count) * sizeof(uint16_t));
        }
        else if (topo->Indices)
        {
            for (int i = 0; i < count; ++i)
                dst16[i] = static_cast<uint16_t>(topo->Indices[i]);
        }
    }
    else if (BytesPerIndex == 4)
    {
        uint32_t* dst32 = static_cast<uint32_t*>(DestIndices);
        if (topo->Indices)
        {
            std::memcpy(dst32, topo->Indices, static_cast<size_t>(count) * sizeof(uint32_t));
        }
        else if (topo->Indices16)
        {
            for (int i = 0; i < count; ++i)
                dst32[i] = static_cast<uint32_t>(topo->Indices16[i]);
        }
    }
}

inline granny_int32x GrannyGetMeshTriangleGroupCount(const granny_mesh* Mesh)
{
    if (!Mesh || !Mesh->PrimaryTopology)
        return 0;
    return Mesh->PrimaryTopology->GroupCount;
}

inline granny_tri_material_group* GrannyGetMeshTriangleGroups(const granny_mesh* Mesh)
{
    if (!Mesh || !Mesh->PrimaryTopology)
        return nullptr;
    return Mesh->PrimaryTopology->Groups;
}

// ============================================================================
// Skeleton, Mesh Binding & Bone Queries
// ============================================================================

inline bool GrannyFindBoneByName(
    const granny_skeleton* Skeleton,
    const char* BoneName,
    granny_int32x* BoneIndex)
{
    if (!Skeleton || !BoneName || !BoneIndex)
        return false;

    for (int i = 0; i < Skeleton->BoneCount; ++i)
    {
        if (Skeleton->Bones[i].Name && _stricmp(Skeleton->Bones[i].Name, BoneName) == 0)
        {
            *BoneIndex = i;
            return true;
        }
    }
    return false;
}

inline granny_mesh_binding* GrannyNewMeshBinding(
    const granny_mesh* Mesh,
    const granny_skeleton* FromSkeleton,
    const granny_skeleton* ToSkeleton)
{
    granny_mesh_binding* binding = new granny_mesh_binding();
    binding->Mesh = Mesh;
    binding->FromSkeleton = FromSkeleton;
    binding->ToSkeleton = ToSkeleton;

    int count = (Mesh && Mesh->BoneBindingCount > 0) ? Mesh->BoneBindingCount : 1;
    binding->BoneIndices.resize(static_cast<size_t>(count), 0);

    if (Mesh && Mesh->BoneBindings && ToSkeleton)
    {
        for (int i = 0; i < Mesh->BoneBindingCount; ++i)
        {
            granny_int32x idx = 0;
            if (Mesh->BoneBindings[i].BoneName &&
                GrannyFindBoneByName(ToSkeleton, Mesh->BoneBindings[i].BoneName, &idx))
            {
                binding->BoneIndices[static_cast<size_t>(i)] = idx;
            }
            else if (i < ToSkeleton->BoneCount)
            {
                binding->BoneIndices[static_cast<size_t>(i)] = i;
            }
        }
    }
    return binding;
}

inline void GrannyFreeMeshBinding(granny_mesh_binding* Binding)
{
    delete Binding;
}

inline int* GrannyGetMeshBindingToBoneIndices(const granny_mesh_binding* Binding)
{
    if (!Binding || Binding->BoneIndices.empty())
        return nullptr;
    return const_cast<int*>(Binding->BoneIndices.data());
}

// ============================================================================
// Local Pose & World Pose 4x4 Matrix Hierarchy
// ============================================================================

namespace GrannyMathInternal
{
    inline void LoadIdentity4x4(granny_matrix_4x4& m)
    {
        std::memset(m, 0, sizeof(granny_matrix_4x4));
        m[0][0] = 1.0f;
        m[1][1] = 1.0f;
        m[2][2] = 1.0f;
        m[3][3] = 1.0f;
    }

    inline void Multiply4x4(granny_matrix_4x4& out, const granny_matrix_4x4& a, const granny_matrix_4x4& b)
    {
        granny_matrix_4x4 tmp;
        for (int r = 0; r < 4; ++r)
        {
            for (int c = 0; c < 4; ++c)
            {
                tmp[r][c] =
                    a[r][0] * b[0][c] +
                    a[r][1] * b[1][c] +
                    a[r][2] * b[2][c] +
                    a[r][3] * b[3][c];
            }
        }
        std::memcpy(out, tmp, sizeof(granny_matrix_4x4));
    }

    inline void TransformToMatrix4x4(const granny_transform& t, granny_matrix_4x4& out)
    {
        LoadIdentity4x4(out);

        float qx = 0.0f, qy = 0.0f, qz = 0.0f, qw = 1.0f;
        if (t.Flags & GrannyHasOrientation)
        {
            qx = t.Orientation[0];
            qy = t.Orientation[1];
            qz = t.Orientation[2];
            qw = t.Orientation[3];
        }

        // Rotation matrix (row-vector convention matching DirectX / Granny)
        float xx = qx * qx, yy = qy * qy, zz = qz * qz;
        float xy = qx * qy, xz = qx * qz, yz = qy * qz;
        float wx = qw * qx, wy = qw * qy, wz = qw * qz;

        float rot[3][3];
        rot[0][0] = 1.0f - 2.0f * (yy + zz);
        rot[0][1] = 2.0f * (xy + wz);
        rot[0][2] = 2.0f * (xz - wy);

        rot[1][0] = 2.0f * (xy - wz);
        rot[1][1] = 1.0f - 2.0f * (xx + zz);
        rot[1][2] = 2.0f * (yz + wx);

        rot[2][0] = 2.0f * (xz + wy);
        rot[2][1] = 2.0f * (yz - wx);
        rot[2][2] = 1.0f - 2.0f * (xx + yy);

        if (t.Flags & GrannyHasScaleShear)
        {
            for (int r = 0; r < 3; ++r)
            {
                for (int c = 0; c < 3; ++c)
                {
                    out[r][c] =
                        t.ScaleShear[r][0] * rot[0][c] +
                        t.ScaleShear[r][1] * rot[1][c] +
                        t.ScaleShear[r][2] * rot[2][c];
                }
            }
        }
        else
        {
            for (int r = 0; r < 3; ++r)
                for (int c = 0; c < 3; ++c)
                    out[r][c] = rot[r][c];
        }

        if (t.Flags & GrannyHasPosition)
        {
            out[3][0] = t.Position[0];
            out[3][1] = t.Position[1];
            out[3][2] = t.Position[2];
        }
    }

    // Blends Src into Dst.  T is the weight of Src in the resulting transform.
    inline void BlendTransform(granny_transform& Dst, const granny_transform& Src, granny_real32 T)
    {
        if (T <= 0.0f)
            return;
        if (T >= 1.0f)
        {
            Dst = Src;
            return;
        }

        Dst.Position[0] += (Src.Position[0] - Dst.Position[0]) * T;
        Dst.Position[1] += (Src.Position[1] - Dst.Position[1]) * T;
        Dst.Position[2] += (Src.Position[2] - Dst.Position[2]) * T;

        const bool dstHasOrientation = (Dst.Flags & GrannyHasOrientation) != 0;
        const bool srcHasOrientation = (Src.Flags & GrannyHasOrientation) != 0;
        if (dstHasOrientation || srcHasOrientation)
        {
            D3DXQUATERNION q0 = dstHasOrientation
                ? D3DXQUATERNION(Dst.Orientation[0], Dst.Orientation[1], Dst.Orientation[2], Dst.Orientation[3])
                : D3DXQUATERNION(0.0f, 0.0f, 0.0f, 1.0f);
            D3DXQUATERNION q1 = srcHasOrientation
                ? D3DXQUATERNION(Src.Orientation[0], Src.Orientation[1], Src.Orientation[2], Src.Orientation[3])
                : D3DXQUATERNION(0.0f, 0.0f, 0.0f, 1.0f);
            D3DXQUATERNION outQ;
            D3DXQuaternionSlerp(&outQ, &q0, &q1, T);
            Dst.Orientation[0] = outQ.x;
            Dst.Orientation[1] = outQ.y;
            Dst.Orientation[2] = outQ.z;
            Dst.Orientation[3] = outQ.w;
        }

        for (int r = 0; r < 3; ++r)
        {
            for (int c = 0; c < 3; ++c)
                Dst.ScaleShear[r][c] += (Src.ScaleShear[r][c] - Dst.ScaleShear[r][c]) * T;
        }

        Dst.Flags |= Src.Flags;
    }
} // namespace GrannyMathInternal

inline granny_local_pose* GrannyNewLocalPose(granny_int32x BoneCount)
{
    granny_local_pose* pose = new granny_local_pose();
    int count = BoneCount > 0 ? BoneCount : 1;
    pose->BoneCount = count;
    pose->Transforms.resize(static_cast<size_t>(count));
    for (int i = 0; i < count; ++i)
    {
        granny_transform& tr = pose->Transforms[static_cast<size_t>(i)];
        std::memset(&tr, 0, sizeof(granny_transform));
        tr.Orientation[3] = 1.0f;
        tr.ScaleShear[0][0] = 1.0f;
        tr.ScaleShear[1][1] = 1.0f;
        tr.ScaleShear[2][2] = 1.0f;
    }
    return pose;
}

inline void GrannyFreeLocalPose(granny_local_pose* LocalPose)
{
    delete LocalPose;
}

inline granny_world_pose* GrannyNewWorldPose(granny_int32x BoneCount)
{
    granny_world_pose* pose = new granny_world_pose();
    int count = BoneCount > 0 ? BoneCount : 1;
    pose->BoneCount = count;
    pose->World4x4.resize(static_cast<size_t>(count));
    pose->Composite4x4.resize(static_cast<size_t>(count));
    for (int i = 0; i < count; ++i)
    {
        GrannyMathInternal::LoadIdentity4x4(pose->World4x4[static_cast<size_t>(i)].m);
        GrannyMathInternal::LoadIdentity4x4(pose->Composite4x4[static_cast<size_t>(i)].m);
    }
    return pose;
}

inline void GrannyFreeWorldPose(granny_world_pose* WorldPose)
{
    delete WorldPose;
}

inline granny_real32* GrannyGetWorldPose4x4(const granny_world_pose* WorldPose, granny_int32x BoneIndex)
{
    if (!WorldPose || BoneIndex < 0 || BoneIndex >= WorldPose->BoneCount)
        return nullptr;
    return const_cast<granny_real32*>(&WorldPose->World4x4[static_cast<size_t>(BoneIndex)].m[0][0]);
}

inline granny_real32* GrannyGetWorldPoseComposite4x4(const granny_world_pose* WorldPose, granny_int32x BoneIndex)
{
    if (!WorldPose || BoneIndex < 0 || BoneIndex >= WorldPose->BoneCount)
        return nullptr;
    return const_cast<granny_real32*>(&WorldPose->Composite4x4[static_cast<size_t>(BoneIndex)].m[0][0]);
}

inline granny_matrix_4x4* GrannyGetWorldPoseComposite4x4Array(const granny_world_pose* WorldPose)
{
    if (!WorldPose || WorldPose->Composite4x4.empty())
        return nullptr;
    return reinterpret_cast<granny_matrix_4x4*>(const_cast<GrannyMatrix4x4Box*>(WorldPose->Composite4x4.data()));
}

inline void GrannyBuildWorldPose(
    const granny_skeleton* Skeleton,
    granny_int32x FirstBone,
    granny_int32x BoneCount,
    const granny_local_pose* LocalPose,
    const granny_real32* Offset4x4,
    granny_world_pose* Result)
{
    if (!Skeleton || !Result)
        return;

    granny_matrix_4x4 rootOffset;
    if (Offset4x4)
        std::memcpy(rootOffset, Offset4x4, sizeof(granny_matrix_4x4));
    else
        GrannyMathInternal::LoadIdentity4x4(rootOffset);

    int endBone = FirstBone + BoneCount;
    if (endBone > Skeleton->BoneCount)
        endBone = Skeleton->BoneCount;
    if (endBone > Result->BoneCount)
        endBone = Result->BoneCount;

    for (int i = FirstBone; i < endBone; ++i)
    {
        const granny_bone& bone = Skeleton->Bones[i];
        const granny_transform& tr =
            (LocalPose && i < LocalPose->BoneCount)
                ? LocalPose->Transforms[static_cast<size_t>(i)]
                : bone.LocalTransform;

        granny_matrix_4x4 localMat;
        GrannyMathInternal::TransformToMatrix4x4(tr, localMat);

        if (bone.ParentIndex >= 0 && bone.ParentIndex < i)
        {
            GrannyMathInternal::Multiply4x4(
                Result->World4x4[static_cast<size_t>(i)].m,
                localMat,
                Result->World4x4[static_cast<size_t>(bone.ParentIndex)].m);
        }
        else
        {
            GrannyMathInternal::Multiply4x4(
                Result->World4x4[static_cast<size_t>(i)].m,
                localMat,
                rootOffset);
        }

        // Check if InverseWorld4x4 is non-zero; otherwise treat as identity
        bool hasInv = false;
        for (int r = 0; r < 4 && !hasInv; ++r)
            for (int c = 0; c < 4 && !hasInv; ++c)
                if (std::fabs(bone.InverseWorld4x4[r][c]) > 1e-6f)
                    hasInv = true;

        if (hasInv)
        {
            GrannyMathInternal::Multiply4x4(
                Result->Composite4x4[static_cast<size_t>(i)].m,
                bone.InverseWorld4x4,
                Result->World4x4[static_cast<size_t>(i)].m);
        }
        else
        {
            std::memcpy(
                Result->Composite4x4[static_cast<size_t>(i)].m,
                Result->World4x4[static_cast<size_t>(i)].m,
                sizeof(granny_matrix_4x4));
        }
    }
}

// ============================================================================
// Model Instantiation & Animation Control Lifecycle
// ============================================================================

inline granny_model_instance* GrannyInstantiateModel(const granny_model* Model)
{
    granny_model_instance* inst = new granny_model_instance();
    inst->SourceModel = Model;
    inst->ModelClock = 0.0f;
    return inst;
}

inline void GrannyFreeModelInstance(granny_model_instance* Instance)
{
    if (!Instance)
        return;
    for (granny_control* ctrl : Instance->Controls)
        delete ctrl;
    Instance->Controls.clear();
    delete Instance;
}

inline granny_skeleton* GrannyGetSourceSkeleton(const granny_model_instance* Instance)
{
    if (!Instance || !Instance->SourceModel)
        return nullptr;
    return Instance->SourceModel->Skeleton;
}

inline granny_control* GrannyPlayControlledAnimation(
    granny_real32 StartTime,
    const granny_animation* Animation,
    granny_model_instance* Model)
{
    if (!Animation || !Model)
        return nullptr;

    granny_control* ctrl = new granny_control();
    ctrl->Animation = Animation;
    ctrl->StartTime = StartTime;
    ctrl->RawLocalClock = 0.0f;
    ctrl->Speed = 1.0f;
    ctrl->LoopCount = 1;
    Model->Controls.push_back(ctrl);
    return ctrl;
}

inline void GrannyFreeControl(granny_control* Control)
{
    if (!Control)
        return;
    Control->IsCompleted = true;
}

inline void GrannyFreeControlOnceUnused(granny_control* Control)
{
    if (Control)
        Control->FreeOnceUnused = true;
}

inline void GrannyCompleteControlAt(granny_control* Control, granny_real32 AtSeconds)
{
    if (!Control)
        return;
    Control->HasCompleteTime = true;
    Control->CompleteTime = AtSeconds;
}

inline bool GrannyControlIsComplete(const granny_control* Control)
{
    if (!Control)
        return true;
    if (Control->IsCompleted)
        return true;
    if (Control->LoopCount > 0 && Control->Animation)
    {
        float totalDuration = Control->Animation->Duration * float(Control->LoopCount);
        if (Control->RawLocalClock >= totalDuration)
            return true;
    }
    return false;
}

inline bool GrannyFreeControlIfComplete(granny_control* Control)
{
    if (!Control)
        return true;
    if (GrannyControlIsComplete(Control))
    {
        Control->IsCompleted = true;
        return true;
    }
    return false;
}

inline void GrannyFreeCompletedModelControls(granny_model_instance* ModelInstance)
{
    if (!ModelInstance)
        return;
    auto it = ModelInstance->Controls.begin();
    while (it != ModelInstance->Controls.end())
    {
        granny_control* ctrl = *it;
        if (ctrl->IsCompleted ||
            (ctrl->HasCompleteTime && ModelInstance->ModelClock >= ctrl->CompleteTime))
        {
            delete ctrl;
            it = ModelInstance->Controls.erase(it);
        }
        else
        {
            ++it;
        }
    }
}

inline void GrannySetModelClock(granny_model_instance* ModelInstance, granny_real32 NewClock)
{
    if (!ModelInstance)
        return;
    granny_real32 dt = NewClock - ModelInstance->ModelClock;
    ModelInstance->ModelClock = NewClock;
    for (granny_control* ctrl : ModelInstance->Controls)
    {
        if (ctrl && !ctrl->IsCompleted)
        {
            ctrl->RawLocalClock += dt * ctrl->Speed;
            if (ctrl->HasCompleteTime && NewClock >= ctrl->CompleteTime)
                ctrl->IsCompleted = true;
        }
    }
}

inline void GrannySetControlSpeed(granny_control* Control, granny_real32 Speed)
{
    if (Control)
        Control->Speed = Speed;
}

inline granny_real32 GrannyGetControlSpeed(const granny_control* Control)
{
    return Control ? Control->Speed : 1.0f;
}

inline void GrannySetControlLoopCount(granny_control* Control, granny_int32x LoopCount)
{
    if (Control)
        Control->LoopCount = LoopCount;
}

inline granny_int32x GrannyGetControlLoopCount(const granny_control* Control)
{
    return Control ? Control->LoopCount : 1;
}

inline void GrannySetControlRawLocalClock(granny_control* Control, granny_real32 LocalClock)
{
    if (Control)
        Control->RawLocalClock = LocalClock;
}

inline granny_real32 GrannyGetControlRawLocalClock(const granny_control* Control)
{
    return Control ? Control->RawLocalClock : 0.0f;
}

inline granny_real32 GrannyGetControlLocalDuration(const granny_control* Control)
{
    if (!Control || !Control->Animation)
        return 0.0f;
    return Control->Animation->Duration;
}

inline granny_real32 GrannyGetControlDurationLeft(const granny_control* Control)
{
    if (!Control || !Control->Animation)
        return 0.0f;
    if (Control->LoopCount == 0)
        return 1e6f;
    float total = Control->Animation->Duration * float(Control->LoopCount);
    float left = total - Control->RawLocalClock;
    return left > 0.0f ? left : 0.0f;
}

inline void GrannySetControlEaseIn(granny_control* Control, bool Enable)
{
    if (Control)
        Control->EaseIn = Enable;
}

inline void GrannySetControlEaseOut(granny_control* Control, bool Enable)
{
    if (Control)
        Control->EaseOut = Enable;
}

inline void GrannySetControlEaseInCurve(
    granny_control* Control,
    granny_real32 StartSeconds,
    granny_real32 EndSeconds,
    granny_real32 /*StartValue*/,
    granny_real32 /*StartTangent*/,
    granny_real32 /*EndTangent*/,
    granny_real32 /*EndValue*/)
{
    if (Control)
    {
        Control->EaseInStart = StartSeconds;
        Control->EaseInEnd = EndSeconds;
    }
}

inline void GrannySetControlEaseOutCurve(
    granny_control* Control,
    granny_real32 StartSeconds,
    granny_real32 EndSeconds,
    granny_real32 /*StartValue*/,
    granny_real32 /*StartTangent*/,
    granny_real32 /*EndTangent*/,
    granny_real32 /*EndValue*/)
{
    if (Control)
    {
        Control->EaseOutStart = StartSeconds;
        Control->EaseOutEnd = EndSeconds;
    }
}

// Returns the blend weight of a control at the given model-local time.
// Controls that are eased in/out simultaneously overlap, which is what makes
// animation transitions smooth instead of snapping to the newest control.
inline granny_real32 GrannyGetControlWeight(const granny_control* Control, granny_real32 CurrentTime)
{
    if (!Control || Control->IsCompleted)
        return 0.0f;

    granny_real32 Weight = 1.0f;

    if (Control->EaseIn && Control->EaseInEnd > Control->EaseInStart)
    {
        granny_real32 u = (CurrentTime - Control->EaseInStart) /
                          (Control->EaseInEnd - Control->EaseInStart);
        if (u <= 0.0f)
            Weight = 0.0f;
        else if (u < 1.0f)
            Weight *= u * u * (3.0f - 2.0f * u); // smoothstep
    }

    if (Control->EaseOut && Control->EaseOutEnd > Control->EaseOutStart)
    {
        granny_real32 u = (CurrentTime - Control->EaseOutStart) /
                          (Control->EaseOutEnd - Control->EaseOutStart);
        if (u >= 1.0f)
            Weight = 0.0f;
        else if (u > 0.0f)
            Weight *= 1.0f - u * u * (3.0f - 2.0f * u);
    }

    return Weight > 0.0f ? Weight : 0.0f;
}

// Selects the track group of an animation that belongs to the given skeleton.
// Older Metin2 animations often contain extra helper groups ("Box01",
// "Plane01", ...) before the actual biped group, so using TrackGroups[0]
// blindly leaves those models stuck in their rest/T-pose.
inline const granny_track_group* GrannyFindAnimationTrackGroupForSkeleton(
    const granny_animation* Animation,
    const granny_skeleton* Skeleton)
{
    if (!Animation || Animation->TrackGroupCount <= 0 || !Animation->TrackGroups)
        return nullptr;

    if (Animation->TrackGroupCount == 1)
        return Animation->TrackGroups[0];

    if (Skeleton && Skeleton->Name && *Skeleton->Name)
    {
        for (int i = 0; i < Animation->TrackGroupCount; ++i)
        {
            const granny_track_group* tg = Animation->TrackGroups[i];
            if (!tg || !tg->Name || _stricmp(tg->Name, Skeleton->Name) != 0)
                continue;

            if (tg->TransformTrackCount <= 0 || !tg->TransformTracks)
                continue;

            // Make sure the tracks actually target bones of this skeleton.
            for (int t = 0; t < tg->TransformTrackCount; ++t)
            {
                granny_int32x BoneIndex = -1;
                if (tg->TransformTracks[t].Name &&
                    GrannyFindBoneByName(Skeleton, tg->TransformTracks[t].Name, &BoneIndex))
                {
                    return tg;
                }
            }
        }
    }

    // No name match: pick the group sharing the most bone names with the
    // skeleton.  This covers exported track groups whose name differs from
    // the skeleton name.
    const granny_track_group* BestGroup = nullptr;
    int BestScore = 0;
    for (int i = 0; i < Animation->TrackGroupCount; ++i)
    {
        const granny_track_group* tg = Animation->TrackGroups[i];
        if (!tg || tg->TransformTrackCount <= 0 || !tg->TransformTracks)
            continue;

        int Score = 0;
        for (int t = 0; t < tg->TransformTrackCount; ++t)
        {
            granny_int32x BoneIndex = -1;
            if (tg->TransformTracks[t].Name &&
                GrannyFindBoneByName(Skeleton, tg->TransformTracks[t].Name, &BoneIndex))
            {
                ++Score;
            }
        }

        if (Score > BestScore)
        {
            BestScore = Score;
            BestGroup = tg;
        }
    }

    if (BestGroup)
        return BestGroup;

    return Animation->TrackGroups[0];
}

// Resolves (and caches) the track group and bone->track mapping used by a
// control for a specific skeleton.
inline const granny_track_group* GrannyGetControlTrackGroup(
    granny_control* Control,
    const granny_skeleton* Skeleton)
{
    if (!Control)
        return nullptr;

    if (Control->TrackGroupResolved && Control->CachedSkeleton == Skeleton)
        return Control->CachedTrackGroup;

    Control->TrackGroupResolved = true;
    Control->CachedSkeleton = Skeleton;
    Control->CachedTrackGroup =
        GrannyFindAnimationTrackGroupForSkeleton(Control->Animation, Skeleton);

    Control->BoneTrackIndices.clear();
    if (!Skeleton)
        return Control->CachedTrackGroup;

    Control->BoneTrackIndices.assign(static_cast<size_t>(Skeleton->BoneCount), -1);

    const granny_track_group* tg = Control->CachedTrackGroup;
    if (tg && tg->TransformTrackCount > 0 && tg->TransformTracks)
    {
        for (int t = 0; t < tg->TransformTrackCount; ++t)
        {
            granny_int32x BoneIndex = -1;
            if (tg->TransformTracks[t].Name &&
                GrannyFindBoneByName(Skeleton, tg->TransformTracks[t].Name, &BoneIndex) &&
                BoneIndex >= 0 && BoneIndex < Skeleton->BoneCount)
            {
                Control->BoneTrackIndices[static_cast<size_t>(BoneIndex)] = t;
            }
        }
    }

    return Control->CachedTrackGroup;
}

inline granny_real32 GrannyEaseControlIn(granny_control* Control, granny_real32 Duration, bool /*FromCurrent*/)
{
    if (Control)
        Control->EaseIn = true;
    return Duration;
}

inline granny_real32 GrannyEaseControlOut(granny_control* Control, granny_real32 Duration)
{
    if (Control)
        Control->EaseOut = true;
    return Duration;
}

inline void GrannySampleCurveVec3(const granny_curve& curve, float t, float* outVec3)
{
    if (curve.ControlCount <= 0 || !curve.Controls)
        return;

    if (curve.Degree == 0 || curve.KnotCount <= 1 || !curve.Knots)
    {
        outVec3[0] = curve.Controls[0];
        outVec3[1] = curve.Controls[1];
        outVec3[2] = curve.Controls[2];
        return;
    }

    if (t <= curve.Knots[0])
    {
        outVec3[0] = curve.Controls[0];
        outVec3[1] = curve.Controls[1];
        outVec3[2] = curve.Controls[2];
        return;
    }

    if (t >= curve.Knots[curve.KnotCount - 1])
    {
        int last = (curve.KnotCount - 1) * 3;
        if (last + 2 < curve.ControlCount)
        {
            outVec3[0] = curve.Controls[last];
            outVec3[1] = curve.Controls[last + 1];
            outVec3[2] = curve.Controls[last + 2];
        }
        return;
    }

    int k = 0;
    while (k + 1 < curve.KnotCount && t > curve.Knots[k + 1])
        ++k;

    float dt = curve.Knots[k + 1] - curve.Knots[k];
    float u = (dt > 1e-6f) ? (t - curve.Knots[k]) / dt : 0.0f;

    int idx0 = k * 3;
    int idx1 = (k + 1) * 3;
    if (idx1 + 2 < curve.ControlCount)
    {
        outVec3[0] = curve.Controls[idx0 + 0] + u * (curve.Controls[idx1 + 0] - curve.Controls[idx0 + 0]);
        outVec3[1] = curve.Controls[idx0 + 1] + u * (curve.Controls[idx1 + 1] - curve.Controls[idx0 + 1]);
        outVec3[2] = curve.Controls[idx0 + 2] + u * (curve.Controls[idx1 + 2] - curve.Controls[idx0 + 2]);
    }
}

inline void GrannySampleCurveQuat(const granny_curve& curve, float t, float* outQuat)
{
    if (curve.ControlCount <= 0 || !curve.Controls)
        return;

    if (curve.Degree == 0 || curve.KnotCount <= 1 || !curve.Knots)
    {
        outQuat[0] = curve.Controls[0];
        outQuat[1] = curve.Controls[1];
        outQuat[2] = curve.Controls[2];
        outQuat[3] = curve.Controls[3];
        return;
    }

    if (t <= curve.Knots[0])
    {
        outQuat[0] = curve.Controls[0];
        outQuat[1] = curve.Controls[1];
        outQuat[2] = curve.Controls[2];
        outQuat[3] = curve.Controls[3];
        return;
    }

    if (t >= curve.Knots[curve.KnotCount - 1])
    {
        int last = (curve.KnotCount - 1) * 4;
        if (last + 3 < curve.ControlCount)
        {
            outQuat[0] = curve.Controls[last];
            outQuat[1] = curve.Controls[last + 1];
            outQuat[2] = curve.Controls[last + 2];
            outQuat[3] = curve.Controls[last + 3];
        }
        return;
    }

    int k = 0;
    while (k + 1 < curve.KnotCount && t > curve.Knots[k + 1])
        ++k;

    float dt = curve.Knots[k + 1] - curve.Knots[k];
    float u = (dt > 1e-6f) ? (t - curve.Knots[k]) / dt : 0.0f;

    int idx0 = k * 4;
    int idx1 = (k + 1) * 4;
    if (idx1 + 3 < curve.ControlCount)
    {
        D3DXQUATERNION q0(curve.Controls[idx0], curve.Controls[idx0 + 1], curve.Controls[idx0 + 2], curve.Controls[idx0 + 3]);
        D3DXQUATERNION q1(curve.Controls[idx1], curve.Controls[idx1 + 1], curve.Controls[idx1 + 2], curve.Controls[idx1 + 3]);
        D3DXQUATERNION outQ;
        D3DXQuaternionSlerp(&outQ, &q0, &q1, u);
        outQuat[0] = outQ.x;
        outQuat[1] = outQ.y;
        outQuat[2] = outQ.z;
        outQuat[3] = outQ.w;
    }
}

inline void GrannySampleCurveScaleShear(const granny_curve& curve, float t, float* outMat3x3)
{
    if (curve.ControlCount < 9 || !curve.Controls)
        return;

    if (curve.Degree == 0 || curve.KnotCount <= 1 || !curve.Knots || t <= curve.Knots[0])
    {
        std::memcpy(outMat3x3, curve.Controls, sizeof(float) * 9);
        return;
    }

    if (t >= curve.Knots[curve.KnotCount - 1])
    {
        int last = (curve.KnotCount - 1) * 9;
        if (last + 8 < curve.ControlCount)
            std::memcpy(outMat3x3, &curve.Controls[last], sizeof(float) * 9);
        return;
    }

    int k = 0;
    while (k + 1 < curve.KnotCount && t > curve.Knots[k + 1])
        ++k;

    float dt = curve.Knots[k + 1] - curve.Knots[k];
    float u = (dt > 1e-6f) ? (t - curve.Knots[k]) / dt : 0.0f;

    int idx0 = k * 9;
    int idx1 = (k + 1) * 9;
    if (idx1 + 8 < curve.ControlCount)
    {
        for (int i = 0; i < 9; ++i)
            outMat3x3[i] = curve.Controls[idx0 + i] + u * (curve.Controls[idx1 + i] - curve.Controls[idx0 + i]);
    }
}

inline void GrannySampleModelAnimations(
    const granny_model_instance* ModelInstance,
    granny_int32x FirstBone,
    granny_int32x BoneCount,
    granny_local_pose* Result)
{
    if (!ModelInstance || !ModelInstance->SourceModel || !ModelInstance->SourceModel->Skeleton || !Result)
        return;

    const granny_skeleton* skel = ModelInstance->SourceModel->Skeleton;

    if (FirstBone < 0)
        FirstBone = 0;
    int endBone = FirstBone + BoneCount;
    if (endBone > skel->BoneCount)
        endBone = skel->BoneCount;
    if (endBone > Result->BoneCount)
        endBone = Result->BoneCount;
    if (FirstBone >= endBone)
        return;

    struct TControlSample
    {
        granny_control* Control;
        granny_real32   Weight;
        granny_real32   AnimationTime;
    };

    static thread_local std::vector<TControlSample> samples;
    samples.clear();

    for (granny_control* ctrl : ModelInstance->Controls)
    {
        if (!ctrl || ctrl->IsCompleted || !ctrl->Animation)
            continue;

        // Resolve the track group that belongs to this model's skeleton instead
        // of assuming TrackGroups[0] (helper/box groups are often first).
        const granny_track_group* tg = GrannyGetControlTrackGroup(ctrl, skel);
        if (!tg || tg->TransformTrackCount <= 0 || !tg->TransformTracks)
            continue;

        const granny_real32 weight = GrannyGetControlWeight(ctrl, ModelInstance->ModelClock);
        if (weight <= 0.0f)
            continue;

        const granny_animation* anim = ctrl->Animation;
        granny_real32 animTime = ctrl->RawLocalClock;
        if (anim->Duration > 0.0f)
        {
            if (ctrl->LoopCount == 0)
                animTime = std::fmod(animTime, anim->Duration);
            else if (animTime > anim->Duration)
                animTime = anim->Duration;
        }

        samples.push_back({ctrl, weight, animTime});
    }

    if (samples.empty())
    {
        for (int i = FirstBone; i < endBone; ++i)
            Result->Transforms[static_cast<size_t>(i)] = skel->Bones[i].LocalTransform;
        return;
    }

    // Blend every active control together.  During a transition the outgoing
    // control is eased out while the incoming one is eased in, so both must
    // contribute to the pose with their respective weights.
    for (int i = FirstBone; i < endBone; ++i)
    {
        const granny_transform& rest = skel->Bones[i].LocalTransform;

        granny_transform blended = rest;
        granny_real32 accumulatedWeight = 0.0f;

        for (const TControlSample& sample : samples)
        {
            granny_transform sampled = rest;

            const granny_track_group* tg = sample.Control->CachedTrackGroup;
            const granny_int32 trackIndex =
                (static_cast<size_t>(i) < sample.Control->BoneTrackIndices.size())
                    ? sample.Control->BoneTrackIndices[static_cast<size_t>(i)]
                    : -1;

            if (tg && trackIndex >= 0 && trackIndex < tg->TransformTrackCount)
            {
                const granny_transform_track& track =
                    tg->TransformTracks[static_cast<size_t>(trackIndex)];

                if (track.PositionCurve.ControlCount >= 3)
                {
                    GrannySampleCurveVec3(track.PositionCurve, sample.AnimationTime, sampled.Position);
                    sampled.Flags |= GrannyHasPosition;
                }
                if (track.OrientationCurve.ControlCount >= 4)
                {
                    GrannySampleCurveQuat(track.OrientationCurve, sample.AnimationTime, sampled.Orientation);
                    sampled.Flags |= GrannyHasOrientation;
                }
                if (track.ScaleShearCurve.ControlCount >= 9)
                {
                    GrannySampleCurveScaleShear(
                        track.ScaleShearCurve,
                        sample.AnimationTime,
                        reinterpret_cast<float*>(sampled.ScaleShear));
                    sampled.Flags |= GrannyHasScaleShear;
                }
            }

            if (accumulatedWeight <= 0.0f)
            {
                blended = sampled;
                accumulatedWeight = sample.Weight;
            }
            else
            {
                const granny_real32 blend =
                    sample.Weight / (accumulatedWeight + sample.Weight);
                GrannyMathInternal::BlendTransform(blended, sampled, blend);
                accumulatedWeight += sample.Weight;
            }
        }

        Result->Transforms[static_cast<size_t>(i)] = blended;
    }
}

inline void GrannySampleModelAnimationsAccelerated(
    const granny_model_instance* ModelInstance,
    granny_int32x BoneCount,
    const granny_real32* Offset4x4,
    granny_local_pose* TempLocalPose,
    granny_world_pose* ResultWorldPose)
{
    if (!ModelInstance || !ModelInstance->SourceModel || !ModelInstance->SourceModel->Skeleton || !ResultWorldPose)
        return;

    const granny_skeleton* skel = ModelInstance->SourceModel->Skeleton;
    GrannySampleModelAnimations(ModelInstance, 0, BoneCount, TempLocalPose);
    GrannyBuildWorldPose(skel, 0, BoneCount, TempLocalPose, Offset4x4, ResultWorldPose);
}

inline void GrannyUpdateModelMatrix(
    const granny_model_instance* ModelInstance,
    granny_real32 SecondsElapsed,
    const granny_real32* ModelMatrix4x4,
    granny_real32* DestMatrix4x4)
{
    if (ModelMatrix4x4 && DestMatrix4x4 && ModelMatrix4x4 != DestMatrix4x4)
    {
        std::memcpy(DestMatrix4x4, ModelMatrix4x4, sizeof(granny_matrix_4x4));
    }
    if (!ModelInstance || !DestMatrix4x4 || SecondsElapsed <= 0.0f)
        return;

    const granny_skeleton* skel =
        (ModelInstance->SourceModel ? ModelInstance->SourceModel->Skeleton : nullptr);

    auto hasLoopTranslation = [](const granny_track_group* tg) {
        return tg && (tg->LoopTranslation[0] != 0.0f ||
                      tg->LoopTranslation[1] != 0.0f ||
                      tg->LoopTranslation[2] != 0.0f);
    };

    // The newest active control drives the current motion.  Whether it carries
    // root motion decides how the translation blend has to be normalized.
    granny_control* newestActive = nullptr;
    for (granny_control* ctrl : ModelInstance->Controls)
    {
        if (ctrl && !ctrl->IsCompleted && ctrl->Animation)
            newestActive = ctrl;
    }

    const granny_track_group* newestTrackGroup = nullptr;
    if (newestActive && newestActive->Animation->Duration > 0.0f)
        newestTrackGroup = GrannyGetControlTrackGroup(newestActive, skel);
    const bool newestHasRootMotion = hasLoopTranslation(newestTrackGroup);

    granny_triple delta = {0.0f, 0.0f, 0.0f};
    granny_real32 allWeight = 0.0f;
    granny_real32 rootWeight = 0.0f;
    bool hasDelta = false;

    // Blend root motion with the control weights.
    for (granny_control* ctrl : ModelInstance->Controls)
    {
        if (!ctrl || !ctrl->Animation || ctrl->IsCompleted) continue;
        if (ctrl->Animation->Duration <= 0.0f) continue;

        const granny_real32 weight = GrannyGetControlWeight(ctrl, ModelInstance->ModelClock);
        if (weight <= 0.0f) continue;

        allWeight += weight;

        const granny_track_group* tg = GrannyGetControlTrackGroup(ctrl, skel);
        if (!hasLoopTranslation(tg)) continue;

        rootWeight += weight;

        const float factor =
            (SecondsElapsed * ctrl->Speed * weight) / ctrl->Animation->Duration;
        delta[0] += tg->LoopTranslation[0] * factor;
        delta[1] += tg->LoopTranslation[1] * factor;
        delta[2] += tg->LoopTranslation[2] * factor;
        hasDelta = true;
    }

    if (!hasDelta && newestHasRootMotion && newestActive &&
        newestActive->Animation->Duration > 0.0f)
    {
        // The incoming locomotion control is still at weight zero (typically
        // the first frame of an idle -> run/walk transition).  Use its full
        // translation immediately so movement does not wait for the pose
        // cross-fade to complete.
        const float factor =
            (SecondsElapsed * newestActive->Speed) / newestActive->Animation->Duration;
        delta[0] = newestTrackGroup->LoopTranslation[0] * factor;
        delta[1] = newestTrackGroup->LoopTranslation[1] * factor;
        delta[2] = newestTrackGroup->LoopTranslation[2] * factor;
        rootWeight = 1.0f;
        hasDelta = true;
    }

    if (hasDelta)
    {
        // If the current motion provides root motion, only movement controls
        // participate in the blend.  This makes starting to walk/run instant
        // and keeps run <-> walk transitions smooth.  When the current motion
        // has no root motion (stopping, idle, attacks), all controls are used
        // so the outgoing movement fades out naturally.
        granny_real32 denominator = newestHasRootMotion ? rootWeight : allWeight;
        if (denominator <= 1e-6f)
            denominator = 1.0f;

        delta[0] /= denominator;
        delta[1] /= denominator;
        delta[2] /= denominator;

        // Granny's TrackGroup coordinates exported from 3ds Max:
        // delta[0] is forward/backward translation (corresponds to Metin2 local Y, where -Y is forward)
        // delta[1] is left/right translation (corresponds to Metin2 local X)
        // delta[2] is up/down translation (corresponds to Metin2 local Z)
        // In ModelMatrix4x4 (row-major D3D):
        // Row 0 (indices 0, 1, 2) is the local X axis
        // Row 1 (indices 4, 5, 6) is the local Y axis
        // Row 2 (indices 8, 9, 10) is the local Z axis
        float m2_local_x = delta[1];
        float m2_local_y = delta[0];
        float m2_local_z = delta[2];

        float dx = m2_local_x * ModelMatrix4x4[0] + m2_local_y * ModelMatrix4x4[4] + m2_local_z * ModelMatrix4x4[8];
        float dy = m2_local_x * ModelMatrix4x4[1] + m2_local_y * ModelMatrix4x4[5] + m2_local_z * ModelMatrix4x4[9];
        float dz = m2_local_x * ModelMatrix4x4[2] + m2_local_y * ModelMatrix4x4[6] + m2_local_z * ModelMatrix4x4[10];
        DestMatrix4x4[12] += dx;
        DestMatrix4x4[13] += dy;
        DestMatrix4x4[14] += dz;
    }
}

// ============================================================================
// CPU Mesh Deformer (Linear Blend Skinning)
// ============================================================================

inline granny_mesh_deformer* GrannyNewMeshDeformer(
    const granny_data_type_definition* InputVertexLayout,
    const granny_data_type_definition* OutputVertexLayout,
    granny_deformation_type DeformationType)
{
    granny_mesh_deformer* deformer = new granny_mesh_deformer();
    deformer->InputType = InputVertexLayout;
    deformer->OutputType = OutputVertexLayout;
    deformer->DeformationType = DeformationType;

    granny_int32x inSize = GrannyGetTotalObjectSize(InputVertexLayout);
    granny_int32x outSize = GrannyGetTotalObjectSize(OutputVertexLayout);
    deformer->InputStride = inSize > 0 ? static_cast<size_t>(inSize) : sizeof(granny_pnt332_vertex);
    deformer->OutputStride = outSize > 0 ? static_cast<size_t>(outSize) : sizeof(granny_pnt332_vertex);

    deformer->HasBoneWeights = false;
    if (InputVertexLayout)
    {
        for (int i = 0; InputVertexLayout[i].Type != GrannyEndMember; ++i)
        {
            if (InputVertexLayout[i].Name &&
                _stricmp(InputVertexLayout[i].Name, GrannyVertexBoneWeightsName) == 0)
            {
                deformer->HasBoneWeights = true;
                break;
            }
        }
    }
    return deformer;
}

inline void GrannyFreeMeshDeformer(granny_mesh_deformer* Deformer)
{
    delete Deformer;
}

inline void GrannyDeformVertices(
    const granny_mesh_deformer* Deformer,
    const granny_int32x* MatrixIndices,
    const granny_real32* MatrixBuffer4x4,
    granny_int32x VertexCount,
    const void* SourceVertices,
    void* DestVertices)
{
    if (!Deformer || !SourceVertices || !DestVertices || VertexCount <= 0)
        return;

    const granny_matrix_4x4* matrices = reinterpret_cast<const granny_matrix_4x4*>(MatrixBuffer4x4);
    const uint8_t* srcBytes = static_cast<const uint8_t*>(SourceVertices);
    uint8_t* dstBytes = static_cast<uint8_t*>(DestVertices);

    for (int v = 0; v < VertexCount; ++v)
    {
        const uint8_t* sv = srcBytes + static_cast<size_t>(v) * Deformer->InputStride;
        uint8_t* dv = dstBytes + static_cast<size_t>(v) * Deformer->OutputStride;

        if (Deformer->HasBoneWeights && Deformer->InputStride >= sizeof(granny_pwnt3432_vertex))
        {
            const granny_pwnt3432_vertex* pwnt = reinterpret_cast<const granny_pwnt3432_vertex*>(sv);
            granny_pnt332_vertex* outPnt = reinterpret_cast<granny_pnt332_vertex*>(dv);

            float px = 0.0f, py = 0.0f, pz = 0.0f;
            float nx = 0.0f, ny = 0.0f, nz = 0.0f;
            float totalWeight = 0.0f;

            for (int w = 0; w < 4; ++w)
            {
                float weight = float(pwnt->BoneWeights[w]) / 255.0f;
                if (weight <= 0.0f)
                    continue;
                totalWeight += weight;

                int localBone = pwnt->BoneIndices[w];
                int boneIdx = MatrixIndices ? MatrixIndices[localBone] : localBone;
                if (matrices && boneIdx >= 0)
                {
                    const granny_matrix_4x4& m = matrices[boneIdx];
                    px += weight * (pwnt->Position[0] * m[0][0] + pwnt->Position[1] * m[1][0] + pwnt->Position[2] * m[2][0] + m[3][0]);
                    py += weight * (pwnt->Position[0] * m[0][1] + pwnt->Position[1] * m[1][1] + pwnt->Position[2] * m[2][1] + m[3][1]);
                    pz += weight * (pwnt->Position[0] * m[0][2] + pwnt->Position[1] * m[1][2] + pwnt->Position[2] * m[2][2] + m[3][2]);

                    nx += weight * (pwnt->Normal[0] * m[0][0] + pwnt->Normal[1] * m[1][0] + pwnt->Normal[2] * m[2][0]);
                    ny += weight * (pwnt->Normal[0] * m[0][1] + pwnt->Normal[1] * m[1][1] + pwnt->Normal[2] * m[2][1]);
                    nz += weight * (pwnt->Normal[0] * m[0][2] + pwnt->Normal[1] * m[1][2] + pwnt->Normal[2] * m[2][2]);
                }
            }

            if (totalWeight > 0.0f && matrices)
            {
                outPnt->Position[0] = px;
                outPnt->Position[1] = py;
                outPnt->Position[2] = pz;
                outPnt->Normal[0] = nx;
                outPnt->Normal[1] = ny;
                outPnt->Normal[2] = nz;
            }
            else
            {
                std::memcpy(outPnt->Position, pwnt->Position, sizeof(float) * 3);
                std::memcpy(outPnt->Normal, pwnt->Normal, sizeof(float) * 3);
            }
            outPnt->UV[0] = pwnt->UV[0];
            outPnt->UV[1] = pwnt->UV[1];
        }
        else
        {
            const granny_pnt332_vertex* inPnt = reinterpret_cast<const granny_pnt332_vertex*>(sv);
            granny_pnt332_vertex* outPnt = reinterpret_cast<granny_pnt332_vertex*>(dv);

            int boneIdx = MatrixIndices ? MatrixIndices[0] : 0;
            if (matrices && boneIdx >= 0)
            {
                const granny_matrix_4x4& m = matrices[boneIdx];
                outPnt->Position[0] = inPnt->Position[0] * m[0][0] + inPnt->Position[1] * m[1][0] + inPnt->Position[2] * m[2][0] + m[3][0];
                outPnt->Position[1] = inPnt->Position[0] * m[0][1] + inPnt->Position[1] * m[1][1] + inPnt->Position[2] * m[2][1] + m[3][1];
                outPnt->Position[2] = inPnt->Position[0] * m[0][2] + inPnt->Position[1] * m[1][2] + inPnt->Position[2] * m[2][2] + m[3][2];

                outPnt->Normal[0] = inPnt->Normal[0] * m[0][0] + inPnt->Normal[1] * m[1][0] + inPnt->Normal[2] * m[2][0];
                outPnt->Normal[1] = inPnt->Normal[0] * m[0][1] + inPnt->Normal[1] * m[1][1] + inPnt->Normal[2] * m[2][1];
                outPnt->Normal[2] = inPnt->Normal[0] * m[0][2] + inPnt->Normal[1] * m[1][2] + inPnt->Normal[2] * m[2][2];
            }
            else
            {
                std::memcpy(outPnt->Position, inPnt->Position, sizeof(float) * 3);
                std::memcpy(outPnt->Normal, inPnt->Normal, sizeof(float) * 3);
            }
            outPnt->UV[0] = inPnt->UV[0];
            outPnt->UV[1] = inPnt->UV[1];
        }
    }
}
