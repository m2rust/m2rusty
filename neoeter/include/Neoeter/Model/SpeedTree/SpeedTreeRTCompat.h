#pragma once

#include <Platform/Platform.h>
#include <string>
#include <vector>
#include <array>
#include <cstring>
#include <cmath>

#define BUILD_SPEED_TREE_RT_SET

// Render bit vectors
#define SpeedTree_BranchGeometry          (1 << 0)
#define SpeedTree_FrondGeometry           (1 << 1)
#define SpeedTree_LeafGeometry            (1 << 2)
#define SpeedTree_BillboardGeometry       (1 << 3)
#define SpeedTree_SimpleBillboardOverride (1 << 4)
#define SpeedTree_AllGeometry             (SpeedTree_BranchGeometry | SpeedTree_FrondGeometry | SpeedTree_LeafGeometry | SpeedTree_BillboardGeometry)

class CSpeedTreeRT
{
public:
    enum EWindMethod
    {
        WIND_GPU, WIND_CPU, WIND_NONE
    };

    enum ELodMethod
    {
        LOD_POP, LOD_SMOOTH, LOD_NONE = 3
    };

    enum ELightingMethod
    {
        LIGHT_DYNAMIC, LIGHT_STATIC
    };

    enum EStaticLightingStyle
    {
        SLS_BASIC, SLS_USE_LIGHT_SOURCES, SLS_SIMULATE_SHADOWS
    };

    enum ECollisionObjectType
    {
        CO_SPHERE, CO_CYLINDER, CO_BOX
    };

    struct SGeometry
    {
        SGeometry();
        ~SGeometry();

        struct SIndexed
        {
            SIndexed();
            ~SIndexed();

            int                     m_nDiscreteLodLevel;
            unsigned short          m_usNumStrips;
            const unsigned short*   m_pStripLengths;
            const unsigned short**  m_pStrips;

            unsigned short          m_usVertexCount;
            const unsigned long*    m_pColors;
            const float*            m_pNormals;
            const float*            m_pBinormals;
            const float*            m_pTangents;
            const float*            m_pCoords;
            const float*            m_pTexCoords0;
            const float*            m_pTexCoords1;
            const float*            m_pWindWeights;
            const unsigned char*    m_pWindMatrixIndices;
        };

        struct SLeaf
        {
            SLeaf();
            ~SLeaf();

            bool                    m_bIsActive;
            float                   m_fAlphaTestValue;
            int                     m_nDiscreteLodLevel;
            unsigned short          m_usLeafCount;

            const unsigned char*    m_pLeafMapIndices;
            const unsigned char*    m_pLeafClusterIndices;
            const float*            m_pCenterCoords;
            const float**           m_pLeafMapTexCoords;
            const float**           m_pLeafMapCoords;

            const unsigned long*    m_pColors;
            const float*            m_pNormals;
            const float*            m_pBinormals;
            const float*            m_pTangents;
            const float*            m_pWindWeights;
            const unsigned char*    m_pWindMatrixIndices;
        };

        struct SBillboard
        {
            SBillboard();
            ~SBillboard();

            bool                    m_bIsActive;
            const float*            m_pTexCoords;
            const float*            m_pCoords;
            float                   m_fAlphaTestValue;
        };

        SIndexed    m_sBranches;
        float       m_fBranchAlphaTestValue;

        SIndexed    m_sFronds;
        float       m_fFrondAlphaTestValue;

        SLeaf       m_sLeaves0;
        SLeaf       m_sLeaves1;

        SBillboard  m_sBillboard0;
        SBillboard  m_sBillboard1;
        SBillboard  m_sHorizontalBillboard;
    };

    struct STextures
    {
        STextures();
        ~STextures();

        const char*         m_pBranchTextureFilename;
        unsigned int        m_uiLeafTextureCount;
        const char**        m_pLeafTextureFilenames;
        unsigned int        m_uiFrondTextureCount;
        const char**        m_pFrondTextureFilenames;
        const char*         m_pCompositeFilename;
        const char*         m_pSelfShadowFilename;
    };

    CSpeedTreeRT();
    virtual ~CSpeedTreeRT();

    // Instances
    bool                Compute(const float* pTransform = 0, unsigned int nSeed = 1, bool bCompositeStrips = true);
    CSpeedTreeRT*       Clone(float x = 0.0f, float y = 0.0f, float z = 0.0f, unsigned int nSeed = 0) const;
    const CSpeedTreeRT* InstanceOf(void) const;
    CSpeedTreeRT*       MakeInstance(void);
    void                DeleteTransientData(void);

    // Tree file loading
    bool                LoadTree(const char* pFilename);
    bool                LoadTree(const unsigned char* pBlock, unsigned int nNumBytes);
    unsigned char*      SaveTree(unsigned int& nNumBytes, bool bSaveLeaves = false) const;

    // Size / Position
    void                GetTreeSize(float& fSize, float& fVariance) const;
    void                SetTreeSize(float fNewSize, float fNewVariance = 0.0f);
    unsigned int        GetSeed() const;
    const float*        GetTreePosition(void) const;
    void                SetTreePosition(float x, float y, float z);
    void                SetLeafTargetAlphaMask(unsigned char ucMask = 0x54);

