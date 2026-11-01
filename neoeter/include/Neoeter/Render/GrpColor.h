#pragma once

class CGraphicColor
{
  public:
    CGraphicColor(const CGraphicColor &c_rSrcColor);
    CGraphicColor(float r, float g, float b, float a);
    CGraphicColor(DWORD color);

    CGraphicColor();
    ~CGraphicColor() = default;

    void Clear();

    void Set(float r, float g, float b, float a);
    void Set(const CGraphicColor &c_rSrcColor);
    void Set(DWORD pack);

    void Blend(float p, const CGraphicColor &c_rSrcColor, const CGraphicColor &c_rDstColor);

    DWORD GetPackValue() const;

  protected:
    float m_r = 0.0F;
    float m_g = 0.0F;
    float m_b = 0.0F;
    float m_a = 0.0F;
};
