#pragma once

class CFlyTrace : public CScreen
{
  public:
    CFlyTrace();
    ~CFlyTrace() override;

    void Destroy();

    void Create(const CFlyingData::TFlyingAttachData &rFlyingAttachData);

    void UpdateNewPosition(const D3DXVECTOR3 &v3Position);

    void Update();
    void Render();

  protected:
    void __Initialize();

    using TTimePosition = std::pair<float, D3DXVECTOR3>;
    using TTimePositionDeque = std::deque<TTimePosition>;

    // CGraphicImageInstance m_ImageInstance;
    // LPDIRECT3DTEXTURE9 m_lpTexture;

    bool m_bRectShape = false;
    DWORD m_dwColor = 0U;
    float m_fSize = 0.0F;
    float m_fTailLength = 0.0F;

    TTimePositionDeque m_TimePositionDeque;

  public:
    static void DestroySystem();

    static CFlyTrace *New();
    static void Delete(CFlyTrace *pkInst);

    static CDynamicPool<CFlyTrace> ms_kPool;
};