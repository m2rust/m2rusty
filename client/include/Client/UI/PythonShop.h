#pragma once

#include <Client/UI/Packet.h>

/*
 *	상점 처리
 *
 *	2003-01-16 anoa	일차 완료
 *	2003-12-26 levites 수정
 *
 *	2012-10-29 rtsummit 새로운 화폐 출현 및 tab 기능 추가로 인한 shop 확장.
 *
 */
typedef enum
{
    SHOP_COIN_TYPE_GOLD, // DEFAULT VALUE
    SHOP_COIN_TYPE_SECONDARY_COIN,
} EShopCoinType;

class CPythonShop : public CSingleton<CPythonShop>
{
  public:
    CPythonShop();
    ~CPythonShop() override;

    void Clear();

    void SetItemData(DWORD dwIndex, const TShopItemData &c_rShopItemData);
    BOOL GetItemData(DWORD dwIndex, const TShopItemData **c_ppItemData);

    void SetItemData(BYTE tabIdx, DWORD dwSlotPos, const TShopItemData &c_rShopItemData);
    BOOL GetItemData(BYTE tabIdx, DWORD dwSlotPos, const TShopItemData **c_ppItemData);

    void SetTabCount(BYTE bTabCount)
    {
        m_bTabCount = bTabCount;
    }
    BYTE GetTabCount() const
    {
        return m_bTabCount;
    }

    void SetTabCoinType(BYTE tabIdx, BYTE coinType);
    BYTE GetTabCoinType(BYTE tabIdx);

    void SetTabName(BYTE tabIdx, const char *name);
    const char *GetTabName(BYTE tabIdx);

    // BOOL GetSlotItemID(DWORD dwSlotPos, DWORD* pdwItemID);

    void Open(BOOL isPrivateShop, BOOL isMainPrivateShop);
    void Close();
    BOOL IsOpen() const;
    BOOL IsPrivateShop() const;
    BOOL IsMainPlayerPrivateShop() const;

    void ClearPrivateShopStock();
    void AddPrivateShopItemStock(TItemPos ItemPos, BYTE dwDisplayPos, DWORD dwPrice);
    void DelPrivateShopItemStock(TItemPos ItemPos);
    int GetPrivateShopItemPrice(TItemPos ItemPos);
    void BuildPrivateShop(const char *c_szName);

  protected:
    BOOL CheckSlotIndex(DWORD dwIndex);

    BOOL m_isShoping = 0;
    BOOL m_isPrivateShop = 0;
    BOOL m_isMainPlayerPrivateShop = 0;

    struct ShopTab
    {
        ShopTab() : coinType(SHOP_COIN_TYPE_GOLD)
        {
        }
        BYTE coinType;
        std::string name;
        TShopItemData items[SHOP_HOST_ITEM_MAX_NUM]{};
    };

    BYTE m_bTabCount = 0U;
    ShopTab m_aShoptabs[SHOP_TAB_COUNT_MAX];

    using TPrivateShopItemStock = std::map<TItemPos, TShopItemTable>;
    TPrivateShopItemStock m_PrivateShopItemStock;
};
