#pragma once

#include <Neoeter/Render/SkyBox.h>
#include <Neoeter/Render/LensFlare.h>
#include <Neoeter/Render/ScreenFilter.h>

#include <Neoeter/Terrain/TerrainType.h>
#include <Neoeter/Terrain/TextureSet.h>

#include <Neoeter/Model/SpeedTree/CSpeedTreeDirectX.h>

#include <Client/Game/MapBase.h>
#include <Client/Game/Area.h>
#include <Client/Game/AreaTerrain.h>
#include <Client/Game/AreaLoaderThread.h>

#include <Client/Game/MonsterAreaInfo.h>

#include <algorithm>

#define LOAD_SIZE_WIDTH 1

#define AROUND_AREA_NUM (1 + (LOAD_SIZE_WIDTH * 2) * (LOAD_SIZE_WIDTH * 2) * 2)
#define MAX_PREPARE_SIZE 9
#define MAX_MAPSIZE 256 // 0 ~ 255, cellsize 200 = 64km

#define TERRAINPATCH_LODMAX 3

using TOutdoorMapCoordinate = struct SOutdoorMapCoordinate
{
    short m_sTerrainCoordX; // Terrain 좌표
    short m_sTerrainCoordY;
};

using TOutdoorMapCoordinateMap = std::map<const std::string, TOutdoorMapCoordinate>;

class CTerrainPatchProxy;
class CTerrainQuadtreeNode;

class CMapOutdoor : public CMapBase
{
  public:
    enum
    {
        VIEW_NONE = 0,
        VIEW_PART,
        VIEW_ALL,
    };

    enum EPart
    {
        PART_TERRAIN,
        PART_OBJECT,
        PART_CLOUD,
        PART_WATER,
        PART_TREE,
        PART_SKY,
        PART_NUM,
    };

    enum ETerrainRenderSort
    {
        DISTANCE_SORT,
        TEXTURE_SORT,
    };

    CMapOutdoor();
    ~CMapOutdoor() override;

    void OnBeginEnvironment() override;

  protected:
    bool Initialize() override;
    void InitializeFog();

    bool Destroy() override;
    void OnSetEnvironmentDataPtr() override;
    void OnResetEnvironmentDataPtr() override;

    void OnRender() override;

    virtual void OnPreAssignTerrainPtr() {};

  public:
    void SetInverseViewAndDynamicShaodwMatrices();
    bool Load(float x, float y, float z) override;
    float GetHeight(float x, float y) override;
    virtual float GetCacheHeight(float x, float y);

    bool Update(float fX, float fY, float fZ) override;
    void UpdateAroundAmbience(float fX, float fY, float fZ) override;

    void Clear() override;

    void SetVisiblePart(int ePart, bool isVisible);
    void SetSplatLimit(int iSplatNum);
    std::vector<int> &GetRenderedSplatNum(int *piPatch, int *piSplat, float *pfSplatRatio);
    CArea::TCRCWithNumberVector &GetRenderedGraphicThingInstanceNum(DWORD *pdwGraphicThingInstanceNum,
                                                                    DWORD *pdwCRCNum) const;

    bool LoadSetting(const char *c_szFileName);

    void ApplyLight(DWORD dwVersion, const D3DLIGHT9 &c_rkLight) override;
    void SetEnvironmentScreenFilter();
    void SetEnvironmentSkyBox();
    void SetEnvironmentLensFlare();

    void CreateCharacterShadowTexture();
    void ReleaseCharacterShadowTexture();
    void SetShadowTextureSize(WORD size);

    bool BeginRenderCharacterShadowToTexture();
    void EndRenderCharacterShadowToTexture();
    void RenderWater();
    void RenderMarkedArea();
    void RecurseRenderAttr(CTerrainQuadtreeNode *Node, bool bCullEnable = TRUE);
    void DrawPatchAttr(long patchnum);
    void ClearGuildArea();
    void RegisterGuildArea(int isx, int isy, int iex, int iey);

    void VisibleMarkedArea();
    void DisableMarkedArea();

