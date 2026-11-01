#pragma once

#include <Platform/Platform.h>
#include <string>
#include <vector>
#include <array>
#include <cstring>
#include <cmath>

#define BUILD_SPEED_TREE_RT_SET

// Render bit vectors
#define SpeedTree_BranchGeometry (1 << 0)
#define SpeedTree_FrondGeometry (1 << 1)
#define SpeedTree_LeafGeometry (1 << 2)
#define SpeedTree_BillboardGeometry (1 << 3)
#define SpeedTree_SimpleBillboardOverride (1 << 4)
#define SpeedTree_AllGeometry                                                                                          \
    (SpeedTree_BranchGeometry | SpeedTree_FrondGeometry | SpeedTree_LeafGeometry | SpeedTree_BillboardGeometry)

class CSpeedTreeRT
{
  public:
    enum EWindMethod
    {
        WIND_GPU,
        WIND_CPU,
        WIND_NONE
    };

    enum ELodMethod
    {
        LOD_POP = 0,
        LOD_SMOOTH = 1,
        LOD_NONE = 3
    };

    enum ELightingMethod
    {
        LIGHT_DYNAMIC,
        LIGHT_STATIC
    };

    enum EStaticLightingStyle
    {
        SLS_BASIC,
        SLS_USE_LIGHT_SOURCES,
        SLS_SIMULATE_SHADOWS
    };

    enum ECollisionObjectType
    {
        CO_SPHERE,
        CO_CYLINDER,
        CO_BOX
    };

    struct SGeometry
    {
        SGeometry();
        ~SGeometry();

        struct SIndexed
        {
            SIndexed();
            ~SIndexed() = default;

            int m_nDiscreteLodLevel = -1;
            unsigned short m_usNumStrips = 0;
            const unsigned short *m_pStripLengths = nullptr;
            const unsigned short **m_pStrips = nullptr;

            unsigned short m_usVertexCount = 0;
            const unsigned long *m_pColors = nullptr;
            const float *m_pNormals = nullptr;
            const float *m_pBinormals = nullptr;
            const float *m_pTangents = nullptr;
            const float *m_pCoords = nullptr;
            const float *m_pTexCoords0 = nullptr;
            const float *m_pTexCoords1 = nullptr;
            const float *m_pWindWeights = nullptr;
            const unsigned char *m_pWindMatrixIndices = nullptr;
        };

        struct SLeaf
        {
            SLeaf();
            ~SLeaf() = default;

            bool m_bIsActive = false;
            float m_fAlphaTestValue = 0.0f;
            int m_nDiscreteLodLevel = -1;
            unsigned short m_usLeafCount = 0;

            const unsigned char *m_pLeafMapIndices = nullptr;
            const unsigned char *m_pLeafClusterIndices = nullptr;
            const float *m_pCenterCoords = nullptr;
            const float **m_pLeafMapTexCoords = nullptr;
            const float **m_pLeafMapCoords = nullptr;

            const unsigned long *m_pColors = nullptr;
            const float *m_pNormals = nullptr;
            const float *m_pBinormals = nullptr;
            const float *m_pTangents = nullptr;
            const float *m_pWindWeights = nullptr;
            const unsigned char *m_pWindMatrixIndices = nullptr;
        };

        struct SBillboard
        {
            SBillboard();
            ~SBillboard() = default;

            bool m_bIsActive = false;
            const float *m_pTexCoords = nullptr;
            const float *m_pCoords = nullptr;
            float m_fAlphaTestValue = 0.0f;
        };

        SIndexed m_sBranches;
        float m_fBranchAlphaTestValue = 0.0f;

        SIndexed m_sFronds;
        float m_fFrondAlphaTestValue = 0.0f;

        SLeaf m_sLeaves0;
        SLeaf m_sLeaves1;

        SBillboard m_sBillboard0;
        SBillboard m_sBillboard1;
        SBillboard m_sHorizontalBillboard;
    };

    struct STextures
    {
        STextures();
        ~STextures() = default;

        const char *m_pBranchTextureFilename = nullptr;
        unsigned int m_uiLeafTextureCount = 0;
        const char **m_pLeafTextureFilenames = nullptr;
        unsigned int m_uiFrondTextureCount = 0;
        const char **m_pFrondTextureFilenames = nullptr;
        const char *m_pCompositeFilename = nullptr;
        const char *m_pSelfShadowFilename = nullptr;
    };

    CSpeedTreeRT();
    virtual ~CSpeedTreeRT();

    // Instances
    bool Compute(const float *pTransform = nullptr, unsigned int nSeed = 1, bool bCompositeStrips = true);
    CSpeedTreeRT *Clone(float x = 0.0f, float y = 0.0f, float z = 0.0f, unsigned int nSeed = 0) const;
    const CSpeedTreeRT *InstanceOf() const;
    CSpeedTreeRT *MakeInstance();
    void DeleteTransientData();

    // Tree file loading
    bool LoadTree(const char *pFilename);
    bool LoadTree(const unsigned char *pBlock, unsigned int nNumBytes);
    unsigned char *SaveTree(unsigned int &nNumBytes, bool bSaveLeaves = false) const;

