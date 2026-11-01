#pragma once

class CFlyingData //: public CPooledObject<CFlyingData>
{
  public:
    enum
    {
        FLY_ATTACH_NONE,
        FLY_ATTACH_EFFECT,
        FLY_ATTACH_OBJECT, // NOT IMPLEMENTED
    };

    enum
    {
        FLY_ATTACH_TYPE_NONE,
        FLY_ATTACH_TYPE_LINE,
        FLY_ATTACH_TYPE_MULTI_LINE,
        FLY_ATTACH_TYPE_SINE,
        FLY_ATTACH_TYPE_EXP,
    };

    struct TFlyingAttachData
    {
        int iType = 0;
        int iFlyType = 0;
        std::string strFilename;

        bool bHasTail = false;
        DWORD dwTailColor = 0U;
        float fTailLength = 0.0F;
        float fTailSize = 0.0F;
        bool bRectShape = false;

        float fRoll = 0.0F;
        // nothing for LINE
        // for MULTI_LINE
        float fDistance = 0.0F;
        // for SINE, EXP
        float fPeriod = 0.0F;
        float fAmplitude = 0.0F;
    };

    friend class CFlyingInstance;

    friend class CSceneFly;
    friend class CFlyTabPage;

    CFlyingData();
    virtual ~CFlyingData();

    void Destroy();

    bool LoadScriptFile(const char *c_szFilename);
    bool SaveScriptFile(const char *c_szFilename);

    const D3DXVECTOR3 &GetAcceleration()
    {
        return m_v3Accel;
    }

    void SetBombEffect(const char *szEffectName);

    DWORD AttachFlyEffect(int iType, const std::string &strFilename, float fRoll, float fArg1, float fArg2);

    TFlyingAttachData &GetAttachDataReference(int iIndex);
    int GetAttachDataCount()
    {
        return m_AttachDataVector.size();
    }

    void DuplicateAttach(int iIndex);
    void RemoveAttach(int iIndex);
    void RemoveAllAttach();

  protected:
    void __Initialize();

    std::string m_strFilename;

    bool m_bSpreading = false;
    bool m_bMaintainParallel = false;

    float m_fInitVel = 0.0F;
    float m_fConeAngle = 0.0F;
    float m_fRollAngle = 0.0F;

    D3DXVECTOR3 m_v3AngVel;

    float m_fGravity = 0.0F;

    float m_fBombRange = 0.0F;
    std::string m_strBombEffectName;
    DWORD m_dwBombEffectID = 0U;

    bool m_bIsHoming = false;
    float m_fHomingMaxAngle = 0.0F;
    float m_fHomingStartTime = 0.0F;

    bool m_bHitOnBackground = false;
    bool m_bHitOnAnotherMonster = false;
    int m_iPierceCount = 0;
    float m_fCollisionSphereRadius = 0.0F;

    float m_fRange = 0.0F;
    D3DXVECTOR3 m_v3Accel;

    std::vector<TFlyingAttachData> m_AttachDataVector;

  public:
    static CFlyingData *New()
    {
        return ms_kPool.Alloc();
    }
    static void Delete(CFlyingData *pData)
    {
        pData->Destroy();
        ms_kPool.Free(pData);
    }

    static void DestroySystem()
    {
        ms_kPool.Destroy();
    }

    static CDynamicPool<CFlyingData> ms_kPool;
};