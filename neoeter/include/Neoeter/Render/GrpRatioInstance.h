#pragma once

class CGraphicRatioInstance
{
  public:
    CGraphicRatioInstance();
    virtual ~CGraphicRatioInstance();

    void Clear();

    void SetRatioReference(const float &ratio);
    void BlendRatioReference(DWORD blendTime, const float &ratio);

    void Update();

    const float &GetCurrentRatioReference() const;

  protected:
    DWORD GetTime();

    float m_curRatio = 0.0F;
    float m_srcRatio = 0.0F;
    float m_dstRatio = 0.0F;

    DWORD m_baseTime;
    DWORD m_blendTime;
};