    // Size / Position
    void GetTreeSize(float &fSize, float &fVariance) const;
    void SetTreeSize(float fNewSize, float fNewVariance = 0.0f);
    unsigned int GetSeed() const;
    const float *GetTreePosition() const;
    void SetTreePosition(float x, float y, float z);
    void SetLeafTargetAlphaMask(unsigned char ucMask = 0x54);

    // Lighting
    ELightingMethod GetBranchLightingMethod() const;
    void SetBranchLightingMethod(ELightingMethod eMethod);
    ELightingMethod GetLeafLightingMethod() const;
    void SetLeafLightingMethod(ELightingMethod eMethod);
    ELightingMethod GetFrondLightingMethod() const;
    void SetFrondLightingMethod(ELightingMethod eMethod);

    EStaticLightingStyle GetStaticLightingStyle() const;
    void SetStaticLightingStyle(EStaticLightingStyle eStyle);
    float GetLeafLightingAdjustment() const;
    void SetLeafLightingAdjustment(float fScalar);

    static bool GetLightState(unsigned int nLightIndex);
    static void SetLightState(unsigned int nLightIndex, bool bLightOn);
    static const float *GetLightAttributes(unsigned int nLightIndex);
    static void SetLightAttributes(unsigned int nLightIndex, const float *pLightAttributes);

    const float *GetBranchMaterial() const;
    void SetBranchMaterial(const float *pMaterial);
    const float *GetLeafMaterial() const;
    void SetLeafMaterial(const float *pMaterial);
    const float *GetFrondMaterial() const;
    void SetFrondMaterial(const float *pMaterial);

    // Camera
    static void GetCamera(float *pPosition, float *pDirection);
    static void SetCamera(const float *pPosition, const float *pDirection);

    // Wind
    static void SetTime(float fTime);
    void ComputeWindEffects(bool bBranches, bool bLeaves, bool bFronds = true);
    void ResetLeafWindState();
    bool GetLeafRockingState() const;
    void SetLeafRockingState(bool bFlag);
    void SetNumLeafRockingGroups(unsigned int nRockingGroups);

    EWindMethod GetLeafWindMethod() const;
    void SetLeafWindMethod(EWindMethod eMethod);
    EWindMethod GetBranchWindMethod() const;
    void SetBranchWindMethod(EWindMethod eMethod);
    EWindMethod GetFrondWindMethod() const;
    void SetFrondWindMethod(EWindMethod eMethod);

    float GetWindStrength() const;
    float SetWindStrength(float fNewStrength, float fOldStrength = -1.0f, float fFrequencyTimeOffset = -1.0f);

    static void SetNumWindMatrices(unsigned int nNumMatrices);
    static void SetWindMatrix(unsigned int nMatrixIndex, const float *pMatrix);
    void GetLocalMatrices(unsigned int &nStartingIndex, unsigned int &nMatrixSpan);
    void SetLocalMatrices(unsigned int nStartingMatrix, unsigned int nMatrixSpan);

    // LOD
    void ComputeLodLevel();
    float GetLodLevel() const;
    void SetLodLevel(float fLodLevel);
    static void SetDropToBillboard(bool bFlag);

    void GetLodLimits(float &fNear, float &fFar) const;
    void SetLodLimits(float fNear, float fFar);

    short GetDiscreteBranchLodLevel(float fLodLevel = -1.0f) const;
    unsigned short GetDiscreteLeafLodLevel(float fLodLevel = -1.0f) const;
    short GetDiscreteFrondLodLevel(float fLodLevel = -1.0f) const;

    unsigned short GetNumBranchLodLevels() const;
    unsigned short GetNumLeafLodLevels() const;
    unsigned short GetNumFrondLodLevels() const;

    // Geometry
    void DeleteBranchGeometry();
    void DeleteFrondGeometry();
    unsigned char *GetFrondGeometryMapIndexes(int nLodLevel) const;
    const float *GetLeafBillboardTable(unsigned int &nEntryCount) const;
    const float *GetLeafLodSizeAdjustments();
    void GetGeometry(SGeometry &sGeometry, unsigned long ulBitVector = SpeedTree_AllGeometry,
                     short sOverrideBranchLodValue = -1, short sOverrideFrondLodValue = -1,
                     short sOverrideLeafLodValue = -1);

    // Textures
    void GetTextures(STextures &sTextures) const;
    void SetLeafTextureCoords(unsigned int nLeafMapIndex, const float *pTexCoords);
    void SetFrondTextureCoords(unsigned int nFrondMapIndex, const float *pTexCoords);
    static bool GetTextureFlip();
    static void SetTextureFlip(bool bFlag);
    void SetBranchTextureFilename(const char *pFilename);
    void SetLeafTextureFilename(unsigned int nLeafMapIndex, const char *pFilename);
    void SetFrondTextureFilename(unsigned int nFrondMapIndex, const char *pFilename);

    // Info & Error
    static void Authorize(const char *pKey);
    static bool IsAuthorized();
    static const char *GetCurrentError();
    static void ResetError();

