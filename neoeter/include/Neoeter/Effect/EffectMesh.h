#pragma once

#include <Platform/Platform.h>
#include <Platform/DirectXMathCompat.h>

#include <Neoeter/Render/GrpScreen.h>
#include <Neoeter/Render/Resource.h>
#include <Neoeter/Render/GrpImageInstance.h>
#include <Neoeter/Render/TextFileLoader.h>

#include "Type.h"
#include "EffectElementBase.h"

class CEffectMesh : public CResource
{
  public:
    using TEffectFrameData = struct SEffectFrameData
    {
        BYTE byChangedFrame = 0U;
        float fVisibility = 0.0F;
        DWORD dwVertexCount = 0U;
        DWORD dwTextureVertexCount = 0U;
        DWORD dwIndexCount = 0U;
        std::vector<TPTVertex> PDTVertexVector;
    };

    using TEffectMeshData = struct SEffectMeshData
    {
        char szObjectName[32]{};
        char szDiffuseMapFileName[128]{};

        std::vector<TEffectFrameData> EffectFrameDataVector;
        std::vector<CGraphicImage *> pImageVector;

        static SEffectMeshData *New();
        static void Delete(SEffectMeshData *pkData);

        static void DestroySystem();

        static CDynamicPool<SEffectMeshData> ms_kPool;
    };

    // About Resource Code

    using TRef = CRef<CEffectMesh>;

    static TType Type();

    CEffectMesh(const char *c_szFileName);
    ~CEffectMesh() override;

    DWORD GetFrameCount() const;
    DWORD GetMeshCount();
    TEffectMeshData *GetMeshDataPointer(DWORD dwMeshIndex);

    std::vector<CGraphicImage *> *GetTextureVectorPointer(DWORD dwMeshIndex);
    std::vector<CGraphicImage *> &GetTextureVectorReference(DWORD dwMeshIndex);

    // Exceptional function for tool
    BOOL GetMeshElementPointer(DWORD dwMeshIndex, TEffectMeshData **ppMeshData);

  protected:
    bool OnLoad(int iSize, const void *c_pvBuf) override;

    void OnClear() override;
    bool OnIsEmpty() const override;
    bool OnIsType(TType type) override;

    BOOL __LoadData_Ver001(int iSize, const BYTE *c_pbBuf);
    BOOL __LoadData_Ver002(int iSize, const BYTE *c_pbBuf);

    int m_iGeomCount;
    int m_iFrameCount;
    std::vector<TEffectMeshData *> m_pEffectMeshDataVector;

    bool m_isData;
};

class CEffectMeshScript : public CEffectElementBase
{
  public:
    using TMeshData = struct SMeshData
    {
        BYTE byBillboardType = 0U;

        BOOL bBlendingEnable = 0;
        BYTE byBlendingSrcType = 0U;
        BYTE byBlendingDestType = 0U;
        BOOL bTextureAlphaEnable = 0;

        BYTE byColorOperationType = 0U;
        D3DXCOLOR ColorFactor;

        BOOL bTextureAnimationLoopEnable = 0;
        float fTextureAnimationFrameDelay = 0.0F;

        DWORD dwTextureAnimationStartFrame = 0U;

        TTimeEventTableFloat TimeEventAlpha;

        SMeshData()
        {
            TimeEventAlpha.clear();
        }
    };
    using TMeshDataVector = std::vector<TMeshData>;

    CEffectMeshScript();
    ~CEffectMeshScript() override;

    const char *GetMeshFileName();

    void ReserveMeshData(DWORD dwMeshCount);
    bool CheckMeshIndex(DWORD dwMeshIndex);
    bool GetMeshDataPointer(DWORD dwMeshIndex, TMeshData **ppMeshData);
    int GetMeshDataCount();

    int GetBillboardType(DWORD dwMeshIndex);
    BOOL isBlendingEnable(DWORD dwMeshIndex);
    BYTE GetBlendingSrcType(DWORD dwMeshIndex);
    BYTE GetBlendingDestType(DWORD dwMeshIndex);
    BOOL isTextureAlphaEnable(DWORD dwMeshIndex);
    BOOL GetColorOperationType(DWORD dwMeshIndex, BYTE *pbyType);
    BOOL GetColorFactor(DWORD dwMeshIndex, D3DXCOLOR *pColor);
    BOOL GetTimeTableAlphaPointer(DWORD dwMeshIndex, TTimeEventTableFloat **pTimeEventAlpha);

    BOOL isMeshAnimationLoop() const;
    BOOL GetMeshAnimationLoopCount() const;
    float GetMeshAnimationFrameDelay() const;
    BOOL isTextureAnimationLoop(DWORD dwMeshIndex);
    float GetTextureAnimationFrameDelay(DWORD dwMeshIndex);
    DWORD GetTextureAnimationStartFrame(DWORD dwMeshIndex);

  protected:
    void OnClear() override;
    bool OnIsData() override;
    BOOL OnLoadScript(CTextFileLoader &rTextFileLoader) override;

    BOOL m_isMeshAnimationLoop = 0;
    int m_iMeshAnimationLoopCount = 0;
    float m_fMeshAnimationFrameDelay = 0.0F;
    TMeshDataVector m_MeshDataVector;

    std::string m_strMeshFileName;

  public:
    static void DestroySystem();

    static CEffectMeshScript *New();
    static void Delete(CEffectMeshScript *pkData);

    static CDynamicPool<CEffectMeshScript> ms_kPool;
};
