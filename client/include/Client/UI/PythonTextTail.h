#pragma once

#include <Neoeter/Platform/Base/Singleton.h>

/*
 *	따라다니는 텍스트 처리
 */
class CPythonTextTail : public CSingleton<CPythonTextTail>
{
  public:
    using TTextTail = struct STextTail
    {
        CGraphicTextInstance *pTextInstance = nullptr;
        CGraphicTextInstance *pOwnerTextInstance = nullptr;

        CGraphicMarkInstance *pMarkInstance = nullptr;
        CGraphicTextInstance *pGuildNameTextInstance = nullptr;

        CGraphicTextInstance *pTitleTextInstance = nullptr;
        CGraphicTextInstance *pLevelTextInstance = nullptr;

        // Todo : 이건 전부 VID로 바꾸도록 한다.
        //        도중 캐릭터가 없어질 경우 튕길 가능성이 있음
        CGraphicObjectInstance *pOwner = nullptr;

        DWORD dwVirtualID = 0U;

        float x = 0.0F, y = 0.0F, z = 0.0F;
        float fDistanceFromPlayer = 0.0F;
        D3DXCOLOR Color;
        BOOL bNameFlag = 0; // 이름도 함께 켤것인지의 플래그

        float xStart = 0.0F, yStart = 0.0F;
        float xEnd = 0.0F, yEnd = 0.0F;

        DWORD LivingTime = 0U;

        float fHeight = 0.0F;

        STextTail()
        {
        }
        virtual ~STextTail()
        {
        }
    };

    using TTextTailMap = std::map<DWORD, TTextTail *>;
    using TTextTailList = std::list<TTextTail *>;
    using TChatTailMap = TTextTailMap;

    CPythonTextTail();
    ~CPythonTextTail() override;

    void GetInfo(std::string *pstInfo);

    void Initialize();
    void Destroy();
    void Clear();

    void UpdateAllTextTail();
    void UpdateShowingTextTail();
    void Render();

    void ArrangeTextTail();
    void HideAllTextTail();
    void ShowAllTextTail();
    void ShowCharacterTextTail(DWORD VirtualID);
    void ShowItemTextTail(DWORD VirtualID);

    void RegisterCharacterTextTail(DWORD dwGuildID, DWORD dwVirtualID, const D3DXCOLOR &c_rColor,
                                   float fAddHeight = 10.0f);
    void RegisterItemTextTail(DWORD VirtualID, const char *c_szText, CGraphicObjectInstance *pOwner);
    void RegisterChatTail(DWORD VirtualID, const char *c_szChat);
    void RegisterInfoTail(DWORD VirtualID, const char *c_szChat);
    void SetCharacterTextTailColor(DWORD VirtualID, const D3DXCOLOR &c_rColor);
    void SetItemTextTailOwner(DWORD dwVID, const char *c_szName);
    void DeleteCharacterTextTail(DWORD VirtualID);
    void DeleteItemTextTail(DWORD VirtualID);

    int Pick(int ixMouse, int iyMouse);
    void SelectItemName(DWORD dwVirtualID);

    bool GetTextTailPosition(DWORD dwVID, float *px, float *py, float *pz);
    bool IsChatTextTail(DWORD dwVID);

    void EnablePKTitle(BOOL bFlag);
    void AttachTitle(DWORD dwVID, const char *c_szName, const D3DXCOLOR &c_rColor);
    void DetachTitle(DWORD dwVID);

    void AttachLevel(DWORD dwVID, const char *c_szText, const D3DXCOLOR &c_rColor);
    void DetachLevel(DWORD dwVID);

  protected:
    TTextTail *RegisterTextTail(DWORD dwVirtualID, const char *c_szText, CGraphicObjectInstance *pOwner, float fHeight,
                                const D3DXCOLOR &c_rColor);
    void DeleteTextTail(TTextTail *pTextTail);

    void UpdateTextTail(TTextTail *pTextTail);
    void RenderTextTailBox(TTextTail *pTextTail);
    void RenderTextTailName(TTextTail *pTextTail);
    void UpdateDistance(const TPixelPosition &c_rCenterPosition, TTextTail *pTextTail);

    bool isIn(TTextTail *pSource, TTextTail *pTarget);

    TTextTailMap m_CharacterTextTailMap;
    TTextTailMap m_ItemTextTailMap;
    TChatTailMap m_ChatTailMap;

    TTextTailList m_CharacterTextTailList;
    TTextTailList m_ItemTextTailList;

  private:
    CDynamicPool<STextTail> m_TextTailPool;
};
