#pragma once

void Environment_Init(SEnvironmentData &envData);
bool Environment_Load(SEnvironmentData &envData, const char *envFileName);

void GetInterpolatedPosition(float curPositionRate, TPixelPosition *PixelPosition);
float GetLinearInterpolation(float begin, float end, float curRate);

void PixelPositionToAttributeCellPosition(TPixelPosition PixelPosition, TCellPosition *pAttrCellPosition);
void AttributeCellPositionToPixelPosition(TCellPosition AttrCellPosition, TPixelPosition *pPixelPosition);

float GetPixelPositionDistance(const TPixelPosition &c_rsrcPosition, const TPixelPosition &c_rdstPosition);

class CEaseOutInterpolation
{
  public:
    CEaseOutInterpolation();
    virtual ~CEaseOutInterpolation();

    void Initialize();

    BOOL Setup(float fStart, float fEnd, float fTime);
    void Interpolate(float fElapsedTime);
    BOOL isPlaying() const;

    float GetValue() const;
    float GetChangingValue() const;

  protected:
    float m_fRemainingTime = 0.0F;
    float m_fValue = 0.0F;
    float m_fSpeed = 0.0F;
    float m_fAcceleration = 0.0F;

    float m_fStartValue = 0.0F;
    float m_fLastValue = 0.0F;
};