    void UpdateSky();
    void RenderCollision();
    void RenderSky();
    void RenderCloud();
    void RenderBeforeLensFlare();
    void RenderAfterLensFlare();
    void RenderScreenFiltering();

    void SetWireframe(bool bWireFrame);
    bool IsWireframe() const;

    bool GetPickingPointWithRay(const CRay &rRay, D3DXVECTOR3 *v3IntersectPt);
    bool GetPickingPointWithRayOnlyTerrain(const CRay &rRay, D3DXVECTOR3 *v3IntersectPt);
    bool GetPickingPoint(D3DXVECTOR3 *v3IntersectPt);
    void GetTerrainCount(short *psTerrainCountX, short *psTerrainCountY) const
    {
        *psTerrainCountX = m_sTerrainCountX;
        *psTerrainCountY = m_sTerrainCountY;
    }

    bool SetTerrainCount(short sTerrainCountX, short sTerrainCountY);

    // Shadow
    void SetDrawShadow(bool bDrawShadow);
    void SetDrawCharacterShadow(bool bDrawChrShadow);

    DWORD GetShadowMapColor(float fx, float fy);

  protected:
    bool __PickTerrainHeight(float &fPos, const D3DXVECTOR3 &v3Start, const D3DXVECTOR3 &v3End, float fStep,
                             float fRayRange, float fLimitRange, D3DXVECTOR3 *pv3Pick);

    virtual void __ClearGarvage();
    virtual void __UpdateGarvage();

    virtual bool LoadTerrain(WORD wTerrainCoordX, WORD wTerrainCoordY, WORD wCellCoordX, WORD wCellCoordY);
    virtual bool LoadArea(WORD wAreaCoordX, WORD wAreaCoordY, WORD wCellCoordX, WORD wCellCoordY);
    virtual void UpdateAreaList(long lCenterX, long lCenterY);
    bool isTerrainLoaded(WORD wX, WORD wY);
    bool isAreaLoaded(WORD wX, WORD wY);

    void AssignTerrainPtr(); // 현재 좌표에서 주위(ex. 3x3)에 있는 것들의
                             // 포인터를 연결한다. (업데이트 시 불려짐)

    void SaveAlphaFogOperation();
    void RestoreAlphaFogOperation();

    //////////////////////////////////////////////////////////////////////////
    // New
    //////////////////////////////////////////////////////////////////////////
    // 여러가지 맵들을 얻는다.
    void GetHeightMap(const BYTE &c_rucTerrainNum, WORD **pwHeightMap);
    void GetNormalMap(const BYTE &c_rucTerrainNum, char **pucNormalMap);

    // Water
    void GetWaterMap(const BYTE &c_rucTerrainNum, BYTE **pucWaterMap);
    void GetWaterHeight(BYTE byTerrainNum, BYTE byWaterNum, long *plWaterHeight);

    //////////////////////////////////////////////////////////////////////////
    // Terrain
    //////////////////////////////////////////////////////////////////////////

    // 데이터
    CTerrain *m_pTerrain[AROUND_AREA_NUM]{};                // Terrain
    CTerrainPatchProxy *m_pTerrainPatchProxyList = nullptr; // CTerrain을 랜더링 할때 실제로 랜더링하는
                                                            // 폴리곤 패치들... Seamless Map 을 위해
                                                            // CTerrain으로부터 독립...

    long m_lViewRadius = 0L;     // 시야 거리.. 셀단위임..
    float m_fHeightScale = 0.0F; // 높이 스케일... 1.0일때 0~655.35미터까지 표현 가능.

    short m_sTerrainCountX = 0,
          m_sTerrainCountY = 0; // seamless map 안에 들어가는 Terrain개수

    TOutdoorMapCoordinate m_CurCoordinate{}; // 현재의 좌표

    long m_lCurCoordStartX = 0L, m_lCurCoordStartY = 0L;
    TOutdoorMapCoordinate m_PrevCoordinate{}; // 현재의 좌표
    TOutdoorMapCoordinateMap m_EntryPointMap;

    WORD m_wPatchCount = 0U;

    //////////////////////////////////////////////////////////////////////////
    // Index Buffer
#ifdef WORLD_EDITOR
    WORD *m_pwIndices; /* temp Index buffer */