    // Lighting
    ELightingMethod     GetBranchLightingMethod(void) const;
    void                SetBranchLightingMethod(ELightingMethod eMethod);
    ELightingMethod     GetLeafLightingMethod(void) const;
    void                SetLeafLightingMethod(ELightingMethod eMethod);
    ELightingMethod     GetFrondLightingMethod(void) const;
    void                SetFrondLightingMethod(ELightingMethod eMethod);

    EStaticLightingStyle GetStaticLightingStyle(void) const;
    void                SetStaticLightingStyle(EStaticLightingStyle eStyle);
    float               GetLeafLightingAdjustment() const;
    void                SetLeafLightingAdjustment(float fScalar);

    static bool         GetLightState(unsigned int nLightIndex);
    static void         SetLightState(unsigned int nLightIndex, bool bLightOn);
    static const float* GetLightAttributes(unsigned int nLightIndex);
    static void         SetLightAttributes(unsigned int nLightIndex, const float* pLightAttributes);

    const float*        GetBranchMaterial(void) const;
    void                SetBranchMaterial(const float* pMaterial);
    const float*        GetLeafMaterial(void) const;
    void                SetLeafMaterial(const float* pMaterial);
    const float*        GetFrondMaterial(void) const;
    void                SetFrondMaterial(const float* pMaterial);

    // Camera
    static void         GetCamera(float* pPosition, float* pDirection);
    static void         SetCamera(const float* pPosition, const float* pDirection);

    // Wind
    static void         SetTime(float fTime);
    void                ComputeWindEffects(bool bBranches, bool bLeaves, bool bFronds = true);
    void                ResetLeafWindState(void);
    bool                GetLeafRockingState(void) const;
    void                SetLeafRockingState(bool bFlag);
    void                SetNumLeafRockingGroups(unsigned int nRockingGroups);

    EWindMethod         GetLeafWindMethod(void) const;
    void                SetLeafWindMethod(EWindMethod eMethod);
    EWindMethod         GetBranchWindMethod(void) const;
    void                SetBranchWindMethod(EWindMethod eMethod);
    EWindMethod         GetFrondWindMethod(void) const;
    void                SetFrondWindMethod(EWindMethod eMethod);

    float               GetWindStrength(void) const;
    float               SetWindStrength(float fNewStrength, float fOldStrength = -1.0f, float fFrequencyTimeOffset = -1.0f);

    static void         SetNumWindMatrices(unsigned int nNumMatrices);
    static void         SetWindMatrix(unsigned int nMatrixIndex, const float* pMatrix);
    void                GetLocalMatrices(unsigned int& nStartingIndex, unsigned int& nMatrixSpan);
    void                SetLocalMatrices(unsigned int nStartingMatrix, unsigned int nMatrixSpan);

    // LOD
    void                ComputeLodLevel(void);
    float               GetLodLevel(void) const;
    void                SetLodLevel(float fLodLevel);
    static void         SetDropToBillboard(bool bFlag);

    void                GetLodLimits(float& fNear, float& fFar) const;
    void                SetLodLimits(float fNear, float fFar);

    short               GetDiscreteBranchLodLevel(float fLodLevel = -1.0f) const;
    unsigned short      GetDiscreteLeafLodLevel(float fLodLevel = -1.0f) const;
    short               GetDiscreteFrondLodLevel(float fLodLevel = -1.0f) const;

    unsigned short      GetNumBranchLodLevels(void) const;
    unsigned short      GetNumLeafLodLevels(void) const;
    unsigned short      GetNumFrondLodLevels(void) const;

    // Geometry
    void                DeleteBranchGeometry(void);
    void                DeleteFrondGeometry(void);
    unsigned char*      GetFrondGeometryMapIndexes(int nLodLevel) const;
    const float*        GetLeafBillboardTable(unsigned int& nEntryCount) const;
    const float*        GetLeafLodSizeAdjustments(void);
    void                GetGeometry(SGeometry& sGeometry, unsigned long ulBitVector = SpeedTree_AllGeometry, short sOverrideBranchLodValue = -1, short sOverrideFrondLodValue = -1, short sOverrideLeafLodValue = -1);

    // Textures
    void                GetTextures(STextures& sTextures) const;
    void                SetLeafTextureCoords(unsigned int nLeafMapIndex, const float* pTexCoords);
    void                SetFrondTextureCoords(unsigned int nFrondMapIndex, const float* pTexCoords);
    static bool         GetTextureFlip(void);
    static void         SetTextureFlip(bool bFlag);
    void                SetBranchTextureFilename(const char* pFilename);
    void                SetLeafTextureFilename(unsigned int nLeafMapIndex, const char* pFilename);
    void                SetFrondTextureFilename(unsigned int nFrondMapIndex, const char* pFilename);

    // Info & Error
    static void         Authorize(const char* pKey);
    static bool         IsAuthorized(void);
    static const char*  GetCurrentError(void);
    static void         ResetError(void);

