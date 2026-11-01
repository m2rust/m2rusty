#pragma once

class CColorTransitionHelper
{
  public:
    CColorTransitionHelper();
    ~CColorTransitionHelper();

    void Clear(const float &c_rfRed, const float &c_rfGreen, const float &c_rfBlue, const float &c_rfAlpha);
    void SetSrcColor(const float &c_rfRed, const float &c_rfGreen, const float &c_rfBlue, const float &c_rfAlpha);
    void SetTransition(const float &c_rfRed, const float &c_rfGreen, const float &c_rfBlue, const float &c_rfAlpha,
                       const DWORD &dwDuration);
    const D3DCOLOR &GetCurColor() const; // { return m_dwCurColor; }

    void StartTransition();
    bool Update();

    bool isTransitionStarted() const
    {
        return m_bTransitionStarted;
    }

  private:
    D3DCOLOR m_dwCurColor = 0U; // 현재 색

    DWORD m_dwStartTime = 0U; // 바뀌기 시작하는 시간
    DWORD m_dwDuration = 0U;  // 얼마 동안에 바뀌는가?

    bool m_bTransitionStarted = false;

    float m_fSrcRed = 0.0F, m_fSrcGreen = 0.0F, m_fSrcBlue = 0.0F, m_fSrcAlpha = 0.0F;
    float m_fDstRed = 0.0F, m_fDstGreen = 0.0F, m_fDstBlue = 0.0F, m_fDstAlpha = 0.0F;
};