    void GetBoundingBox(float *pBounds) const;
    unsigned int GetLeafTriangleCount(float fLodLevel = -1.0f) const;
    unsigned int GetBranchTriangleCount(float fLodLevel = -1.0f) const;
    unsigned int GetFrondTriangleCount(float fLodLevel = -1.0f) const;

    // Collisions
    unsigned int GetCollisionObjectCount();
    void GetCollisionObject(unsigned int nIndex, ECollisionObjectType &eType, float *pPosition, float *pDimensions);

    // User data
    const char *GetUserData() const;

  private:
    struct SCollisionObj
    {
        ECollisionObjectType m_eType;
        float m_afPos[3];
        float m_afDims[3];
    };

    float m_afPos[3]{};
    float m_afBoundingBox[6]{};
    float m_fSize = 400.0f;
    float m_fSizeVariance = 0.0f;
    unsigned int m_nSeed = 1;
    float m_fWindStrength = 0.0f;
    float m_fLeafLightingAdjustment = 1.0f;
    ELightingMethod m_eBranchLighting = LIGHT_STATIC;
    ELightingMethod m_eLeafLighting = LIGHT_STATIC;
    ELightingMethod m_eFrondLighting = LIGHT_STATIC;
    EWindMethod m_eBranchWind = WIND_NONE;
    EWindMethod m_eLeafWind = WIND_NONE;
    EWindMethod m_eFrondWind = WIND_NONE;
    float m_afBranchMaterial[13]{};
    float m_afLeafMaterial[13]{};
    float m_afFrondMaterial[13]{};

    // Texture filenames
    std::string m_strBranchTexture;
    std::string m_strCompositeTexture;
    std::string m_strSelfShadowTexture;
    std::vector<std::string> m_vLeafTextures;
    mutable std::vector<const char *> m_vpLeafTextures;
    std::vector<std::string> m_vFrondTextures;
    mutable std::vector<const char *> m_vpFrondTextures;

    // Collisions
    std::vector<SCollisionObj> m_vCollisions;

    // ------------------------------------------------------------------
    // Procedurally generated geometry.
    // The .spt files shipped with Metin2 only contain generation parameters
    // (Bezier splines, sizes, texture names); the geometry itself was produced
    // by the proprietary SpeedTreeRT generator. This clean-room replacement
    // builds a deterministic tree (tapered trunk/limbs + leaf cards) so the
    // existing SpeedTreeLib renderer has geometry to draw.
    // ------------------------------------------------------------------
    void GenerateGeometry();
    void UpdateLeafBillboards();
    void UpdateLeafBillboardsAt(const float *pTreePos);
    bool LoadBakedGeometry(unsigned int nHash);

    enum ETreeShape
    {
        SHAPE_BROADLEAF,
        SHAPE_BIRCH,
        SHAPE_CONIFER,
        SHAPE_PALM,
        SHAPE_BAOBAB,
        SHAPE_SMALL
    };

    ETreeShape m_eShape = SHAPE_BROADLEAF;

    bool m_bGeometryGenerated = false;
    bool m_bBakedGeometry = false;
    float m_fBranchAlpha = 1.0f;
    float m_fFrondAlpha = 0.0f;
    float m_fLeafAlpha = 128.0f;
    std::vector<float> m_vBranchCoords;
    std::vector<unsigned long> m_vBranchColors;
    std::vector<float> m_vBranchTexCoords0;
    std::vector<float> m_vBranchTexCoords1;
    std::vector<unsigned short> m_vBranchStrip;
    unsigned short m_usBranchStripLength = 0;
    std::vector<const unsigned short *> m_vpBranchStrips;

    std::vector<float> m_vFrondCoords;
    std::vector<unsigned long> m_vFrondColors;
    std::vector<float> m_vFrondTexCoords0;
    std::vector<float> m_vFrondTexCoords1;
    std::vector<unsigned short> m_vFrondStrip;
    unsigned short m_usFrondStripLength = 0;
    std::vector<const unsigned short *> m_vpFrondStrips;

    std::vector<float> m_vLeafCenters;
    std::vector<float> m_vLeafHalfW;
    std::vector<float> m_vLeafHalfH;
    std::vector<unsigned long> m_vLeafColors;
    std::vector<float> m_vLeafSize;
    std::vector<float> m_vLeafAngle;
    std::vector<std::array<float, 16>> m_vLeafCoords;
    std::vector<std::array<float, 8>> m_vLeafTexCoords;
    std::vector<const float *> m_vpLeafCoords;
    std::vector<const float *> m_vpLeafTexCoords;
    std::vector<unsigned char> m_vLeafMapIndices;
    std::vector<unsigned char> m_vLeafClusterIndices;

    // Instancing
    const CSpeedTreeRT *m_pInstanceOf = nullptr;

    // Static states
    static std::string ms_strError;
    static bool ms_bTextureFlip;
    static bool ms_bDropToBillboard;
    static float ms_afCameraPos[3];
    static float ms_afCameraDir[3];
    static float ms_afLightAttrs[16];
    static bool ms_bLightOn;
    static unsigned int ms_unNumWindMatrices;
};