    void                GetBoundingBox(float* pBounds) const;
    unsigned int        GetLeafTriangleCount(float fLodLevel = -1.0f) const;
    unsigned int        GetBranchTriangleCount(float fLodLevel = -1.0f) const;
    unsigned int        GetFrondTriangleCount(float fLodLevel = -1.0f) const;

    // Collisions
    unsigned int        GetCollisionObjectCount(void);
    void                GetCollisionObject(unsigned int nIndex, ECollisionObjectType& eType, float* pPosition, float* pDimensions);

    // User data
    const char*         GetUserData(void) const;

private:
    struct SCollisionObj
    {
        ECollisionObjectType m_eType;
        float m_afPos[3];
        float m_afDims[3];
    };

    float                       m_afPos[3];
    float                       m_afBoundingBox[6];
    float                       m_fSize;
    float                       m_fSizeVariance;
    unsigned int                m_nSeed;
    float                       m_fWindStrength;
    float                       m_fLeafLightingAdjustment;
    ELightingMethod             m_eBranchLighting;
    ELightingMethod             m_eLeafLighting;
    ELightingMethod             m_eFrondLighting;
    EWindMethod                 m_eBranchWind;
    EWindMethod                 m_eLeafWind;
    EWindMethod                 m_eFrondWind;
    float                       m_afBranchMaterial[13];
    float                       m_afLeafMaterial[13];
    float                       m_afFrondMaterial[13];

    // Texture filenames
    std::string                 m_strBranchTexture;
    std::string                 m_strCompositeTexture;
    std::string                 m_strSelfShadowTexture;
    std::vector<std::string>    m_vLeafTextures;
    mutable std::vector<const char*> m_vpLeafTextures;
    std::vector<std::string>    m_vFrondTextures;
    mutable std::vector<const char*> m_vpFrondTextures;

    // Collisions
    std::vector<SCollisionObj>  m_vCollisions;

    // ------------------------------------------------------------------
    // Procedurally generated geometry.
    // The .spt files shipped with Metin2 only contain generation parameters
    // (Bezier splines, sizes, texture names); the geometry itself was produced
    // by the proprietary SpeedTreeRT generator. This clean-room replacement
    // builds a deterministic tree (tapered trunk/limbs + leaf cards) so the
    // existing SpeedTreeLib renderer has geometry to draw.
    // ------------------------------------------------------------------
    void                        GenerateGeometry();
    void                        UpdateLeafBillboards();
    void                        UpdateLeafBillboardsAt(const float* pTreePos);
    bool                        LoadBakedGeometry(unsigned int nHash);

    enum ETreeShape
    {
        SHAPE_BROADLEAF,
        SHAPE_BIRCH,
        SHAPE_CONIFER,
        SHAPE_PALM,
        SHAPE_BAOBAB,
        SHAPE_SMALL
    };

    ETreeShape                  m_eShape;

    bool                        m_bGeometryGenerated;
    bool                        m_bBakedGeometry;
    float                       m_fBranchAlpha;
    float                       m_fFrondAlpha;
    float                       m_fLeafAlpha;
    std::vector<float>          m_vBranchCoords;
    std::vector<unsigned long>  m_vBranchColors;
    std::vector<float>          m_vBranchTexCoords0;
    std::vector<float>          m_vBranchTexCoords1;
    std::vector<unsigned short> m_vBranchStrip;
    unsigned short              m_usBranchStripLength;
    std::vector<const unsigned short*> m_vpBranchStrips;

    std::vector<float>          m_vFrondCoords;
    std::vector<unsigned long>  m_vFrondColors;
    std::vector<float>          m_vFrondTexCoords0;
    std::vector<float>          m_vFrondTexCoords1;
    std::vector<unsigned short> m_vFrondStrip;
    unsigned short              m_usFrondStripLength;
    std::vector<const unsigned short*> m_vpFrondStrips;

    std::vector<float>          m_vLeafCenters;
    std::vector<float>          m_vLeafHalfW;
    std::vector<float>          m_vLeafHalfH;
    std::vector<unsigned long>  m_vLeafColors;
    std::vector<float>          m_vLeafSize;
    std::vector<float>          m_vLeafAngle;
    std::vector<std::array<float, 16> > m_vLeafCoords;
    std::vector<std::array<float, 8> >  m_vLeafTexCoords;
    std::vector<const float*>   m_vpLeafCoords;
    std::vector<const float*>   m_vpLeafTexCoords;
    std::vector<unsigned char>  m_vLeafMapIndices;
    std::vector<unsigned char>  m_vLeafClusterIndices;

    // Instancing
    const CSpeedTreeRT*         m_pInstanceOf;

    // Static states
    static std::string          ms_strError;
    static bool                 ms_bTextureFlip;
    static bool                 ms_bDropToBillboard;
    static float                ms_afCameraPos[3];
    static float                ms_afCameraDir[3];
    static float                ms_afLightAttrs[16];
    static bool                 ms_bLightOn;
    static unsigned int         ms_unNumWindMatrices;
};