    CGraphicIndexBuffer m_IndexBuffer;
    WORD m_wNumIndices;
#else
    WORD *m_pwaIndices[TERRAINPATCH_LODMAX]{};

    CGraphicIndexBuffer m_IndexBuffer[TERRAINPATCH_LODMAX];
    WORD m_wNumIndices[TERRAINPATCH_LODMAX]{};
#endif
    virtual void DestroyTerrain();

    void CreateTerrainPatchProxyList();
    void DestroyTerrainPatchProxyList();

    void UpdateTerrain(float fX, float fY);

    void ConvertTerrainToTnL(long lx, long ly);

    void AssignPatch(long lPatchNum, long lx0, long ly0, long lx1, long ly1);

    //////////////////////////////////////////////////////////////////////////
    // Index Buffer
    void ADDLvl1TL(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl1T(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl1TR(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl1L(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl1R(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl1BL(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl1B(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl1BR(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl1M(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl2TL(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl2T(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl2TR(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl2L(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl2R(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl2BL(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl2B(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl2BR(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);
    void ADDLvl2M(WORD *pIndices, WORD &rwCount, const WORD &c_rwCurCount, const BYTE &c_rucNumLineWarp);

  public:
    BOOL GetTerrainPointer(BYTE c_byTerrainNum, CTerrain **ppTerrain);
    float GetTerrainHeight(float fx, float fy);
    bool GetWaterHeight(int iX, int iY, long *plWaterHeight);
    bool GetNormal(int ix, int iy, D3DXVECTOR3 *pv3Normal);

    void RenderTerrain();

    long GetViewRadius() const
    {
        return m_lViewRadius;
    }
    float GetHeightScale() const
    {
        return m_fHeightScale;
    }

    const TOutdoorMapCoordinate &GetEntryPoint(const std::string &c_rstrEntryPointName) const;
    void SetEntryPoint(const std::string &c_rstrEntryPointName, const TOutdoorMapCoordinate &c_rOutdoorMapCoordinate);
    const TOutdoorMapCoordinate &GetCurCoordinate()
    {
        return m_CurCoordinate;
    }
    const TOutdoorMapCoordinate &GetPrevCoordinate()
    {
        return m_PrevCoordinate;
    }

    //////////////////////////////////////////////////////////////////////////
    // Area
    //////////////////////////////////////////////////////////////////////////
  protected:
    CArea *m_pArea[AROUND_AREA_NUM]{}; // Data

    virtual void DestroyArea();

    void __UpdateArea(D3DXVECTOR3 &v3Player);
#ifdef WORLD_EDITOR
    void __NEW_WorldEditor_UpdateArea();
#endif
    void __Game_UpdateArea(D3DXVECTOR3 &v3Player);

    void __BuildDynamicSphereInstanceVector();

    void __CollectShadowReceiver(D3DXVECTOR3 &v3Target, D3DXVECTOR3 &v3Light);
    void __CollectCollisionPCBlocker(D3DXVECTOR3 &v3Eye, D3DXVECTOR3 &v3Target, float fDistance);
    void __CollectCollisionShadowReceiver(D3DXVECTOR3 &v3Target, D3DXVECTOR3 &v3Light);
    void __UpdateAroundAreaList();
    bool __IsInShadowReceiverList(CGraphicObjectInstance *pkObjInstTest);
    bool __IsInPCBlockerList(CGraphicObjectInstance *pkObjInstTest);

    void ConvertToMapCoords(float fx, float fy, int *iCellX, int *iCellY, BYTE *pucSubCellX, BYTE *pucSubCellY,
                            WORD *pwTerrainNumX, WORD *pwTerrainNumY);

  public:
    BOOL GetAreaPointer(BYTE c_byAreaNum, CArea **ppArea);
    void RenderArea(bool bRenderAmbience = true);
    void RenderBlendArea();
    void RenderDungeon();
    void RenderEffect();
    void RenderPCBlocker();
    void RenderTree();

    //////////////////////////////////////////////////////////////////////////
    // For Grass
    //////////////////////////////////////////////////////////////////////////
    float GetHeight(float *pPos);
    bool GetBrushColor(float fX, float fY, float *pLowColor, float *pHighColor);
    bool isAttrOn(float fX, float fY, BYTE byAttr);
    bool GetAttr(float fX, float fY, BYTE *pbyAttr);
    bool isAttrOn(int iX, int iY, BYTE byAttr);
    bool GetAttr(int iX, int iY, BYTE *pbyAttr);

    void SetMaterialDiffuse(float fr, float fg, float fb);
    void SetMaterialAmbient(float fr, float fg, float fb);
    void SetTerrainMaterial(const PR_MATERIAL *pMaterial);

    bool GetTerrainNum(float fx, float fy, BYTE *pbyTerrainNum);
    bool GetTerrainNumFromCoord(WORD wCoordX, WORD wCoordY, BYTE *pbyTerrainNum) const;

  protected:
    //////////////////////////////////////////////////////////////////////////
    // New
    //////////////////////////////////////////////////////////////////////////
    long m_lCenterX = 0L, m_lCenterY = 0L;   // Terrain 좌표 내의 셀 좌표...
    long m_lOldReadX = 0L, m_lOldReadY = 0L; /* Last center */

    //////////////////////////////////////////////////////////////////////////
    // Octree
    //////////////////////////////////////////////////////////////////////////
    CTerrainQuadtreeNode *m_pRootNode = nullptr;

    void BuildQuadTree();
    CTerrainQuadtreeNode *AllocQuadTreeNode(long x0, long y0, long x1, long y1) const;
    void SubDivideNode(CTerrainQuadtreeNode *Node);
    void UpdateQuadTreeHeights(CTerrainQuadtreeNode *Node);

    void FreeQuadTree();

    struct TPatchDrawStruct
    {
        float fDistance;
        BYTE byTerrainNum;
        long lPatchNum;
        CTerrainPatchProxy *pTerrainPatchProxy;

        bool operator<(const TPatchDrawStruct &rhs) const
        {
            return fDistance < rhs.fDistance;
        }
    };

  public:
    using TTerrainNumVector = std::vector<BYTE>;
    struct FSortPatchDrawStructWithTerrainNum
    {
        static TTerrainNumVector m_TerrainNumVector;
        FSortPatchDrawStructWithTerrainNum()
        {
            m_TerrainNumVector.clear();
        }

        bool operator()(const TPatchDrawStruct &lhs, const TPatchDrawStruct &rhs)
        {
            DWORD lhsTerrainNumOrder = 0;
            DWORD rhsTerrainNumOrder = 0;
            bool blhsOrderFound = false;
            bool brhsOrderFound = false;

            auto lhsIterator = std::ranges::find(m_TerrainNumVector, lhs.byTerrainNum);
            auto rhsIterator = std::ranges::find(m_TerrainNumVector, rhs.byTerrainNum);

            if (lhsIterator != m_TerrainNumVector.end())
            {
                blhsOrderFound = true;
                lhsTerrainNumOrder = lhsIterator - m_TerrainNumVector.begin();
            }
            if (rhsIterator != m_TerrainNumVector.end())
            {
                brhsOrderFound = true;
                rhsTerrainNumOrder = rhsIterator - m_TerrainNumVector.begin();
            }
            if (!brhsOrderFound)
            {
                m_TerrainNumVector.push_back(rhs.byTerrainNum);
                rhsTerrainNumOrder = m_TerrainNumVector.size() - 1;
            }
            if (!blhsOrderFound)
            {
                lhsIterator = std::ranges::find(m_TerrainNumVector, lhs.byTerrainNum);
                if (lhsIterator != m_TerrainNumVector.end())
                {
                    blhsOrderFound = true;
                    lhsTerrainNumOrder = lhsIterator - m_TerrainNumVector.begin();
                }
                if (!blhsOrderFound)
                {
                    m_TerrainNumVector.push_back(lhs.byTerrainNum);
                    lhsTerrainNumOrder = m_TerrainNumVector.size() - 1;
                }
            }

            return lhsTerrainNumOrder < rhsTerrainNumOrder;
        }
    };

  protected:
    std::vector<std::pair<float, long>> m_PatchVector;
    std::vector<TPatchDrawStruct> m_PatchDrawStructVector;

    void SetPatchDrawVector();

    void NEW_DrawWireFrame(CTerrainPatchProxy *pTerrainPatchProxy, WORD wPrimitiveCount,
                           D3DPRIMITIVETYPE ePrimitiveType) const;

    void DrawWireFrame(long patchnum, WORD wPrimitiveCount, D3DPRIMITIVETYPE ePrimitiveType);
    void DrawWater(long patchnum);

    bool m_bDrawWireFrame = false;
    bool m_bDrawShadow = false;
    bool m_bDrawChrShadow = false;

    //////////////////////////////////////////////////////////////////////////
    // Water
    D3DXMATRIX m_matBump;
    void LoadWaterTexture();
    void UnloadWaterTexture();
    // Water
    //////////////////////////////////////////////////////////////////////////

    //////////////////////////////////////////////////////////////////////////
    // Alpha Fog
    CGraphicImageInstance m_AlphaFogImageInstance;
    D3DXMATRIX m_matAlphaFogTexture;
    // Alpha Fog
    //////////////////////////////////////////////////////////////////////////

    //////////////////////////////////////////////////////////////////////////
    // Character Shadow
    LPDIRECT3DTEXTURE9 m_lpCharacterShadowMapTexture = nullptr;
    LPDIRECT3DSURFACE9 m_lpCharacterShadowMapRenderTargetSurface = nullptr;
    LPDIRECT3DSURFACE9 m_lpCharacterShadowMapDepthSurface = nullptr;
    D3DVIEWPORT9 m_ShadowMapViewport{};
    WORD m_wShadowMapSize = 0U;

    // Backup Device Context
    LPDIRECT3DSURFACE9 m_lpBackupRenderTargetSurface = nullptr;
    LPDIRECT3DSURFACE9 m_lpBackupDepthSurface = nullptr;
    D3DVIEWPORT9 m_BackupViewport{};

    // Character Shadow
    //////////////////////////////////////////////////////////////////////////

    // View Frustum Culling
    D3DXPLANE m_plane[6];

    void BuildViewFrustum(const D3DXMATRIX &mat);

    CTextureSet m_TextureSet;

    CSkyBox m_SkyBox;
    CLensFlare m_LensFlare;
    CScreenFilter m_ScreenFilter;

    void SetIndexBuffer();
    void SelectIndexBuffer(BYTE byLODLevel, WORD *pwPrimitiveCount, D3DPRIMITIVETYPE *pePrimitiveType);

    D3DXMATRIX m_matWorldForCommonUse;
    D3DXMATRIX m_matViewInverse;

    D3DXMATRIX m_matSplatAlpha;
    D3DXMATRIX m_matStaticShadow;
    D3DXMATRIX m_matDynamicShadow;
    D3DXMATRIX m_matDynamicShadowScale;
    D3DXMATRIX m_matLightView;

    float m_fTerrainTexCoordBase = 0.0F;
    float m_fWaterTexCoordBase = 0.0F;

    float m_fXforDistanceCaculation = 0.0F, m_fYforDistanceCaculation = 0.0F;

    using TTerrainPtrVector = std::vector<CTerrain *>;
    using TTerrainPtrVectorIterator = TTerrainPtrVector::iterator;
    using TAreaPtrVector = std::vector<CArea *>;
    using TAreaPtrVectorIterator = TAreaPtrVector::iterator;

    TTerrainPtrVector m_TerrainVector;
    TTerrainPtrVector m_TerrainDeleteVector;
    TTerrainPtrVector m_TerrainLoadRequestVector;
    TTerrainPtrVector m_TerrainLoadWaitVector;
    TTerrainPtrVectorIterator m_TerrainPtrVectorIterator;

    TAreaPtrVector m_AreaVector;
    TAreaPtrVector m_AreaDeleteVector;
    TAreaPtrVector m_AreaLoadRequestVector;
    TAreaPtrVector m_AreaLoadWaitVector;
    TAreaPtrVectorIterator m_AreaPtrVectorIterator;

    struct FPushToDeleteVector
    {
        enum EDeleteDir
        {
            DELETE_LEFT,
            DELETE_RIGHT,
            DELETE_TOP,
            DELETE_BOTTOM,
        };

        EDeleteDir m_eLRDeleteDir;
        EDeleteDir m_eTBDeleteDir;
        TOutdoorMapCoordinate m_CurCoordinate{};

        FPushToDeleteVector(EDeleteDir eLRDeleteDir, EDeleteDir eTBDeleteDir, TOutdoorMapCoordinate CurCoord)
            : m_eLRDeleteDir(eLRDeleteDir), m_CurCoordinate(CurCoord), m_eTBDeleteDir(eTBDeleteDir)
        {
        }
    };

    struct FPushTerrainToDeleteVector : public FPushToDeleteVector
    {
        TTerrainPtrVector m_ReturnTerrainVector;

        FPushTerrainToDeleteVector(EDeleteDir eLRDeleteDir, EDeleteDir eTBDeleteDir, TOutdoorMapCoordinate CurCoord)
            : FPushToDeleteVector(eLRDeleteDir, eTBDeleteDir, CurCoord)
        {
            m_ReturnTerrainVector.clear();
        }

        void operator()(CTerrain *pTerrain);
    };

    struct FPushAreaToDeleteVector : public FPushToDeleteVector
    {
        TAreaPtrVector m_ReturnAreaVector;

        FPushAreaToDeleteVector(EDeleteDir eLRDeleteDir, EDeleteDir eTBDeleteDir, TOutdoorMapCoordinate CurCoord)
            : FPushToDeleteVector(eLRDeleteDir, eTBDeleteDir, CurCoord)
        {
            m_ReturnAreaVector.clear();
        }

        void operator()(CArea *pArea);
    };

    void InitializeVisibleParts();
    bool IsVisiblePart(int ePart) const;

    float __GetNoFogDistance() const;
    float __GetFogDistance() const;

    DWORD m_dwVisiblePartFlags = 0U;

    int m_iRenderedSplatNumSqSum = 0;
    int m_iRenderedSplatNum = 0;
    int m_iRenderedPatchNum = 0;
    std::vector<int> m_RenderedTextureNumVector;
    int m_iSplatLimit = 0;

    int m_iPatchTerrainVertexCount = 0;
    int m_iPatchWaterVertexCount = 0;

    int m_iPatchTerrainVertexSize = 0;
    int m_iPatchWaterVertexSize = 0;

    DWORD m_dwRenderedCRCNum = 0U;
    DWORD m_dwRenderedGraphicThingInstanceNum = 0U;

    std::list<RECT> m_rkList_kGuildArea;

    void __RenderTerrain_RecurseRenderQuadTree(CTerrainQuadtreeNode *Node, bool bCullCheckNeed = true);
    int __RenderTerrain_RecurseRenderQuadTree_CheckBoundingCircle(const D3DXVECTOR3 &c_v3Center,
                                                                  const float &c_fRadius);

    void __RenderTerrain_AppendPatch(const D3DXVECTOR3 &c_rv3Center, float fDistance, long lPatchNum);

    void __RenderTerrain_RenderSoftwareTransformPatch();
    void __RenderTerrain_RenderHardwareTransformPatch();

    void __HardwareTransformPatch_RenderPatchSplat(long patchnum, WORD wPrimitiveCount,
                                                   D3DPRIMITIVETYPE ePrimitiveType);
    void __HardwareTransformPatch_RenderPatchNone(long patchnum, WORD wPrimitiveCount, D3DPRIMITIVETYPE ePrimitiveType);

    struct SoftwareTransformPatch_SData
    {
        enum
        {
            SPLAT_VB_NUM = 8,
            NONE_VB_NUM = 8,
        };

        IDirect3DVertexBuffer9 *m_pkVBSplat[SPLAT_VB_NUM];
        IDirect3DVertexBuffer9 *m_pkVBNone[NONE_VB_NUM];
        DWORD m_dwSplatPos;
        DWORD m_dwNonePos;
        DWORD m_dwLightVersion;
    } m_kSTPD{};

    struct SoftwareTransformPatch_SRenderState
    {
        D3DXMATRIX m_m4Proj;
        D3DXMATRIX m_m4Frustum;
        D3DXMATRIX m_m4DynamicShadow;
        D3DLIGHT9 m_kLight{};
        D3DMATERIAL9 m_kMtrl{};
        D3DXVECTOR3 m_v3Player;
        DWORD m_dwFogColor = 0U;
        float m_fScreenHalfWidth = 0.0F;
        float m_fScreenHalfHeight = 0.0F;

        float m_fFogNearDistance = 0.0F;
        float m_fFogFarDistance = 0.0F;
        float m_fFogNearTransZ = 0.0F;
        float m_fFogFarTransZ = 0.0F;
        float m_fFogLenInv = 0.0F;
    };

    struct SoftwareTransformPatch_STVertex
    {
        D3DXVECTOR4 kPosition;
    };

    struct SoftwareTransformPatch_STLVertex
    {
        D3DXVECTOR4 kPosition;
        DWORD dwDiffuse = 0U;
        DWORD dwFog = 0U;
        D3DXVECTOR2 kTexTile;
        D3DXVECTOR2 kTexAlpha;
        D3DXVECTOR2 kTexStaticShadow;
        D3DXVECTOR2 kTexDynamicShadow;
    };

    void __SoftwareTransformPatch_ApplyRenderState();
    void __SoftwareTransformPatch_RestoreRenderState(DWORD dwFogEnable);

    void __SoftwareTransformPatch_Initialize();
    bool __SoftwareTransformPatch_Create();
    void __SoftwareTransformPatch_Destroy();
    void __SoftwareTransformPatch_BuildPipeline(SoftwareTransformPatch_SRenderState &rkTPRS);
    void __SoftwareTransformPatch_BuildPipeline_BuildFogFuncTable(SoftwareTransformPatch_SRenderState &rkTPRS);
    bool __SoftwareTransformPatch_SetTransform(SoftwareTransformPatch_SRenderState &rkTPRS,
                                               SoftwareTransformPatch_STLVertex *akTransVertex,
                                               CTerrainPatchProxy &rkTerrainPatchProxy, UINT uTerrainX, UINT uTerrainY,
                                               bool isFogEnable, bool isDynamicShadow);

    bool __SoftwareTransformPatch_SetSplatStream(SoftwareTransformPatch_STLVertex *akSrcVertex);
    bool __SoftwareTransformPatch_SetShadowStream(SoftwareTransformPatch_STLVertex *akSrcVertex);

    void __SoftwareTransformPatch_ApplyStaticShadowRenderState();
    void __SoftwareTransformPatch_RestoreStaticShadowRenderState();

    void __SoftwareTransformPatch_ApplyFogShadowRenderState();
    void __SoftwareTransformPatch_RestoreFogShadowRenderState();
    void __SoftwareTransformPatch_ApplyDynamicShadowRenderState();
    void __SoftwareTransformPatch_RestoreDynamicShadowRenderState();
    void __SoftwareTransformPatch_RenderPatchSplat(SoftwareTransformPatch_SRenderState &rkTPRS, long patchnum,
                                                   WORD wPrimitiveCount, D3DPRIMITIVETYPE ePrimitiveType,
                                                   bool isFogEnable);
    void __SoftwareTransformPatch_RenderPatchNone(SoftwareTransformPatch_SRenderState &rkTPRS, long patchnum,
                                                  WORD wPrimitiveCount, D3DPRIMITIVETYPE ePrimitiveType);

    std::vector<CGraphicObjectInstance *> m_ShadowReceiverVector;
    std::vector<CGraphicObjectInstance *> m_PCBlockerVector;

    float m_fOpaqueWaterDepth = 0.0F;
    CGraphicImageInstance m_WaterInstances[30];

  public:
    float GetOpaqueWaterDepth() const
    {
        return m_fOpaqueWaterDepth;
    }
    void SetOpaqueWaterDepth(float fOpaqueWaterDepth)
    {
        m_fOpaqueWaterDepth = fOpaqueWaterDepth;
    }
    void SetTerrainRenderSort(ETerrainRenderSort eTerrainRenderSort)
    {
        m_eTerrainRenderSort = eTerrainRenderSort;
    }
    ETerrainRenderSort GetTerrainRenderSort()
    {
        return m_eTerrainRenderSort;
    }

  protected:
    ETerrainRenderSort m_eTerrainRenderSort;

    CGraphicImageInstance m_attrImageInstance;
    CGraphicImageInstance m_BuildingTransparentImageInstance;
    D3DXMATRIX m_matBuildingTransparent;

    CDynamicPool<CMonsterAreaInfo> m_kPool_kMonsterAreaInfo;
    TMonsterAreaInfoPtrVector m_MonsterAreaInfoPtrVector;
    TMonsterAreaInfoPtrVectorIterator m_MonsterAreaInfoPtrVectorIterator;

  public:
    bool LoadMonsterAreaInfo();

    CMonsterAreaInfo *AddMonsterAreaInfo(long lOriginX, long lOriginY, long lSizeX, long lSizeY);
    void RemoveAllMonsterAreaInfo();

    DWORD GetMonsterAreaInfoCount()
    {
        return m_MonsterAreaInfoPtrVector.size();
    }
    bool GetMonsterAreaInfoFromVectorIndex(DWORD dwMonsterAreaInfoVectorIndex, CMonsterAreaInfo **ppMonsterAreaInfo);

    CMonsterAreaInfo *AddNewMonsterAreaInfo(long lOriginX, long lOriginY, long lSizeX, long lSizeY,
                                            CMonsterAreaInfo::EMonsterAreaInfoType eMonsterAreaInfoType, DWORD dwVID,
                                            DWORD dwCount, CMonsterAreaInfo::EMonsterDir eMonsterDir);

    void GetBaseXY(DWORD *pdwBaseX, DWORD *pdwBaseY) const;
    void SetBaseXY(DWORD dwBaseX, DWORD dwBaseY);

    void SetTransparentTree(bool bTransparentTree)
    {
        m_bTransparentTree = bTransparentTree;
    }
    void EnableTerrainOnlyForHeight(bool bFlag)
    {
        m_bEnableTerrainOnlyForHeight = bFlag;
    }
    void EnablePortal(bool bFlag);
    bool IsEnablePortal() const
    {
        return m_bEnablePortal;
    }

  protected:
    DWORD m_dwBaseX = 0U;
    DWORD m_dwBaseY = 0U;

    D3DXVECTOR3 m_v3Player;

    bool m_bShowEntirePatchTextureCount = false;
    bool m_bTransparentTree = false;
    bool m_bEnableTerrainOnlyForHeight = false;
    bool m_bEnablePortal = false;

    // XMas
  private:
    struct SXMasTree
    {
        CSpeedTreeWrapper *m_pkTree;
        int m_iEffectID;
    } m_kXMas{};

    void __XMasTree_Initialize();
    void __XMasTree_Create(float x, float y, float z, const char *c_szTreeName, const char *c_szEffName);

  public:
    void XMasTree_Destroy();
    void XMasTree_Set(float x, float y, float z, const char *c_szTreeName, const char *c_szEffName);

    // Special Effect
  private:
    using TSpecialEffectMap = std::map<DWORD, int>;
    TSpecialEffectMap m_kMap_dwID_iEffectID;

  public:
    void SpecialEffect_Create(DWORD dwID, float x, float y, float z, const char *c_szEffName);
    void SpecialEffect_Delete(DWORD dwID);
    void SpecialEffect_Destroy();

  private:
    struct SHeightCache
    {
        struct SItem
        {
            DWORD m_dwKey;
            float m_fHeight;
        };

        enum
        {
            HASH_SIZE = 100,
        };

        std::vector<SItem> m_akVct_kItem[HASH_SIZE];

        bool m_isUpdated = false;
    } m_kHeightCache;

    void __HeightCache_Init();
    void __HeightCache_Update();

  public:
    void SetEnvironmentDataName(const std::string &strEnvironmentDataName);
    std::string &GetEnvironmentDataName();

  protected:
    std::string m_settings_envDataName;
    std::string m_envDataName;

  private:
    bool m_bSettingTerrainVisible = false;
};
