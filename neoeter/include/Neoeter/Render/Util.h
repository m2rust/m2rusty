#pragma once

#include <Neoeter/Platform/Base/FileLoader.h>

#include <map>
#include <vector>

#ifdef _WIN32
#include "Platform/DirectXMathCompat.h"
#else
#include "Platform/DirectXMathCompat.h"
#endif

template <typename T> class CTransitor
{
  public:
    CTransitor() = default;
    ~CTransitor() = default;

    void SetActive(BOOL bActive = TRUE)
    {
        m_bActivated = bActive;
    }

    BOOL isActive()
    {
        return m_bActivated;
    }

    BOOL isActiveTime(float fcurTime)
    {
        if (fcurTime >= m_fEndTime)
            return FALSE;

        return TRUE;
    }

    DWORD GetID()
    {
        return m_dwID;
    }

    void SetID(DWORD dwID)
    {
        m_dwID = dwID;
    }

    void SetSourceValue(const T &c_rSourceValue)
    {
        m_SourceValue = c_rSourceValue;
    }

    void SetTransition(const T &c_rSourceValue, const T &c_rTargetValue, float fStartTime, float fBlendTime)
    {
        m_SourceValue = c_rSourceValue;
        m_TargetValue = c_rTargetValue;
        m_fStartTime = fStartTime;
        m_fEndTime = fStartTime + fBlendTime;
    }

    BOOL GetValue(float fcurTime, T *pValue)
    {
        if (fcurTime <= m_fStartTime)
            return FALSE;

        float const fPercentage = (fcurTime - m_fStartTime) / (m_fEndTime - m_fStartTime);
        *pValue = m_SourceValue + ((m_TargetValue - m_SourceValue) * fPercentage);
        return TRUE;
    }

  protected:
    DWORD m_dwID = 0U; // Public Transitor ID

    BOOL m_bActivated = 0; // Have been started to blend?
    float m_fStartTime = 0.0F;
    float m_fEndTime = 0.0F;

    T m_SourceValue;
    T m_TargetValue;
};

using TTransitorFloat = CTransitor<float>;
using TTransitorVector3 = CTransitor<D3DXVECTOR3>;
using TTransitorColor = CTransitor<D3DXCOLOR>;

///////////////////////////////////////////////////////////////////////////////////////////////////

void PrintfTabs(FILE *File, int iTabCount, const char *c_szString, ...);

// typedef CTokenVector TTokenVector;

extern bool LoadTextData(const char *c_szFileName, CTokenMap &rstTokenMap);
extern bool LoadMultipleTextData(const char *c_szFileName, CTokenVectorMap &rstTokenVectorMap);

extern D3DXVECTOR3 TokenToVector(CTokenVector &rVector);
extern D3DXCOLOR TokenToColor(CTokenVector &rVector);

#define GOTO_CHILD_NODE(TextFileLoader, Index) CTextFileLoader::CGotoChild Child(TextFileLoader, Index);

///////////////////////////////////////////////////////////////////////////////////////////////////

extern int CALLBACK EnumFontFamExProc(CONST LOGFONT *plogFont, CONST TEXTMETRIC *textMetric, DWORD dwWord,
                                      LPARAM lParam);
extern int GetCharsetFromCodePage(WORD codePage);
extern const char *GetFontFaceFromCodePageNT(WORD codePage);
extern const char *GetFontFaceFromCodePage9x(WORD codePage);
extern DWORD GetDefaultCodePage();
extern const char *GetDefaultFontFace();
extern const char *GetFontFaceFromCodePage(WORD codePage);
extern void SetDefaultFontFace(const char *fontFace);
extern bool SetDefaultCodePage(DWORD codePage);
extern void base64_decode(const char *str, char *resultStr);

extern DWORD GetMaxTextureWidth();
extern DWORD GetMaxTextureHeight();