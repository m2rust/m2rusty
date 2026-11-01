///////////////////////////////////////////////////////////////////////
//	CLensFlare Class
//
//	(c) 2003 IDV, Inc.
//
//	*** INTERACTIVE DATA VISUALIZATION (IDV) PROPRIETARY INFORMATION ***
//
//	This software is supplied under the terms of a license agreement or
//	nondisclosure agreement with Interactive Data Visualization and may
//	not be copied or disclosed except in accordance with the terms of
//	that agreement.
//
//      Copyright (c) 2001-2003 IDV, Inc.
//      All Rights Reserved.
//
//		IDV, Inc.
//		1233 Washington St. Suite 610
//		Columbia, SC 29201
//		Voice: (803) 799-1699
//		Fax:   (803) 931-0320
//		Web:   http://www.idvinc.com
//

///////////////////////////////////////////////////////////////////////
//	Preprocessor

#pragma once

#include "GrpImageInstance.h"
#include "GrpScreen.h"

#include <float.h>
#include <string>
#include <vector>

///////////////////////////////////////////////////////////////////////
//	Constants

const float c_fHalfMaxBright = 0.45f;
const float c_fDistanceScale = 1.0f;
const int c_nDepthTestDimension = 15;

///////////////////////////////////////////////////////////////////////
//	CFlare

class CFlare
{
  public:
    void Draw(float fBrightScale, int nWidth, int nHeight, int nX, int nY);
    void Init(const std::string &strPath);

    CFlare();
    virtual ~CFlare();

  private:
    struct SFlarePiece
    {
        SFlarePiece() : m_pColor(nullptr)
        {
        }
        CGraphicImageInstance m_imageInstance;
        float m_fPosition = 0.0f; // -1.0 = light location, 0.0 = center, 1.0 = far end of
                                  // flare
        float m_fWidth = 0.0f;    // height = width
        float *m_pColor;
    };

    std::vector<SFlarePiece *> m_vFlares;
};

///////////////////////////////////////////////////////////////////////
//	CLensFlare
class CLensFlare : public CScreen
{
  public:
    CLensFlare();
    ~CLensFlare() override;

    void Compute(const D3DXVECTOR3 &c_rv3LightDirection); // D3DTS_VIEW로 부터 카메라 방향을
                                                          // 얻어오므로, 카메라 설정 뒤에 해야 함.

    void DrawBeforeFlare();
    void DrawAfterFlare();
    void DrawFlare();

    void SetMainFlare(const std::string &strSunFile, float fSunSize);
    void Initialize(std::string strPath);

    void SetFlareLocation(double dX, double dY);

    void SetVisible(bool bState)
    {
        m_bFlareVisible = bState;
    }
    bool IsVisible() const
    {
        return m_bFlareVisible;
    }

    void SetBrightnesses(float fBeforeBright, float fAfterBright);

    void ReadControlPixels();
    void AdjustBrightness();

    void CharacterizeFlare(bool bEnabled, bool bShowMainFlare, float fMaxBrightness, const D3DXCOLOR &c_rColor);

  protected:
    float Interpolate(float fStart, float fEnd, float fPercent);

  private:
    float m_afFlarePos[2]{}, m_afFlareWinPos[2]{};
    float m_fBeforeBright = 0.0f, m_fAfterBright = 0.0f;
    bool m_bFlareVisible = false, m_bDrawFlare = true, m_bDrawBrightScreen = true;
    float m_fSunSize = 0;
    CFlare m_cFlare;
    float *m_pControlPixels;
    float *m_pTestPixels;
    bool m_bEnabled = true;
    bool m_bShowMainFlare = true;
    float m_fMaxBrightness = 1.0f;
    float m_afColor[4]{};

    CGraphicImageInstance m_SunFlareImageInstance;

    void ReadDepthPixels(float *pPixels);
    void ClampBrightness();
};
