#pragma once

// change CatMull to cubic spline
#include <Neoeter/Model/Grn/ThingInstance.h>

class CWeaponTrace
{
    /*
    public:
        typedef struct SSplineValue
        {
            float fValue1;
            float fValue2;
            float fValue3;
        } TSplineValue;
    */
  public:
    static void DestroySystem();
    static void Delete(CWeaponTrace *pkWTDel);
    static CWeaponTrace *New();

    CWeaponTrace();
    virtual ~CWeaponTrace();

    void Clear();

    void TurnOn();
    void TurnOff();

    void UseAlpha();
    void UseTexture();

    void SetTexture(const char *c_szFileName);
    bool SetWeaponInstance(CGraphicThingInstance *pInstance, DWORD dwModelIndex, const char *c_szBoneName);
    void SetPosition(float fx, float fy, float fz);
    void SetRotation(float fRotation);

    void SetLifeTime(float fLifeTime);
    void SetSamplingTime(float fSamplingTime);

    void Update(float fReachScale);
    void Render();

    void Initialize();

  protected:
    bool BuildVertex();

    float m_fLastUpdate = 0.0F;

    using TTimePoint = std::pair<float, D3DXVECTOR3>;
    using TTimePointList = std::deque<TTimePoint>;
    TTimePointList m_ShortTimePointList;
    TTimePointList m_LongTimePointList;

    std::vector<TPDTVertex> m_PDTVertexVector;

    float m_fLifeTime = 0.0F;
    float m_fSamplingTime = 0.0F;

    // std::vector<TPDTVertex> m_PDTVertexVector;
    // std::vector<TPDTVertex> m_CurvingTraceVector;
    // std::vector<TSplineValue> m_SplineValueVector;

    CGraphicThingInstance *m_pInstance = nullptr;
    DWORD m_dwModelInstanceIndex = 0U;

    CGraphicImageInstance m_ImageInstance;

    float m_fx = 0.0F;
    float m_fy = 0.0F;
    float m_fz = 0.0F;
    float m_fRotation = 0.0F;
    float m_fLength = 0.0F;

    BOOL m_isPlaying = 0;
    bool m_bUseTexture = false;

    int m_iBoneIndex = 0;

    static CDynamicPool<CWeaponTrace> ms_kPool;
};
